// The passthrough: checks a request against its connector, reserves credits, forwards it with
// our credentials, charges what the upstream reports and hides our credentials from the reply.
// Usage records keep metadata only: never bodies, queries or result content.
//
// Answers are kept for every account (ADR 0079): a question asked again within its connector's
// cache rule is answered from the cache, without asking the provider, at a discounted price.
// `Cache-Control: no-cache` asks the provider anyway (and keeps the new answer);
// `X-Octopus-Cache: peek` only says whether an answer is kept and what it would cost.

import {
  cachedAnswer,
  cacheKey,
  cacheSeconds,
  chargedSince,
  chargeFor,
  type Connector,
  type Db,
  discounted,
  DuplicateRequest,
  hasCachedAnswer,
  hold,
  holdFor,
  injectFields,
  InsufficientFunds,
  keepAnswer,
  maskSecrets,
  microsToUsd,
  monthSpend,
  normalizePath,
  noteCacheHit,
  noteProviderProblem,
  parseJson,
  recordUsage,
  routeAllowed,
  routePattern,
  secretForms,
  secretNames,
  settle,
  settleHold,
  type TokenOwner,
  upstreamHeaders,
  upstreamUrl,
  usdToMicros,
  wallet as walletOf,
} from "@octopusseo/cloud-core";

import type { Concurrency, RateLimiter } from "./limits.ts";
import type { Registry } from "./registry.ts";

/** Upstream replies larger than this are refused (and not charged). */
export const MAX_RESPONSE_BYTES = 32 * 1024 * 1024;

/** The charge of a request in US dollars, read by the apps for their budgets. */
export const CHARGE_HEADER = "X-Octopus-Charge-Usd";
/** The balance left for new requests, in US dollars. */
export const AVAILABLE_HEADER = "X-Octopus-Available-Usd";
/** `hit` (answered from the cache), `miss` (asked, and kept), absent when not kept; `peek` asks. */
export const CACHE_HEADER = "X-Octopus-Cache";
/** How old a cached answer is, in seconds. */
export const CACHE_AGE_HEADER = "X-Octopus-Cache-Age";
/** What a cached answer would have cost without the discount, in US dollars. */
export const FULL_CHARGE_HEADER = "X-Octopus-Full-Charge-Usd";

/** Questions on their way to a provider, by cache key: the same question waits for the answer. */
const inflight = new Map<string, Promise<void>>();

export interface ProxyDeps {
  db: Db;
  registry: Registry;
  fetch: (request: Request) => Promise<Response>;
  limiter: RateLimiter;
  concurrency: Concurrency;
  log: (event: Record<string, unknown>) => void;
  now?: () => number;
}

/**
 * What a provider says about a problem with our account (DataForSEO's `status_code` and
 * `status_message`, or a `message` or `error`), for our log: the provider's words about us,
 * never the user's request or data.
 */
export function accountProblem(body: unknown): string | undefined {
  if (!body || typeof body !== "object") return undefined;
  const envelope = body as Record<string, unknown>;
  // DataForSEO answers "20000 Ok." on the envelope and gives the reason on the task.
  const task = Array.isArray(envelope.tasks) && envelope.tasks[0] && typeof envelope.tasks[0] === "object" ? (envelope.tasks[0] as Record<string, unknown>) : null;
  const fields = envelope.status_code === 20000 && task ? task : envelope;
  const text = [fields.status_code, fields.status_message ?? fields.message ?? fields.error]
    .filter((value) => typeof value === "string" || typeof value === "number")
    .join(" ")
    .trim();
  return text ? text.slice(0, 200) : undefined;
}

/** A JSON error the apps can show and map by status (401, 402, 429 and 503 make them fall back). */
export function problem(
  status: number,
  error: string,
  message: string,
  headers: Record<string, string> = {},
  extra: Record<string, unknown> = {},
): Response {
  return new Response(JSON.stringify({ error, message, ...extra }), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...headers },
  });
}

const TOO_LARGE = Symbol("too large");

/** Reads a body up to `limit` bytes. */
async function readLimited(stream: ReadableStream<Uint8Array> | null, limit: number): Promise<Uint8Array | typeof TOO_LARGE> {
  if (!stream) return new Uint8Array();
  const chunks: Uint8Array[] = [];
  let size = 0;
  const reader = stream.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > limit) {
      await reader.cancel().catch(() => undefined);
      return TOO_LARGE;
    }
    chunks.push(value);
  }
  return Buffer.concat(chunks);
}

