// api.octopuson.com routes: device sign-in, catalog, account, metered server work and the
// connector passthrough.

import type { HttpBindings } from "@hono/node-server";
import {
  accountOf,
  catalogEntry,
  type Db,
  InsufficientFunds,
  isMeterSku,
  meterCatalog,
  meterTerms,
  meterUsage,
  pollDevice,
  recordSupportMessage,
  redeemConnectCode,
  revokeToken,
  startDevice,
  type SupportContext,
  type TokenOwner,
  verifyToken,
  wallet,
} from "@octopusseo/cloud-core";
import { type Context, Hono, type MiddlewareHandler } from "hono";

import { forward, problem, type ProxyDeps } from "./proxy.ts";

export interface AppDeps extends ProxyDeps {
  db: Db;
  key: Buffer;
  portalUrl: string;
  trustProxy: boolean;
}

type Env = { Bindings: Partial<HttpBindings>; Variables: { owner: TokenOwner } };

/** The client address: the proxy's X-Forwarded-For when trusted, else the socket. */
function clientIp(c: Context<Env>, trustProxy: boolean): string {
  if (trustProxy) {
    const forwarded = c.req.header("x-forwarded-for")?.split(",").at(-1)?.trim();
    if (forwarded) return forwarded;
  }
  return c.env?.incoming?.socket?.remoteAddress ?? "unknown";
}

/** `keyword-miner/0.1.0 macOS` from the apps, trimmed to safe characters. */
function appName(c: Context<Env>): string | null {
  const value = c.req.header("x-octopus-app")?.replace(/[^\w./ ()-]/g, "").slice(0, 60);
  return value || null;
}

async function owner(c: Context<Env>, db: Db): Promise<TokenOwner | null> {
  const header = c.req.header("authorization") ?? "";
  const match = /^Bearer\s+(\S+)$/i.exec(header);
  return match ? verifyToken(db, match[1]!) : null;
}

function unauthorized(): Response {
  return problem(401, "invalid_token", "Sign in to your OctopusOn SEO account again.", {
    "www-authenticate": 'Bearer realm="OctopusOn SEO"',
  });
}

