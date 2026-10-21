import { describe, expect, it } from "vitest";

import { catalogEntry, type ConnectorInput, connectorSchema, secretNames } from "./connector.ts";
import { digest, masterKey, newDeviceCode, newToken, normalizeUserCode, open, sameDigest, seal } from "./crypto.ts";
import { parsePath, select, selectNumber } from "./jsonpath.ts";
import { creditsToMicros, microsToCredits, usdToMicros } from "./money.ts";
import { chargeFor, holdFor, settle } from "./pricing.ts";
import { providerBalance, testConnector } from "./probe.ts";
import { matchesPattern, normalizePath, routeAllowed, routePattern } from "./routes.ts";
import { SEED_CONNECTORS } from "./seeds.ts";

const KEY = Buffer.alloc(32, 7);

function connector(changes: Partial<ConnectorInput> = {}) {
  return connectorSchema.parse({
    id: "example",
    name: "Example",
    upstream: "https://api.example.com",
    auth: { type: "header", name: "X-API-KEY", template: "{secret}", secret: "api_key" },
    routes: [{ method: "POST", path: "/v1/search" }],
    pricing: { cost: { from: "fixed", usd: 0.002 }, hold_usd: 0.01 },
    terms: { confirmed: false },
    status: "draft",
    ...changes,
  });
}

describe("money", () => {
  it("rounds costs up to whole micro-dollars without float noise", () => {
    expect(usdToMicros(0.0006)).toBe(600);
    expect(usdToMicros(0.1 + 0.2)).toBe(300_000);
    expect(usdToMicros(0.0000001)).toBe(1);
    expect(() => usdToMicros(-1)).toThrow(RangeError);
    expect(() => usdToMicros(Number.NaN)).toThrow(RangeError);
  });

  it("converts credits", () => {
    expect(microsToCredits(2_025, 1000)).toBe(2.03);
    expect(creditsToMicros(2.5, 1000)).toBe(2_500);
  });
});

describe("jsonpath", () => {
  const body = { cost: 0.0025, tasks: [{ status_code: 20000, result: [{ items: [1, 2, 3] }] }], "odd-key": "4" };

  it("selects keys, indexes and wildcards", () => {
    expect(select(body, "$.cost")).toEqual([0.0025]);
    expect(select(body, "$.tasks[0].status_code")).toEqual([20000]);
    expect(select(body, "$.tasks[*].result[*].items[*]")).toEqual([1, 2, 3]);
    expect(select(body, "$['odd-key']")).toEqual(["4"]);
    expect(select(body, "$.missing.deeper")).toEqual([]);
    expect(select(body, "$.tasks[5]")).toEqual([]);
  });

  it("reads numbers, including numbers written as strings", () => {
    expect(selectNumber(body, "$.cost")).toBe(0.0025);
    expect(selectNumber(body, "$['odd-key']")).toBe(4);
    expect(selectNumber(body, "$.tasks")).toBeUndefined();
  });

  it("rejects syntax outside the subset", () => {
    expect(typeof parsePath("cost")).toBe("string");
    expect(typeof parsePath("$..cost")).toBe("string");
    expect(typeof parsePath("$.items[?(@.x)]")).toBe("string");
  });
});

describe("routes", () => {
  it("normalises paths and refuses ones that could escape", () => {
    expect(normalizePath("/v3/serp/google/organic/live/advanced/")).toBe("/v3/serp/google/organic/live/advanced");
    expect(normalizePath("/search.json")).toBe("/search.json");
    for (const attack of [
      "/v3/serp/../appendix/user_data",
      "/v3/./appendix",
      "/v3/%2e%2e/appendix",
      "/v3/%2E%2E/appendix",
      "/v3%2fappendix",
      "/v3\\appendix",
      "//evil.example/v3",
      "/v3/serp%00",
      "/v3/%E0%A4%A",
      "v3/serp",
      "/v3/serp/\u0001",
    ]) {
      expect(normalizePath(attack), attack).toBeNull();
    }
  });

  it("matches one segment with * and any number with **", () => {
    expect(matchesPattern("/v3/serp/google/organic/live/advanced", "/v3/serp/*/organic/live/*")).toBe(true);
    expect(matchesPattern("/v3/serp/google/organic/live", "/v3/serp/*/organic/live/*")).toBe(false);
    expect(matchesPattern("/v3/serp/google/x/organic/live/a", "/v3/serp/*/organic/live/*")).toBe(false);
    expect(matchesPattern("/v3/a/b/task_get/regular/1", "/v3/**/task_get/**")).toBe(true);
    expect(matchesPattern("/v3/task_get", "/v3/**/task_get/**")).toBe(true);
    expect(matchesPattern("/v2/wordstat/topRequests", "/v2/wordstat/top*")).toBe(true);
    expect(matchesPattern("/v2/wordstat/dynamics", "/v2/wordstat/top*")).toBe(false);
    expect(matchesPattern("/search.json", "/search.json")).toBe(true);
    expect(matchesPattern("/searchXjson", "/search.json")).toBe(false);
  });

  it("allows only listed methods and paths, and deny wins", () => {
    const routes = [{ method: "POST", path: "/v3/serp/**" }];
    expect(routeAllowed(routes, [], "post", "/v3/serp/google/organic/live/advanced")).toBe(true);
    expect(routeAllowed(routes, [], "GET", "/v3/serp/google/organic/live/advanced")).toBe(false);
    expect(routeAllowed(routes, ["/v3/**/task_get/**"], "POST", "/v3/serp/google/organic/task_get/advanced/1")).toBe(false);
    expect(routePattern(routes, "POST", "/v3/serp/x")).toBe("/v3/serp/**");
    expect(routePattern(routes, "GET", "/v3/serp/x")).toBe("?");
  });
});

