import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  applySeedCacheRules,
  applySeedRoutes,
  cachedAnswer,
  cacheKey,
  cacheReport,
  cacheRule,
  cacheSeconds,
  canonical,
  discounted,
  hasCachedAnswer,
  keepAnswer,
  noteCacheHit,
  purgeAnswers,
} from "./index.ts";
import { type ConnectorInput, connectorSchema } from "./connector.ts";
import type { Db } from "./db/client.ts";
import { getConnector, saveConnector } from "./plugins.ts";
import { SEED_CONNECTORS } from "./seeds.ts";
import { DEFAULT_SETTINGS } from "./settings.ts";
import { testDb } from "./testing.ts";

const dataforseo = connectorSchema.parse(SEED_CONNECTORS.find((seed) => seed.id === "dataforseo"));
const yandex = connectorSchema.parse(SEED_CONNECTORS.find((seed) => seed.id === "yandex-search"));
const serpapi = connectorSchema.parse(SEED_CONNECTORS.find((seed) => seed.id === "serpapi"));

function json(value: unknown): Uint8Array {
  return Buffer.from(JSON.stringify(value));
}

/** A concrete path of a route pattern. */
function example(pattern: string): string {
  return pattern.replaceAll("**", "x").replaceAll("*", "x");
}

describe("rules", () => {
  it("every seed route has its own cache decision", () => {
    for (const seed of SEED_CONNECTORS) {
      const connector = connectorSchema.parse(seed);
      for (const route of connector.routes) {
        expect(cacheRule(connector, route.method, example(route.path)), `${connector.id} ${route.path}`).toBeDefined();
      }
    }
  });

  it("keeps answers as long as their data lasts", () => {
    expect(cacheSeconds(dataforseo, "POST", "/v3/backlinks/summary/live")).toBe(24 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/backlinks/timeseries_new_lost_summary/live")).toBe(72 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/dataforseo_labs/google/ranked_keywords/live")).toBe(168 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/keywords_data/google_ads/search_volume/live")).toBe(720 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/serp/google/organic/live/advanced")).toBe(12 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/ai_optimization/chat_gpt/llm_responses/live")).toBe(24 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/serp/google/maps/live/advanced")).toBe(12 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/business_data/google/my_business_info/live")).toBe(168 * 3600);
    expect(cacheSeconds(dataforseo, "POST", "/v3/business_data/business_listings/search/live")).toBe(168 * 3600);
    expect(cacheSeconds(dataforseo, "GET", "/v3/dataforseo_labs/categories")).toBe(720 * 3600);
    expect(cacheSeconds({ ...dataforseo, cache: { ...dataforseo.cache, enabled: false } }, "POST", "/v3/backlinks/summary/live")).toBe(0);
    const plain = connectorSchema.parse({ ...SEED_CONNECTORS[0], cache: undefined } as ConnectorInput);
    expect(plain.cache.rules).toEqual([]);
    expect(cacheSeconds(plain, "POST", "/v3/backlinks/summary/live")).toBe(0);
  });

  it("sells kept answers at a discount", () => {
    expect(DEFAULT_SETTINGS.cacheEnabled).toBe(true);
    expect(DEFAULT_SETTINGS.cacheDiscountPct).toBe(70);
    expect(discounted(2700, 70)).toBe(810);
    expect(discounted(1001, 70)).toBe(301);
    expect(discounted(0, 70)).toBe(0);
    expect(discounted(2700, 0)).toBe(2700);
  });
});

describe("keys", () => {
  const path = "/v3/backlinks/summary/live";
  const search = new URLSearchParams();

  it("are the same for the same question, whoever asks and however it is written", () => {
    const a = cacheKey(dataforseo, "POST", path, search, json([{ target: "Example.com ", limit: 10, tag: "user-1" }]));
    const b = cacheKey(dataforseo, "post", path, search, json([{ limit: 10, target: "example.com", tag: "user-2" }]));
    expect(a).toBe(b);
    expect(a).toMatch(/^[0-9a-f]{64}$/);
    // Another question, path or connector is another key.
    expect(cacheKey(dataforseo, "POST", path, search, json([{ target: "example.com", limit: 20 }]))).not.toBe(a);
    expect(cacheKey(dataforseo, "POST", "/v3/backlinks/anchors/live", search, json([{ target: "example.com", limit: 10 }]))).not.toBe(a);
    // Keywords compare in lower case with single spaces; other text stays as written.
    expect(canonical({ keywords: ["  Spor  Ayakkabı "], language_code: "TR" }, new Set(["keywords"]))).toEqual({
      keywords: ["spor ayakkabı"],
      language_code: "TR",
    });
  });

  it("leave out our credentials and the apps' own keys", () => {
    // Yandex: the key is taken before the folder id is written into the body.
    const body = json({ query: { queryText: "Kahve " }, page: 0 });
    expect(cacheKey(yandex, "POST", "/v2/web/search", search, body)).toBe(
      cacheKey(yandex, "POST", "/v2/web/search", search, json({ page: 0, query: { queryText: "kahve" } })),
    );
    // SerpApi: the apps' api_key parameter is not part of the question.
    const one = cacheKey(serpapi, "GET", "/search.json", new URLSearchParams({ q: "Kahve", gl: "tr", api_key: "a" }), null);
    const two = cacheKey(serpapi, "GET", "/search.json", new URLSearchParams({ gl: "tr", q: "kahve", api_key: "b" }), null);
    expect(one).toBe(two);
    expect(cacheKey(serpapi, "GET", "/search.json", new URLSearchParams({ q: "kahve", gl: "de" }), null)).not.toBe(one);
  });
});