function usd(micros: number): string {
  return microsToUsd(micros).toFixed(6);
}

/** The full price of an answer whose provider cost was `upstreamMicros`. */
function fullPrice(connector: Connector, defaultMarkup: number, upstreamMicros: number): number {
  return chargeFor(upstreamMicros, connector.pricing.markup ?? defaultMarkup, usdToMicros(connector.pricing.min_charge_usd));
}

/** Reserves credits for a request: its hold id, or the refusal to send. */
async function reserve(
  deps: ProxyDeps,
  owner: TokenOwner,
  connectorId: string,
  micros: number,
  key: string | null,
): Promise<string | Response> {
  try {
    const { holdId } = await hold(deps.db, owner.userId, micros, {
      connectorId,
      tokenId: owner.tokenId,
      idempotencyKey: key ? `${owner.userId}:${key.slice(0, 120)}` : undefined,
    });
    return holdId;
  } catch (error) {
    if (error instanceof InsufficientFunds) {
      return problem(402, "insufficient_credits", "Not enough OctopusOn SEO credits.", {}, {
        available_usd: microsToUsd(error.availableMicros),
        needed_usd: microsToUsd(micros),
      });
    }
    if (error instanceof DuplicateRequest || (error as { code?: string }).code === "23505") {
      return problem(409, "duplicate_request", "This request was already sent.");
    }
    throw error;
  }
}

function startOfUtcDay(now: number): Date {
  const date = new Date(now);
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()));
}