describe("crypto", () => {
  it("seals and opens secrets, and refuses altered ones", () => {
    const sealed = seal(KEY, "login:password");
    expect(sealed.startsWith("v1.")).toBe(true);
    expect(sealed).not.toContain("password");
    expect(open(KEY, sealed)).toBe("login:password");
    expect(seal(KEY, "same")).not.toBe(seal(KEY, "same"));
    expect(() => open(Buffer.alloc(32, 8), sealed)).toThrow();
    const parts = sealed.split(".");
    parts[3] = `${parts[3]!.slice(0, -2)}AA`;
    expect(() => open(KEY, parts.join("."))).toThrow();
    expect(() => open(KEY, "v2.a.b.c")).toThrow("unknown secret format");
  });

  it("checks the master key length", () => {
    expect(() => masterKey(undefined)).toThrow();
    expect(() => masterKey(Buffer.alloc(16).toString("base64"))).toThrow();
    expect(masterKey(KEY.toString("base64"))).toEqual(KEY);
  });

  it("makes prefixed tokens stored as digests", () => {
    const token = newToken();
    expect(token.value.startsWith("oct_")).toBe(true);
    expect(token.value.length).toBeGreaterThan(40);
    expect(token.hash).toBe(digest(token.value));
    expect(token.prefix).toBe(token.value.slice(0, 10));
    expect(sameDigest(token.hash, digest(token.value))).toBe(true);
    expect(sameDigest(token.hash, digest("other"))).toBe(false);
    expect(sameDigest(token.hash, "ab")).toBe(false);
  });

  it("makes user codes people can type", () => {
    const { userCode, deviceCode } = newDeviceCode();
    expect(userCode).toMatch(/^[A-HJKMNP-Z2-9]{4}-[A-HJKMNP-Z2-9]{4}$/);
    expect(deviceCode.length).toBeGreaterThan(40);
    expect(normalizeUserCode(" abcd efgh ")).toBe("ABCD-EFGH");
    expect(normalizeUserCode("abcd-efgh")).toBe("ABCD-EFGH");
  });
});

