// Providers' answers kept for every account (ADR 0079): the same question asked again within
// its rule's time is answered from the cache, without paying the provider, and the user pays
// a discounted price. Every program on OctopusOn SEO credits gets this through the gateway.
//
// A question is found by a hash of its connector, method, path, query and body, with JSON
// fields sorted, fields that change between identical questions (a client's tag) left out,
// and the connector's `normalize` fields compared trimmed and in lower case. The hash is taken
// before our credentials are written into the body. Who asked is never recorded.

import { createHash } from "node:crypto";
import { gunzipSync, gzipSync } from "node:zlib";

import { and, asc, desc, eq, gt, gte, lte, sql } from "drizzle-orm";

import type { Connector } from "./connector.ts";
import type { Db } from "./db/client.ts";
import { answerCache, usageEvents } from "./db/schema.ts";
import { matchesPattern } from "./routes.ts";

/** Fields that differ between identical questions: a client's own label and callbacks. */
const VOLATILE = new Set(["tag", "postback_url", "pingback_url", "postback_data"]);

/** The rule that decides how long a request's answer is kept, if any. */
export function cacheRule(connector: Connector, method: string, path: string) {
  const wanted = method.toUpperCase();
  return connector.cache.rules.find((rule) => (!rule.method || rule.method === wanted) && matchesPattern(path, rule.path));
}

/** How long a request's answer is kept, in seconds; 0 when it is not. */
export function cacheSeconds(connector: Connector, method: string, path: string): number {
  if (!connector.cache.enabled) return 0;
  return Math.round((cacheRule(connector, method, path)?.hours ?? 0) * 3600);
}

/** A string compared trimmed, in lower case, with single spaces. */
function cleaned(text: string): string {
  return text.trim().toLowerCase().replace(/\s+/g, " ");
}

/** Every string inside a value cleaned. */
function cleanAll(value: unknown): unknown {
  if (typeof value === "string") return cleaned(value);
  if (Array.isArray(value)) return value.map(cleanAll);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value as Record<string, unknown>).map(([key, inner]) => [key, cleanAll(inner)]));
  }
  return value;
}

/** A JSON value with object keys sorted, volatile fields left out and `normalize` fields cleaned. */
export function canonical(value: unknown, normalize: ReadonlySet<string>): unknown {
  if (Array.isArray(value)) return value.map((item) => canonical(item, normalize));
  if (!value || typeof value !== "object") return value;
  const sorted: Record<string, unknown> = {};
  for (const key of Object.keys(value as Record<string, unknown>).sort()) {
    if (VOLATILE.has(key)) continue;
    const inner = (value as Record<string, unknown>)[key];
    sorted[key] = normalize.has(key) ? cleanAll(canonical(inner, normalize)) : canonical(inner, normalize);
  }
  return sorted;
}

/** The cache key of a request: the same question gives the same key, whoever asks. */
export function cacheKey(
  connector: Connector,
  method: string,
  path: string,
  search: URLSearchParams,
  body: Uint8Array | null,
): string {
  const normalize = new Set(connector.cache.normalize);
  const query = new URLSearchParams(search);
  for (const name of connector.strip_query) query.delete(name);
  if (connector.auth.type === "query") query.delete(connector.auth.param);
  const pairs = [...query.entries()]
    .map(([name, value]): [string, string] => [name, normalize.has(name) ? cleaned(value) : value])
    .sort(([a, x], [b, y]) => (a === b ? x.localeCompare(y) : a.localeCompare(b)));
  let content = "";
  if (body && body.byteLength > 0) {
    const text = Buffer.from(body).toString("utf8");
    try {
      content = JSON.stringify(canonical(JSON.parse(text), normalize));
    } catch {
      content = `raw:${createHash("sha256").update(body).digest("hex")}`;
    }
  }
  return createHash("sha256")
    .update([connector.id, method.toUpperCase(), path, new URLSearchParams(pairs).toString(), content].join("\n"))
    .digest("hex");
}

/** The price of an answer served from the cache: the full price less the discount, rounded up. */
export function discounted(fullMicros: number, discountPct: number): number {
  if (fullMicros <= 0) return 0;
  return Math.ceil((fullMicros * (100 - discountPct)) / 100);
}

/** A kept answer. */
export interface CachedAnswer {
  status: number;
  contentType: string | null;
  body: Uint8Array;
  upstreamMicros: number;
  createdAt: Date;
  expiresAt: Date;
}

/** The kept answer of a question, while it is fresh at `now`. */
export async function cachedAnswer(db: Db, key: string, now: Date): Promise<CachedAnswer | null> {
  const [row] = await db
    .select()
    .from(answerCache)
    .where(and(eq(answerCache.key, key), gt(answerCache.expiresAt, now)));
  if (!row) return null;
  return {
    status: row.status,
    contentType: row.contentType,
    body: gunzipSync(row.body),
    upstreamMicros: row.upstreamMicros,
    createdAt: row.createdAt,
    expiresAt: row.expiresAt,
  };
}