describe("storage", () => {
  let db: Db;
  let close: () => Promise<void>;
  beforeEach(async () => {
    ({ db, close } = await testDb());
  });
  afterEach(async () => close());

  const now = new Date("2026-10-22T10:00:00Z");
  const later = (hours: number) => new Date(now.getTime() + hours * 3600_000);
  const answer = (key: string, size = 10) => ({
    key,
    connectorId: "dataforseo",
    route: "/v3/backlinks/*/live",
    status: 200,
    contentType: "application/json",
    body: json({ status_code: 20000, cost: 0.02, tasks: [{ result: ["x".repeat(size)] }] }),
    upstreamMicros: 20_000,
    seconds: 24 * 3600,
    now,
    maxBytes: 4096 * 1024,
  });

  it("keeps answers until they expire and counts their use", async () => {
    expect(await keepAnswer(db, answer("k1"))).toBe(true);
    const kept = await cachedAnswer(db, "k1", later(1));
    expect(kept?.upstreamMicros).toBe(20_000);
    expect(JSON.parse(Buffer.from(kept!.body).toString("utf8")).cost).toBe(0.02);
    expect(await hasCachedAnswer(db, "k1", later(23))).toMatchObject({ upstreamMicros: 20_000 });
    expect(await cachedAnswer(db, "k1", later(25))).toBeNull();
    await noteCacheHit(db, "k1", later(1));
    await noteCacheHit(db, "k1", later(2));
    const report = await cacheReport(db, now);
    expect(report.kept).toEqual([{ connectorId: "dataforseo", entries: 1, bytes: expect.any(Number) }]);

    // A newer answer replaces it; one too large or not kept is not written.
    expect(await keepAnswer(db, { ...answer("k1"), upstreamMicros: 30_000, now: later(5) })).toBe(true);
    expect((await cachedAnswer(db, "k1", later(26)))?.upstreamMicros).toBe(30_000);
    expect(await keepAnswer(db, { ...answer("k2", 5000), maxBytes: 1000 })).toBe(false);
    expect(await keepAnswer(db, { ...answer("k3"), seconds: 0 })).toBe(false);
  });

  it("forgets expired answers, then the oldest beyond the space allowed", async () => {
    await keepAnswer(db, { ...answer("old"), now: later(-30), seconds: 3600 });
    await keepAnswer(db, { ...answer("a", 20_000), now: later(-3) });
    await keepAnswer(db, { ...answer("b", 20_000), now: later(-2) });
    await keepAnswer(db, { ...answer("c", 20_000), now: later(-1) });
    expect(await purgeAnswers(db, now, 1 << 30)).toEqual({ expired: 1, trimmed: 0 });
    const total = (await cacheReport(db, now)).kept[0]!.bytes;
    expect(await purgeAnswers(db, now, total - 1)).toEqual({ expired: 0, trimmed: 1 });
    expect(await cachedAnswer(db, "a", now)).toBeNull();
    expect(await cachedAnswer(db, "c", now)).not.toBeNull();
  });

  it("gives connectors in the database the seeds' rules, keeping a switched-off cache off", async () => {
    const seed = SEED_CONNECTORS.find((connector) => connector.id === "dataforseo")!;
    await saveConnector(db, { ...seed, cache: undefined } as ConnectorInput, "test");
    await saveConnector(db, { ...SEED_CONNECTORS.find((connector) => connector.id === "serper")!, cache: { enabled: false, rules: [{ path: "/search", hours: 1 }] } }, "test");
    expect(await applySeedCacheRules(db, "test")).toEqual(["dataforseo", "serper"]);
    expect((await getConnector(db, "dataforseo"))?.cache.rules.length).toBeGreaterThan(5);
    const serper = await getConnector(db, "serper");
    expect(serper?.cache.enabled).toBe(false);
    expect(serper?.cache.rules).toEqual([
      { path: "/search", hours: 12 },
      { path: "/autocomplete", hours: 168 },
    ]);
    expect(await applySeedCacheRules(db, "test")).toEqual([]);
  });

  it("adds the seeds' new routes to connectors in the database and keeps their own", async () => {
    const seed = SEED_CONNECTORS.find((connector) => connector.id === "dataforseo")!;
    const own = { method: "POST" as const, path: "/v3/on_page/instant_pages" };
    await saveConnector(db, { ...seed, routes: [...seed.routes.filter((route) => !route.path.includes("business_data")), own] }, "test");
    const changed = await applySeedRoutes(db, "test");
    expect(changed).toEqual([
      { id: "dataforseo", added: ["POST /v3/business_data/google/my_business_info/live", "POST /v3/business_data/business_listings/search/live"] },
    ]);
    const routes = (await getConnector(db, "dataforseo"))!.routes.map((route) => route.path);
    expect(routes).toContain("/v3/on_page/instant_pages");
    expect(routes.filter((path) => path.startsWith("/v3/business_data"))).toHaveLength(2);
    expect(await applySeedRoutes(db, "test")).toEqual([]);
  });
});