describe("connector schema", () => {
  it("fills defaults", () => {
    const parsed = connector();
    expect(parsed.inject).toEqual([]);
    expect(parsed.deny).toEqual([]);
    expect(parsed.pricing.markup).toBeNull();
    expect(parsed.pricing.charge_on_error).toBe(false);
    expect(parsed.limits).toEqual({ rpm: 60, concurrency: 4, timeout_s: 60, max_body_kb: 256 });
  });

  it("can only be active once the terms are confirmed", () => {
    expect(() => connector({ status: "active" })).toThrow(/terms/);
    expect(connector({ status: "active", terms: { confirmed: true, date: "2026-10-18" } }).status).toBe("active");
  });

  it("refuses unsafe upstreams and patterns", () => {
    expect(() => connector({ upstream: "http://api.example.com" })).toThrow();
    expect(() => connector({ upstream: "https://api.example.com/v1" })).toThrow();
    expect(() => connector({ upstream: "https://user:pass@api.example.com" })).toThrow();
    expect(() => connector({ routes: [{ method: "POST", path: "/v1/../x" }] })).toThrow();
    expect(() => connector({ routes: [{ method: "POST", path: "v1/search" }] })).toThrow();
    expect(() => connector({ routes: [] })).toThrow();
    expect(() => connector({ routes: [{ method: "DELETE" as "GET", path: "/v1" }] })).toThrow();
    expect(() => connector({ id: "Bad_Id" })).toThrow();
    expect(() => connector({ auth: { type: "header", name: "X-API-KEY", template: "key", secret: "k" } })).toThrow();
    expect(() =>
      connector({ pricing: { cost: { from: "json", path: "$..cost" }, hold_usd: 0.01 } }),
    ).toThrow(/JSONPath/);
  });

  it("needs a route for the test request's method", () => {
    expect(() => connector({ test: { method: "GET", path: "/v1/search" } })).toThrow(/test/);
  });

  it("lists the secrets it needs", () => {
    expect(secretNames(connector())).toEqual(["api_key"]);
    expect(
      secretNames(connector({ inject: [{ type: "json_field", field: "folderId", secret: "folder_id" }] })),
    ).toEqual(["api_key", "folder_id"]);
    expect(secretNames(connector({ auth: { type: "none" } }))).toEqual([]);
  });

  it("shows only usable connectors in the catalog, with the price multiplier", () => {
    expect(catalogEntry(connector(), 0.35)).toBeNull();
    const entry = catalogEntry(connector({ status: "test" }), 0.35);
    expect(entry?.price_multiplier).toBeCloseTo(1.35);
    expect(entry).not.toHaveProperty("auth");
    expect(entry).not.toHaveProperty("pricing");
    const own = catalogEntry(
      connector({ status: "test", pricing: { cost: { from: "fixed", usd: 0.002 }, hold_usd: 0.01, markup: 0.5 } }),
      0.35,
    );
    expect(own?.price_multiplier).toBeCloseTo(1.5);
  });

  it("seeds valid drafts whose routes cover what the apps call", () => {
    const seeds = SEED_CONNECTORS.map((seed) => connectorSchema.parse(seed));
    expect(seeds.map((seed) => seed.id)).toEqual(["dataforseo", "serpapi", "serper", "yandex-search", "wordstat"]);
    for (const seed of seeds) {
      expect(seed.status).toBe("draft");
      expect(seed.terms.confirmed).toBe(false);
    }
    const byId = Object.fromEntries(seeds.map((seed) => [seed.id, seed]));
    const allowed = (id: string, method: string, path: string) =>
      routeAllowed(byId[id]!.routes, byId[id]!.deny, method, path);
    for (const path of [
      "/v3/serp/google/organic/live/advanced",
      "/v3/serp/bing/organic/live/advanced",
      "/v3/serp/google/autocomplete/live/advanced",
      "/v3/keywords_data/google_ads/search_volume/live",
      "/v3/keywords_data/google_ads/keywords_for_keywords/live",
      "/v3/dataforseo_labs/google/related_keywords/live",
      "/v3/dataforseo_labs/google/search_intent/live",
      "/v3/backlinks/summary/live",
      "/v3/backlinks/backlinks/live",
      "/v3/backlinks/referring_domains/live",
      "/v3/backlinks/timeseries_new_lost_summary/live",
      "/v3/backlinks/bulk_ranks/live",
      "/v3/serp/google/ai_mode/live/advanced",
      "/v3/ai_optimization/chat_gpt/llm_responses/live",
      "/v3/ai_optimization/perplexity/llm_responses/live",
      "/v3/ai_optimization/llm_mentions/target_metrics/live",
      "/v3/ai_optimization/llm_mentions/search_mentions/live",
    ]) {
      expect(allowed("dataforseo", "POST", path), path).toBe(true);
    }
    // AI answers only live: no task queues or scraper downloads.
    expect(allowed("dataforseo", "POST", "/v3/ai_optimization/chat_gpt/llm_responses/task_post")).toBe(false);
    expect(allowed("dataforseo", "POST", "/v3/ai_optimization/chat_gpt/llm_scraper/live/html")).toBe(false);
    // Only live reports: no task queues or account data of the backlink index.
    expect(allowed("dataforseo", "POST", "/v3/backlinks/index")).toBe(false);
    expect(allowed("dataforseo", "POST", "/v3/backlinks/backlinks/task_post")).toBe(false);
    for (const path of [
      "/v3/appendix/user_data",
      "/v3/serp/google/organic/task_get/advanced/0001",
      "/v3/serp/google/organic/task_post",
      "/v3/serp/google/organic/tasks_ready",
    ]) {
      expect(allowed("dataforseo", "POST", path), path).toBe(false);
      expect(allowed("dataforseo", "GET", path), path).toBe(false);
    }
    expect(allowed("serpapi", "GET", "/search.json")).toBe(true);
    expect(allowed("serpapi", "GET", "/searches/abc.json")).toBe(false);
    expect(allowed("serpapi", "GET", "/account.json")).toBe(false);
    expect(allowed("serper", "POST", "/search")).toBe(true);
    expect(allowed("yandex-search", "POST", "/v2/web/search")).toBe(true);
    expect(allowed("wordstat", "POST", "/v2/wordstat/topRequests")).toBe(true);
    expect(allowed("wordstat", "POST", "/v2/web/search")).toBe(false);
    expect(byId.serpapi!.strip_query).toEqual(["api_key"]);
  });
});