/** Whether a fresh answer is kept for a question (asking the price first). */
export async function hasCachedAnswer(db: Db, key: string, now: Date): Promise<{ upstreamMicros: number; createdAt: Date } | null> {
  const [row] = await db
    .select({ upstreamMicros: answerCache.upstreamMicros, createdAt: answerCache.createdAt })
    .from(answerCache)
    .where(and(eq(answerCache.key, key), gt(answerCache.expiresAt, now)));
  return row ?? null;
}

/** Counts a kept answer's use. */
export async function noteCacheHit(db: Db, key: string, now: Date): Promise<void> {
  await db
    .update(answerCache)
    .set({ hits: sql`${answerCache.hits} + 1`, lastHitAt: now })
    .where(eq(answerCache.key, key));
}

/** An answer to keep. */
export interface AnswerToKeep {
  key: string;
  connectorId: string;
  route: string;
  status: number;
  contentType: string | null;
  body: Uint8Array;
  upstreamMicros: number;
  seconds: number;
  now: Date;
  /** Largest answer kept, before compression. */
  maxBytes: number;
}

/** Keeps an answer (replacing an older one); false when it is too large or not kept. */
export async function keepAnswer(db: Db, answer: AnswerToKeep): Promise<boolean> {
  if (answer.seconds <= 0 || answer.body.byteLength > answer.maxBytes) return false;
  const body = gzipSync(answer.body);
  const row = {
    key: answer.key,
    connectorId: answer.connectorId,
    route: answer.route,
    status: answer.status,
    contentType: answer.contentType,
    body,
    upstreamMicros: answer.upstreamMicros,
    size: body.byteLength,
    hits: 0,
    createdAt: answer.now,
    expiresAt: new Date(answer.now.getTime() + answer.seconds * 1000),
    lastHitAt: null,
  };
  await db
    .insert(answerCache)
    .values(row)
    .onConflictDoUpdate({
      target: answerCache.key,
      set: {
        connectorId: row.connectorId,
        route: row.route,
        status: row.status,
        contentType: row.contentType,
        body: row.body,
        upstreamMicros: row.upstreamMicros,
        size: row.size,
        hits: 0,
        createdAt: row.createdAt,
        expiresAt: row.expiresAt,
        lastHitAt: null,
      },
    });
  return true;
}

/** Forgets answers that expired, then the oldest ones beyond `maxTotalBytes`; how many went. */
export async function purgeAnswers(db: Db, now: Date, maxTotalBytes: number): Promise<{ expired: number; trimmed: number }> {
  const expired = await db.delete(answerCache).where(lte(answerCache.expiresAt, now)).returning({ key: answerCache.key });
  const [total] = await db.select({ bytes: sql<string>`coalesce(sum(${answerCache.size}), 0)` }).from(answerCache);
  let excess = Number(total?.bytes ?? 0) - maxTotalBytes;
  let trimmed = 0;
  while (excess > 0) {
    const oldest = await db
      .select({ key: answerCache.key, size: answerCache.size })
      .from(answerCache)
      .orderBy(asc(answerCache.createdAt))
      .limit(200);
    if (oldest.length === 0) break;
    const going: string[] = [];
    for (const row of oldest) {
      if (excess <= 0) break;
      going.push(row.key);
      excess -= row.size;
    }
    for (const key of going) await db.delete(answerCache).where(eq(answerCache.key, key));
    trimmed += going.length;
  }
  return { expired: expired.length, trimmed };
}

/** What the cache did since a moment, per connector: for the admin panel. */
export async function cacheReport(db: Db, since: Date) {
  const usage = await db
    .select({
      connectorId: usageEvents.connectorId,
      requests: sql<string>`count(*)`,
      hits: sql<string>`count(*) filter (where ${usageEvents.cached})`,
      saved: sql<string>`coalesce(sum(${usageEvents.savedMicros}), 0)`,
      chargedOnHits: sql<string>`coalesce(sum(${usageEvents.chargedMicros}) filter (where ${usageEvents.cached}), 0)`,
    })
    .from(usageEvents)
    .where(gte(usageEvents.createdAt, since))
    .groupBy(usageEvents.connectorId)
    .orderBy(usageEvents.connectorId);
  const kept = await db
    .select({
      connectorId: answerCache.connectorId,
      entries: sql<string>`count(*)`,
      bytes: sql<string>`coalesce(sum(${answerCache.size}), 0)`,
    })
    .from(answerCache)
    .groupBy(answerCache.connectorId)
    .orderBy(desc(sql`2`));
  return {
    connectors: usage.map((row) => ({
      connectorId: row.connectorId,
      requests: Number(row.requests),
      hits: Number(row.hits),
      savedMicros: Number(row.saved),
      chargedOnHitsMicros: Number(row.chargedOnHits),
    })),
    kept: kept.map((row) => ({ connectorId: row.connectorId, entries: Number(row.entries), bytes: Number(row.bytes) })),
  };
}