/** Forwards `request` (whose path is `/v1/p/<connectorId>/…`) for a signed-in user. */
export async function forward(
  deps: ProxyDeps,
  owner: TokenOwner,
  connectorId: string,
  request: Request,
  app: string | null,
): Promise<Response> {
  const now = deps.now ?? Date.now;
  const started = now();
  const url = new URL(request.url);
  const loaded = await deps.registry.connector(connectorId);
  if (!loaded || (loaded.connector.status === "test" && owner.role !== "admin")) {
    return problem(503, "connector_unavailable", "This source is not available with OctopusOn SEO credits right now.");
  }
  const { connector, secrets } = loaded;
  const method = request.method.toUpperCase();
  const raw = url.pathname.slice(`/v1/p/${connectorId}`.length) || "/";
  const path = normalizePath(raw);
  if (!path) return problem(400, "invalid_path", "The path is not allowed.");
  if (!routeAllowed(connector.routes, connector.deny, method, path)) {
    return problem(403, "route_not_allowed", `${method} ${path} is not open through OctopusOn SEO credits.`);
  }
  if (secretNames(connector).some((name) => !secrets[name])) {
    deps.log({ level: "error", event: "connector_secret_missing", connector: connector.id });
    return problem(503, "connector_unavailable", "This source is not available with OctopusOn SEO credits right now.");
  }

  let body: Uint8Array | null = null;
  // The question as asked, before our credentials are written into it: what the cache key uses.
  let asked: Uint8Array | null = null;
  if (method === "POST") {
    const limit = connector.limits.max_body_kb * 1024;
    if (Number(request.headers.get("content-length") ?? 0) > limit) return problem(413, "body_too_large", "The request body is too large.");
    const read = await readLimited(request.body, limit);
    if (read === TOO_LARGE) return problem(413, "body_too_large", "The request body is too large.");
    body = read;
    asked = read;
    if (connector.inject.length > 0) {
      body = injectFields(connector, body, secrets);
      if (!body) return problem(400, "invalid_body", "This source expects a JSON object or a list of objects.");
    }
  }

  const wait = deps.limiter.take(`${owner.tokenId}:${connector.id}`, connector.limits.rpm);
  if (wait > 0) return problem(429, "rate_limited", "Too many requests; try again shortly.", { "retry-after": String(wait) });
  const release = deps.concurrency.acquire(`${owner.userId}:${connector.id}`, connector.limits.concurrency);
  if (!release) return problem(429, "too_many_parallel", "Too many requests at once; try again shortly.", { "retry-after": "1" });

  let done: (() => void) | undefined;
  let waitingFor: string | null = null;
  try {
    const settings = await deps.registry.settings();
    const route = routePattern(connector.routes, method, path);
    const seconds = settings.cacheEnabled ? cacheSeconds(connector, method, path) : 0;
    const key = seconds > 0 ? cacheKey(connector, method, path, url.searchParams, asked) : null;
    const refresh = /no-cache|no-store/i.test(request.headers.get("cache-control") ?? "");
    if (request.headers.get(CACHE_HEADER)?.toLowerCase() === "peek") {
      // The price of a question before asking it: free, and the provider is not asked.
      const kept = key ? await hasCachedAnswer(deps.db, key, new Date(now())) : null;
      const full = kept ? fullPrice(connector, settings.markup, kept.upstreamMicros) : 0;
      return new Response(
        JSON.stringify({
          cached: kept !== null,
          cacheable: key !== null,
          age_s: kept ? Math.max(0, Math.round((now() - kept.createdAt.getTime()) / 1000)) : null,
          charge_usd: kept ? microsToUsd(discounted(full, settings.cacheDiscountPct)) : null,
          full_charge_usd: kept ? microsToUsd(full) : null,
        }),
        {
          status: 200,
          headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", [CACHE_HEADER]: kept ? "hit" : "miss" },
        },
      );
    }
    if (key) {
      // The same question on its way to the provider for someone else: wait for its answer,
      // then become the one others wait for (no await between the check and the claim).
      const pending = refresh ? undefined : inflight.get(key);
      if (pending) await pending;
      if (!inflight.has(key)) {
        waitingFor = key;
        inflight.set(key, new Promise<void>((resolve) => (done = resolve)));
      }
    }
    const holdMicros = holdFor(connector, method, path);
    if (owner.monthlyCapMicros !== null && (await monthSpend(deps.db, owner.tokenId)) + holdMicros > owner.monthlyCapMicros) {
      return problem(402, "token_limit", "This device reached its monthly spending limit.");
    }
    if (
      settings.dailyLimitMicros > 0 &&
      (await chargedSince(deps.db, owner.userId, startOfUtcDay(now()))) + holdMicros > settings.dailyLimitMicros
    ) {
      return problem(402, "daily_limit", "The daily spending limit of this account is reached.");
    }

    const idempotency = request.headers.get("idempotency-key");
    const usage = {
      userId: owner.userId,
      tokenId: owner.tokenId,
      connectorId: connector.id,
      method,
      route,
      app,
    };

    if (key && !refresh) {
      const kept = await cachedAnswer(deps.db, key, new Date(now()));
      if (kept) {
        const full = fullPrice(connector, settings.markup, kept.upstreamMicros);
        const price = discounted(full, settings.cacheDiscountPct);
        let available: number;
        if (price > 0) {
          const reserved = await reserve(deps, owner, connector.id, price, idempotency);
          if (reserved instanceof Response) return reserved;
          available = (await settleHold(deps.db, reserved, price)).availableMicros;
        } else {
          available = (await walletOf(deps.db, owner.userId)).availableMicros;
        }
        await noteCacheHit(deps.db, key, new Date(now()));
        await recordUsage(deps.db, {
          ...usage,
          status: kept.status,
          chargedMicros: price,
          cached: true,
          savedMicros: kept.upstreamMicros,
          fullChargeMicros: full,
          durationMs: now() - started,
        });
        deps.log({ level: "info", event: "cache_hit", connector: connector.id, route, key: key.slice(0, 8), charged_micros: price, ms: now() - started });
        const headers = new Headers({
          "cache-control": "no-store",
          [CHARGE_HEADER]: usd(price),
          [AVAILABLE_HEADER]: usd(available),
          [FULL_CHARGE_HEADER]: usd(full),
          [CACHE_HEADER]: "hit",
          [CACHE_AGE_HEADER]: String(Math.max(0, Math.round((now() - kept.createdAt.getTime()) / 1000))),
        });
        if (kept.contentType) headers.set("content-type", kept.contentType);
        return new Response(Buffer.from(kept.body), { status: kept.status, headers });
      }
    }

    const reserved = await reserve(deps, owner, connector.id, holdMicros, idempotency);
    if (reserved instanceof Response) return reserved;
    const holdId = reserved;
    const target = upstreamUrl(connector, raw, url.searchParams, secrets);
    let upstream: Response;
    let reply: Uint8Array | typeof TOO_LARGE;
    try {
      upstream = await deps.fetch(
        new Request(target, {
          method,
          headers: upstreamHeaders(connector, request.headers, secrets),
          body: method === "POST" ? body : null,
          redirect: "manual",
          signal: AbortSignal.timeout(connector.limits.timeout_s * 1000),
        }),
      );
      reply = await readLimited(upstream.body, MAX_RESPONSE_BYTES);
    } catch (error) {
      await settleHold(deps.db, holdId, 0);
      const timeout = error instanceof Error && (error.name === "TimeoutError" || error.name === "AbortError");
      const status = timeout ? 504 : 502;
      await recordUsage(deps.db, { ...usage, status, durationMs: now() - started });
      deps.log({ level: "warn", event: "upstream_failed", connector: connector.id, route, status });
      return problem(status, timeout ? "upstream_timeout" : "upstream_failed", "The source did not answer; nothing was charged.");
    }
    if (reply === TOO_LARGE) {
      await settleHold(deps.db, holdId, 0);
      await recordUsage(deps.db, { ...usage, status: 502, upstreamStatus: upstream.status, durationMs: now() - started });
      return problem(502, "upstream_too_large", "The source's answer was too large; nothing was charged.");
    }

    const answer = parseJson(reply, upstream.headers.get("content-type"));
    const outcome = settle(connector, settings.markup, upstream.status, answer);
    const wallet = await settleHold(deps.db, holdId, outcome.chargeMicros);
    // Our account's problems are not the user's: make the apps fall back instead of signing out.
    const ours = upstream.status === 401 || upstream.status === 402 || upstream.status === 403;
    const redirected = upstream.status >= 300 && upstream.status < 400;
    const status = ours ? 503 : redirected ? 502 : upstream.status;
    await recordUsage(deps.db, {
      ...usage,
      status,
      upstreamStatus: upstream.status,
      upstreamCostMicros: outcome.upstreamMicros,
      chargedMicros: outcome.chargeMicros,
      durationMs: now() - started,
    });
    deps.log({
      level: ours ? "error" : "info",
      event: ours ? "upstream_account" : "forwarded",
      connector: connector.id,
      route,
      status,
      upstream_status: upstream.status,
      ...(ours ? { upstream_message: accountProblem(answer) } : {}),
      charged_micros: outcome.chargeMicros,
      ms: now() - started,
    });
    const charge = { [CHARGE_HEADER]: usd(outcome.chargeMicros), [AVAILABLE_HEADER]: usd(wallet.availableMicros) };
    if (ours) {
      // Our account's problem (a spent balance, a closed API): the admin panel shows it.
      await noteProviderProblem(deps.db, connector.id, { route, status: upstream.status, message: accountProblem(answer) }).catch(() => false);
      return problem(503, "upstream_unavailable", "This source is not available with OctopusOn SEO credits right now.", charge);
    }
    if (redirected) return problem(502, "upstream_redirect", "The source answered with a redirect.", charge);

    const headers = new Headers({ "cache-control": "no-store", ...charge });
    const contentType = upstream.headers.get("content-type");
    if (contentType) headers.set("content-type", contentType);
    const retryAfter = upstream.headers.get("retry-after");
    if (upstream.status === 429 || upstream.status === 503) headers.set("retry-after", retryAfter ?? "30");
    const masked = maskSecrets(reply, secretForms(connector, secrets));
    if (key && upstream.status === 200 && !outcome.error) {
      // Kept for everyone who asks the same within the rule's time; our credentials masked.
      const kept = await keepAnswer(deps.db, {
        key,
        connectorId: connector.id,
        route,
        status: 200,
        contentType,
        body: masked,
        upstreamMicros: outcome.upstreamMicros,
        seconds,
        now: new Date(now()),
        maxBytes: settings.cacheMaxEntryKb * 1024,
      }).catch((error: unknown) => {
        deps.log({ level: "warn", event: "cache_write_failed", connector: connector.id, message: error instanceof Error ? error.message : String(error) });
        return false;
      });
      if (kept) headers.set(CACHE_HEADER, "miss");
    }
    return new Response(masked, { status, headers });
  } finally {
    if (waitingFor) inflight.delete(waitingFor);
    done?.();
    release();
  }
}