describe("pricing", () => {
  it("adds the markup and rounds up", () => {
    expect(chargeFor(600, 0.35)).toBe(810);
    expect(chargeFor(1, 0.35)).toBe(2);
    expect(chargeFor(0, 0.35)).toBe(0);
    expect(chargeFor(100, 0.35, 1000)).toBe(1000);
  });

  it("reads DataForSEO costs and errors inside 200 responses", () => {
    const dataforseo = connectorSchema.parse(SEED_CONNECTORS[0]);
    const ok = { cost: 0.002, tasks: [{ status_code: 20000 }] };
    expect(settle(dataforseo, 0.35, 200, ok)).toEqual({ error: false, upstreamMicros: 2000, chargeMicros: 2700 });
    const failed = { cost: 0.002, tasks: [{ status_code: 40501 }] };
    expect(settle(dataforseo, 0.35, 200, failed)).toEqual({ error: true, upstreamMicros: 2000, chargeMicros: 0 });
    expect(settle(dataforseo, 0.35, 500, null)).toEqual({ error: true, upstreamMicros: 0, chargeMicros: 0 });
    expect(holdFor(dataforseo)).toBe(500_000);
    expect(holdFor(dataforseo, "POST", "/v3/backlinks/summary/live")).toBe(500_000);
    expect(holdFor(dataforseo, "post", "/v3/ai_optimization/llm_mentions/search_mentions/live")).toBe(2_000_000);
  });

  it("prices Serper credits, SerpApi searches and counted items", () => {
    const serper = connectorSchema.parse(SEED_CONNECTORS[2]);
    expect(settle(serper, 0.35, 200, { credits: 2 })).toEqual({ error: false, upstreamMicros: 2000, chargeMicros: 2700 });
    const serpapi = connectorSchema.parse(SEED_CONNECTORS[1]);
    expect(settle(serpapi, 0.35, 200, { search_metadata: { status: "Success" } }).chargeMicros).toBe(20_250);
    expect(settle(serpapi, 0.35, 200, { search_metadata: { status: "Error" } })).toEqual({
      error: true,
      upstreamMicros: 0,
      chargeMicros: 0,
    });
    const items = connector({
      pricing: { cost: { from: "per_item", path: "$.items[*]", usd_per_item: 0.001, base_usd: 0.002 }, hold_usd: 0.1 },
    });
    expect(settle(items, 0, 200, { items: [1, 2, 3] }).chargeMicros).toBe(5000);
  });

  it("charges errors only when told to", () => {
    const charging = connector({
      pricing: { cost: { from: "json", path: "$.cost" }, hold_usd: 0.1, charge_on_error: true },
    });
    expect(settle(charging, 0, 400, { cost: 0.001 }).chargeMicros).toBe(1000);
    expect(settle(charging, 0, 200, { cost: -5 }).chargeMicros).toBe(0);
  });
});

describe("admin probes", () => {
  it("sends the test request with our key and masks it in the preview", async () => {
    const serpapi = connectorSchema.parse(SEED_CONNECTORS[1]);
    const seen: Request[] = [];
    const result = await testConnector(serpapi, { api_key: "serpapi-key-123456" }, 0.35, async (request) => {
      seen.push(request);
      return new Response(JSON.stringify({ search_metadata: { status: "Success" }, echo: request.url }), {
        headers: { "content-type": "application/json" },
      });
    });
    const url = new URL(seen[0]!.url);
    expect(url.pathname).toBe("/search.json");
    expect(url.searchParams.get("q")).toBe("kahve");
    expect(url.searchParams.get("api_key")).toBe("serpapi-key-123456");
    expect(result).toMatchObject({ status: 200, error: false, upstreamMicros: 15_000, chargeMicros: 20_250 });
    expect(result.preview).not.toContain("serpapi-key-123456");
  });

  it("reads our provider balance", async () => {
    const dataforseo = connectorSchema.parse(SEED_CONNECTORS[0]);
    const result = await providerBalance(dataforseo, { credentials: "login:password-123" }, async (request) => {
      expect(new URL(request.url).pathname).toBe("/v3/appendix/user_data");
      expect(request.headers.get("authorization")).toBe(`Basic ${Buffer.from("login:password-123").toString("base64")}`);
      return Response.json({ cost: 0, tasks: [{ status_code: 20000, result: [{ money: { balance: 48.25 } }] }] });
    });
    expect(result.value).toBe(48.25);
    expect(dataforseo.balance?.warn_below).toBe(20);
  });

  it("writes injected fields into test bodies", async () => {
    const wordstat = connectorSchema.parse({
      ...SEED_CONNECTORS[4],
      test: { method: "POST", path: "/v2/wordstat/topRequests", body: { phrase: "kahve" } },
    });
    await testConnector(wordstat, { api_key: "yc-key-123456", folder_id: "b1g" }, 0.35, async (request) => {
      expect(await request.json()).toEqual({ phrase: "kahve", folderId: "b1g" });
      return Response.json({});
    });
  });
});