export function createApp(deps: AppDeps): Hono<Env> {
  const app = new Hono<Env>();
  const { db } = deps;

  app.get("/health", (c) => c.json({ ok: true }));

  app.get("/v1/catalog", async (c) => {
    const caller = c.req.header("authorization") ? await owner(c, db) : null;
    const settings = await deps.registry.settings();
    const connectors = (await deps.registry.connectors())
      .map(({ connector }) => catalogEntry(connector, settings.markup))
      .filter((entry) => entry && (entry.status === "active" || caller?.role === "admin"));
    return c.json(
      { credit_unit_micros: settings.creditUnitMicros, portal_url: deps.portalUrl, connectors, meters: meterCatalog(settings) },
      200,
      { "cache-control": "no-store" },
    );
  });

  app.post("/v1/device/code", async (c) => {
    const wait = deps.limiter.take(`device-code:${clientIp(c, deps.trustProxy)}`, 10);
    if (wait > 0) return problem(429, "rate_limited", "Too many sign-in attempts.", { "retry-after": String(wait) });
    const body = (await c.req.json().catch(() => ({}))) as { client?: unknown };
    const client = typeof body.client === "string" && body.client.trim() ? body.client.trim() : "OctopusOn SEO app";
    const started = await startDevice(db, client.replace(/[^\w./ ()-]/g, ""));
    const verification = `${deps.portalUrl}/cihaz`;
    return c.json({
      device_code: started.deviceCode,
      user_code: started.userCode,
      verification_uri: verification,
      verification_uri_complete: `${verification}?code=${encodeURIComponent(started.userCode)}`,
      expires_in: started.expiresIn,
      interval: started.interval,
    });
  });

  app.post("/v1/device/token", async (c) => {
    const wait = deps.limiter.take(`device-token:${clientIp(c, deps.trustProxy)}`, 60);
    if (wait > 0) return c.json({ error: "slow_down" }, 400);
    const body = (await c.req.json().catch(() => ({}))) as { device_code?: unknown };
    if (typeof body.device_code !== "string" || body.device_code.length > 200) {
      return c.json({ error: "invalid_request" }, 400);
    }
    const poll = await pollDevice(db, deps.key, body.device_code);
    switch (poll.status) {
      case "approved":
        return c.json({ access_token: poll.token, token_type: "bearer" }, 200, { "cache-control": "no-store" });
      case "pending":
        return c.json({ error: "authorization_pending" }, 400);
      case "slow_down":
        return c.json({ error: "slow_down" }, 400);
      case "denied":
        return c.json({ error: "access_denied" }, 400);
      case "expired":
        return c.json({ error: "expired_token" }, 400);
      case "unknown":
        return c.json({ error: "invalid_grant" }, 400);
    }
  });

  // One-click sign-in: the program swaps the code the portal sent to its loopback address,
  // with the PKCE verifier only it holds.
  app.post("/v1/connect/token", async (c) => {
    const wait = deps.limiter.take(`connect:${clientIp(c, deps.trustProxy)}`, 30);
    if (wait > 0) return c.json({ error: "slow_down" }, 429, { "retry-after": String(wait) });
    const body = (await c.req.json().catch(() => ({}))) as { code?: unknown; code_verifier?: unknown };
    if (typeof body.code !== "string" || typeof body.code_verifier !== "string") {
      return c.json({ error: "invalid_request" }, 400);
    }
    const redeemed = await redeemConnectCode(db, body.code, body.code_verifier);
    if (!redeemed) return c.json({ error: "invalid_grant" }, 400);
    return c.json({ access_token: redeemed.token, token_type: "bearer" }, 200, { "cache-control": "no-store" });
  });

  const signedIn: MiddlewareHandler<Env> = async (c, next) => {
    const found = await owner(c, db);
    if (!found) return unauthorized();
    c.set("owner", found);
    await next();
  };
  app.use("/v1/me", signedIn);
  app.use("/v1/support", signedIn);
  app.use("/v1/logout", signedIn);
  app.use("/v1/meter", signedIn);
  app.use("/v1/p/*", signedIn);

  app.get("/v1/me", async (c) => {
    const signedIn = c.get("owner");
    const person = await accountOf(db, signedIn.userId);
    const settings = await deps.registry.settings();
    const current = await wallet(db, signedIn.userId);
    return c.json(
      {
        user_id: signedIn.userId,
        email: person?.email ?? null,
        name: person?.name ?? null,
        balance_micros: current.balanceMicros,
        held_micros: current.heldMicros,
        available_micros: current.availableMicros,
        credit_unit_micros: settings.creditUnitMicros,
        low_balance: current.availableMicros < settings.lowBalanceMicros,
        portal_url: deps.portalUrl,
      },
      200,
      { "cache-control": "no-store" },
    );
  });

  // A message to OctopusOn support from a program (a button beside an error), with the program,
  // page and error it was sent about; the admin panel's support page lists them.
  app.post("/v1/support", async (c) => {
    const { userId } = c.get("owner");
    const body = (await c.req.json().catch(() => null)) as { message?: unknown; context?: SupportContext } | null;
    const context = body?.context && typeof body.context === "object" ? body.context : null;
    const outcome = await recordSupportMessage(db, userId, { message: body?.message, context });
    if (outcome === "empty") return problem(400, "invalid_request", "Write a message or send the error it is about.");
    if (outcome === "too_many") return problem(429, "too_many_messages", "Too many messages this hour; try again later.");
    return c.json({ ok: true }, 201);
  });

  // Server work a web program did for the account (pages crawled), charged in batches: the
  // month's free units first, then credits. A batch sent twice is charged once.
  app.post("/v1/meter", async (c) => {
    const { userId, tokenId } = c.get("owner");
    const body = (await c.req.json().catch(() => null)) as { sku?: unknown; units?: unknown; idempotency_key?: unknown } | null;
    const sku = typeof body?.sku === "string" ? body.sku : "";
    const units = body?.units;
    const key = typeof body?.idempotency_key === "string" ? body.idempotency_key.trim() : "";
    if (!isMeterSku(sku) || typeof units !== "number" || !Number.isInteger(units) || units < 1 || units > 100_000 || !key || key.length > 200) {
      return problem(400, "invalid_request", "Send a known sku, 1–100000 whole units and an idempotency_key.");
    }
    const settings = await deps.registry.settings();
    try {
      const receipt = await meterUsage(db, userId, { sku, units, idempotencyKey: key, tokenId, app: appName(c) }, meterTerms(sku, settings));
      return c.json(
        {
          sku: receipt.sku,
          units: receipt.units,
          free_units: receipt.freeUnits,
          charged_micros: receipt.chargedMicros,
          free_left: receipt.freeLeft,
          available_micros: receipt.wallet.availableMicros,
          duplicate: receipt.duplicate,
        },
        200,
        { "cache-control": "no-store" },
      );
    } catch (error) {
      if (error instanceof InsufficientFunds) {
        return problem(402, "insufficient_credits", "Not enough OctopusOn SEO credits.", {}, { available_micros: error.availableMicros });
      }
      throw error;
    }
  });

  app.post("/v1/logout", async (c) => {
    const { userId, tokenId } = c.get("owner");
    await revokeToken(db, userId, tokenId);
    return c.body(null, 204);
  });

  app.on(["GET", "POST"], "/v1/p/:connector/*", (c) =>
    forward(deps, c.get("owner"), c.req.param("connector"), c.req.raw, appName(c)),
  );
  app.all("/v1/p/*", () => problem(405, "method_not_allowed", "Only GET and POST are forwarded."));

  app.notFound(() => problem(404, "not_found", "No such endpoint."));
  app.onError((error) => {
    deps.log({ level: "error", event: "unhandled", message: error instanceof Error ? error.message : String(error) });
    return problem(500, "internal", "Something went wrong.");
  });
  return app;
}
