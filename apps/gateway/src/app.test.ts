import {
  approveDevice,
  createConnectCode,
  parseConnectRequest,
  type ConnectorInput,
  createToken,
  credit,
  type Db,
  getConnector,
  saveConnector,
  SEED_CONNECTORS,
  setConnectorSecret,
  setTokenCap,
  updateSettings,
  wallet,
  schema,
} from "@octopusseo/cloud-core";
import { addUser, eq, testDb } from "@octopusseo/cloud-core/testing";
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";

import { createApp } from "./app.ts";
import { Concurrency, RateLimiter } from "./limits.ts";
import { AVAILABLE_HEADER, CHARGE_HEADER } from "./proxy.ts";
import { Registry } from "./registry.ts";

const KEY = Buffer.alloc(32, 9);
const SECRETS: Record<string, Record<string, string>> = {
  dataforseo: { credentials: "octopus@example.com:dfs-password-123" },
  serpapi: { api_key: "serpapi-secret-key-123" },
  serper: { api_key: "serper-secret-key-123" },
  "yandex-search": { api_key: "yc-secret-api-key", folder_id: "b1gfolder123" },
  wordstat: { api_key: "yc-secret-api-key", folder_id: "b1gfolder123" },
};

type Handler = (request: Request) => Promise<Response> | Response;
let db: Db;
let close: () => Promise<void>;
let admin: string;
let userId: string;
let token: string;
let tokenId: string;
let sent: Request[];
let handler: Handler;
let logs: Record<string, unknown>[];
let app: ReturnType<typeof createApp>;

function json(value: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json", ...headers } });
}

async function activate(id: string, changes: Partial<ConnectorInput> = {}): Promise<void> {
  const seed = SEED_CONNECTORS.find((connector) => connector.id === id)!;
  await saveConnector(
    db,
    { ...seed, ...changes, status: changes.status ?? "active", terms: { confirmed: true, date: "2026-10-18", note: "test" } },
    admin,
  );
  for (const [name, value] of Object.entries(SECRETS[id] ?? {})) await setConnectorSecret(db, KEY, id, name, value, admin);
}

function call(path: string, init: Omit<RequestInit, "headers"> & { headers?: Record<string, string> } = {}, bearer: string | null = token) {
  const headers: Record<string, string> = { ...init.headers };
  if (bearer) headers.authorization = `Bearer ${bearer}`;
  return app.request(path, { ...init, headers });
}

const SERP_BODY = JSON.stringify([{ keyword: "kahve", location_code: 2792, language_code: "tr" }]);

beforeAll(async () => {
  ({ db, close } = await testDb());
  admin = await addUser(db, "admin@example.com", "admin");
  userId = await addUser(db, "user@example.com");
  ({ value: token, id: tokenId } = await createToken(db, userId, "MacBook", "keyword-miner/0.1.0"));
  await credit(db, userId, "purchase", 5_000_000);
  await activate("dataforseo");
  await activate("serpapi");
  await activate("serper", { limits: { rpm: 3, concurrency: 4, timeout_s: 60, max_body_kb: 16 } });
  await activate("yandex-search");
  await activate("wordstat", { status: "test" });
});
afterAll(async () => close());

beforeEach(() => {
  sent = [];
  logs = [];
  handler = () => json({});
  app = createApp({
    db,
    key: KEY,
    portalUrl: "https://my.octopuson.com",
    trustProxy: true,
    registry: new Registry(db, KEY, 0),
    fetch: async (request) => {
      sent.push(request.clone());
      return handler(request);
    },
    limiter: new RateLimiter(),
    concurrency: new Concurrency(),
    log: (event) => logs.push(event),
  });
});

describe("authentication", () => {
  it("refuses missing, wrong and revoked tokens", async () => {
    expect((await call("/v1/p/serper/search", { method: "POST" }, null)).status).toBe(401);
    const wrong = await call("/v1/p/serper/search", { method: "POST" }, "oct_wrong");
    expect(wrong.status).toBe(401);
    expect(wrong.headers.get("www-authenticate")).toContain("Bearer");
    expect((await call("/v1/me", {}, null)).status).toBe(401);
    expect(sent).toHaveLength(0);
  });
});

describe("catalog", () => {
  it("lists active connectors without credentials or pricing internals", async () => {
    const response = await call("/v1/catalog", {}, null);
    const body = (await response.json()) as { connectors: { id: string; price_multiplier: number }[]; credit_unit_micros: number };
    expect(body.connectors.map((entry) => entry.id)).toEqual(["dataforseo", "serpapi", "serper", "yandex-search"]);
    expect(body.connectors[0]?.price_multiplier).toBeCloseTo(1.35);
    expect(body.credit_unit_micros).toBe(1000);
    expect(JSON.stringify(body)).not.toContain("secret");
    expect(JSON.stringify(body)).not.toContain("credentials");
  });

  it("shows connectors in test to admins only", async () => {
    const { value } = await createToken(db, admin, "admin");
    const body = (await (await call("/v1/catalog", {}, value)).json()) as { connectors: { id: string }[] };
    expect(body.connectors.map((entry) => entry.id)).toContain("wordstat");
    const forUser = await call("/v1/p/wordstat/v2/wordstat/topRequests", { method: "POST", body: "{}" });
    expect(forUser.status).toBe(503);
  });
});

describe("passthrough", () => {
  it("forwards DataForSEO with our credentials and charges the reported cost plus markup", async () => {
    handler = () => json({ cost: 0.002, tasks: [{ status_code: 20000, result: [{ items: [] }] }] });
    const before = await wallet(db, userId);
    const response = await call("/v1/p/dataforseo/v3/serp/google/organic/live/advanced", {
      method: "POST",
      body: SERP_BODY,
      headers: { "content-type": "application/json", cookie: "session=1", "x-octopus-app": "keyword-miner/0.1.0 macOS" },
    });
    expect(response.status).toBe(200);
    expect(response.headers.get(CHARGE_HEADER)).toBe("0.002700");
    expect(Number(response.headers.get(AVAILABLE_HEADER))).toBeCloseTo((before.availableMicros - 2700) / 1e6, 6);
    expect(((await response.json()) as { cost: number }).cost).toBe(0.002);

    const upstream = sent[0]!;
    expect(upstream.url).toBe("https://api.dataforseo.com/v3/serp/google/organic/live/advanced");
    expect(upstream.headers.get("authorization")).toBe(
      `Basic ${Buffer.from(SECRETS.dataforseo!.credentials!).toString("base64")}`,
    );
    expect(upstream.headers.get("cookie")).toBeNull();
    expect(upstream.headers.get("x-octopus-app")).toBeNull();
    expect(await upstream.text()).toBe(SERP_BODY);

    const after = await wallet(db, userId);
    expect(after.balanceMicros).toBe(before.balanceMicros - 2700);
    expect(after.heldMicros).toBe(0);
  });

  it("records metadata only", async () => {
    const rows = await db.select().from(schema.usageEvents).where(eq(schema.usageEvents.connectorId, "dataforseo"));
    expect(rows[0]).toMatchObject({
      route: "/v3/serp/*/organic/live/*",
      status: 200,
      upstreamStatus: 200,
      upstreamCostMicros: 2000,
      chargedMicros: 2700,
      app: "keyword-miner/0.1.0 macOS",
      tokenId,
    });
    expect(JSON.stringify(rows)).not.toContain("kahve");
  });

  it("replaces the app's own SerpApi key with ours and hides it in the reply", async () => {
    handler = (request) =>
      json({ search_metadata: { status: "Success" }, search_parameters: { url: request.url } });
    const response = await call("/v1/p/serpapi/search.json?engine=google&q=kahve&api_key=users-own-key");
    expect(response.status).toBe(200);
    const upstream = new URL(sent[0]!.url);
    expect(upstream.searchParams.getAll("api_key")).toEqual([SECRETS.serpapi!.api_key]);
    expect(upstream.searchParams.get("q")).toBe("kahve");
    const text = await response.text();
    expect(text).not.toContain(SECRETS.serpapi!.api_key!);
    expect(text).toContain("***");
    expect(response.headers.get(CHARGE_HEADER)).toBe("0.020250");
  });

  it("sends Serper our key instead of the app's", async () => {
    handler = () => json({ organic: [], credits: 1 });
    const response = await call("/v1/p/serper/search", {
      method: "POST",
      body: JSON.stringify({ q: "kahve" }),
      headers: { "x-api-key": "users-own-key", "content-type": "application/json" },
    });
    expect(response.status).toBe(200);
    expect(sent[0]!.headers.get("x-api-key")).toBe(SECRETS.serper!.api_key);
    expect(response.headers.get(CHARGE_HEADER)).toBe("0.001350");
  });

  it("writes the Yandex Cloud folder into the body", async () => {
    handler = () => json({ rawData: "" });
    const response = await call("/v1/p/yandex-search/v2/web/search", {
      method: "POST",
      body: JSON.stringify({ query: { searchType: "SEARCH_TYPE_TR", queryText: "kahve" }, folderId: "users-folder" }),
      headers: { "content-type": "application/json" },
    });
    expect(response.status).toBe(200);
    expect(sent[0]!.headers.get("authorization")).toBe(`Api-Key ${SECRETS["yandex-search"]!.api_key}`);
    const body = (await sent[0]!.json()) as { folderId: string; query: { queryText: string } };
    expect(body.folderId).toBe("b1gfolder123");
    expect(body.query.queryText).toBe("kahve");
    const refused = await call("/v1/p/yandex-search/v2/web/search", { method: "POST", body: "not json" });
    expect(refused.status).toBe(400);
  });
});

describe("refusals", () => {
  it("forwards only open routes and refuses path tricks", async () => {
    const cases: [string, string, number][] = [
      ["POST", "/v1/p/dataforseo/v3/appendix/user_data", 403],
      ["POST", "/v1/p/dataforseo/v3/serp/google/organic/task_get/advanced/1234", 403],
      ["GET", "/v1/p/dataforseo/v3/serp/google/organic/live/advanced", 403],
      ["POST", "/v1/p/dataforseo/v3/serp/google/organic/live/advanced/..%2f..%2fappendix", 400],
      ["POST", "/v1/p/dataforseo/v3/serp/../appendix/user_data", 403],
      ["GET", "/v1/p/serpapi/searches/abc.json", 403],
      ["GET", "/v1/p/serpapi/account.json", 403],
    ];
    for (const [method, path, status] of cases) {
      const response = await call(path, { method, body: method === "POST" ? SERP_BODY : undefined });
      expect(response.status, `${method} ${path}`).toBe(status);
    }
    expect((await call("/v1/p/serper/search", { method: "DELETE" })).status).toBe(405);
    expect(sent).toHaveLength(0);
    expect((await wallet(db, userId)).heldMicros).toBe(0);
  });

  it("answers 402 without enough credits and forwards nothing", async () => {
    const poor = await addUser(db, "poor@example.com");
    const { value } = await createToken(db, poor, "PC");
    const response = await call("/v1/p/serper/search", { method: "POST", body: "{}" }, value);
    expect(response.status).toBe(402);
    expect(((await response.json()) as { error: string }).error).toBe("insufficient_credits");
    expect(sent).toHaveLength(0);
  });

  it("answers 503 for disabled and unknown connectors", async () => {
    const yandex = (await getConnector(db, "yandex-search"))!;
    await saveConnector(db, { ...yandex, status: "disabled" }, admin);
    expect((await call("/v1/p/yandex-search/v2/web/search", { method: "POST", body: "{}" })).status).toBe(503);
    expect((await call("/v1/p/nope/anything", { method: "POST", body: "{}" })).status).toBe(503);
    await saveConnector(db, { ...yandex, status: "active" }, admin);
  });

  it("limits the request rate per token with Retry-After", async () => {
    handler = () => json({ credits: 1 });
    const statuses: number[] = [];
    for (let i = 0; i < 4; i += 1) statuses.push((await call("/v1/p/serper/search", { method: "POST", body: "{}" })).status);
    expect(statuses).toEqual([200, 200, 200, 429]);
    const limited = await call("/v1/p/serper/search", { method: "POST", body: "{}" });
    expect(Number(limited.headers.get("retry-after"))).toBeGreaterThan(0);
  });

  it("limits requests in flight", async () => {
    const { value } = await createToken(db, userId, "parallel");
    let unblock: () => void = () => undefined;
    const blocked = new Promise<void>((resolve) => {
      unblock = resolve;
    });
    handler = async () => {
      await blocked;
      return json({ cost: 0.001, tasks: [{ status_code: 20000 }] });
    };
    const path = "/v1/p/dataforseo/v3/serp/google/organic/live/advanced";
    const running = Array.from({ length: 6 }, () => call(path, { method: "POST", body: SERP_BODY }, value));
    while (sent.length < 6) await new Promise((resolve) => setTimeout(resolve, 5));
    const extra = await call(path, { method: "POST", body: SERP_BODY }, value);
    expect(extra.status).toBe(429);
    expect(((await extra.json()) as { error: string }).error).toBe("too_many_parallel");
    unblock();
    expect((await Promise.all(running)).map((response) => response.status)).toEqual([200, 200, 200, 200, 200, 200]);
  });

  it("refuses bodies over the connector's limit", async () => {
    const response = await call("/v1/p/serper/search", { method: "POST", body: "x".repeat(17 * 1024) });
    expect(response.status).toBe(413);
  });

  it("refuses a request sent twice with the same idempotency key", async () => {
    handler = () => json({ search_metadata: { status: "Success" } });
    const headers = { "idempotency-key": "abc-1" };
    expect((await call("/v1/p/serpapi/search.json?q=a", { headers })).status).toBe(200);
    expect((await call("/v1/p/serpapi/search.json?q=a", { headers })).status).toBe(409);
  });

  it("stops at a device's monthly limit and the daily limit", async () => {
    const { value, id } = await createToken(db, userId, "capped");
    await setTokenCap(db, userId, id, 10_000);
    const capped = await call("/v1/p/dataforseo/v3/serp/google/organic/live/advanced", { method: "POST", body: SERP_BODY }, value);
    expect(capped.status).toBe(402);
    expect(((await capped.json()) as { error: string }).error).toBe("token_limit");

    await updateSettings(db, { dailyLimitMicros: 1_000 }, admin);
    const daily = await call("/v1/p/serpapi/search.json?q=b");
    expect(daily.status).toBe(402);
    expect(((await daily.json()) as { error: string }).error).toBe("daily_limit");
    await updateSettings(db, { dailyLimitMicros: 50_000_000 }, admin);
  });
});

describe("upstream outcomes", () => {
  const path = "/v1/p/dataforseo/v3/serp/google/organic/live/advanced";

  it("does not charge errors reported inside a 200 reply", async () => {
    handler = () => json({ cost: 0.002, tasks: [{ status_code: 40501, status_message: "Invalid Field" }] });
    const before = await wallet(db, userId);
    const response = await call(path, { method: "POST", body: SERP_BODY });
    expect(response.status).toBe(200);
    expect(response.headers.get(CHARGE_HEADER)).toBe("0.000000");
    expect((await wallet(db, userId)).balanceMicros).toBe(before.balanceMicros);
  });

  it("turns problems with our provider account into 503", async () => {
    for (const status of [401, 402, 403]) {
      handler = () => json({ status_message: "auth failed" }, status);
      const response = await call(path, { method: "POST", body: SERP_BODY });
      expect(response.status).toBe(503);
      expect(((await response.json()) as { error: string }).error).toBe("upstream_unavailable");
    }
    expect(logs.some((event) => event.event === "upstream_account")).toBe(true);
  });

  it("passes rate limits through with Retry-After", async () => {
    handler = () => json({ message: "slow down" }, 429, { "retry-after": "12" });
    const response = await call(path, { method: "POST", body: SERP_BODY });
    expect(response.status).toBe(429);
    expect(response.headers.get("retry-after")).toBe("12");
  });

  it("frees the hold when the upstream times out or fails", async () => {
    const before = await wallet(db, userId);
    handler = () => {
      throw new DOMException("timed out", "TimeoutError");
    };
    expect((await call(path, { method: "POST", body: SERP_BODY })).status).toBe(504);
    handler = () => {
      throw new TypeError("fetch failed");
    };
    expect((await call(path, { method: "POST", body: SERP_BODY })).status).toBe(502);
    expect(await wallet(db, userId)).toEqual(before);
  });

  it("does not follow redirects", async () => {
    handler = () => new Response(null, { status: 302, headers: { location: "https://elsewhere.example" } });
    const response = await call(path, { method: "POST", body: SERP_BODY });
    expect(response.status).toBe(502);
    expect(sent[0]!.redirect).toBe("manual");
  });
});

describe("device sign-in", () => {
  it("gives the app a token after approval in the portal", async () => {
    const person = await addUser(db, "device@example.com");
    const started = await app.request("/v1/device/code", {
      method: "POST",
      body: JSON.stringify({ client: "serp-finder/0.1.0 Windows" }),
      headers: { "content-type": "application/json", "x-forwarded-for": "203.0.113.5" },
    });
    const code = (await started.json()) as {
      device_code: string;
      user_code: string;
      verification_uri: string;
      verification_uri_complete: string;
      interval: number;
    };
    expect(code.verification_uri).toBe("https://my.octopuson.com/cihaz");
    expect(code.verification_uri_complete).toBe(`https://my.octopuson.com/cihaz?code=${code.user_code}`);

    const poll = () =>
      app.request("/v1/device/token", {
        method: "POST",
        body: JSON.stringify({ device_code: code.device_code }),
        headers: { "content-type": "application/json" },
      });
    expect(await (await poll()).json()).toEqual({ error: "authorization_pending" });
    expect(await (await poll()).json()).toEqual({ error: "slow_down" });
    expect(await approveDevice(db, KEY, code.user_code, person)).toBe(true);
    await db.update(schema.deviceCodes).set({ lastPolledAt: null }).where(eq(schema.deviceCodes.userCode, code.user_code));
    const granted = (await (await poll()).json()) as { access_token: string; token_type: string };
    expect(granted.token_type).toBe("bearer");
    expect((await (await poll()).json()) as unknown).toEqual({ error: "expired_token" });

    const me = await call("/v1/me", {}, granted.access_token);
    expect(await me.json()).toMatchObject({ email: "device@example.com", available_micros: 0, low_balance: true });
    expect((await call("/v1/logout", { method: "POST" }, granted.access_token)).status).toBe(204);
    expect((await call("/v1/me", {}, granted.access_token)).status).toBe(401);
  });

  it("rejects malformed polls and limits code requests per address", async () => {
    const bad = await app.request("/v1/device/token", { method: "POST", body: "{}" });
    expect(await bad.json()).toEqual({ error: "invalid_request" });
    const unknown = await app.request("/v1/device/token", { method: "POST", body: JSON.stringify({ device_code: "x" }) });
    expect(await unknown.json()).toEqual({ error: "invalid_grant" });
    const statuses: number[] = [];
    for (let i = 0; i < 11; i += 1) {
      const response = await app.request("/v1/device/code", { method: "POST", body: "{}", headers: { "x-forwarded-for": "198.51.100.7" } });
      statuses.push(response.status);
    }
    expect(statuses.filter((status) => status === 429)).toHaveLength(1);
  });
});

describe("one-click sign-in", () => {
  it("swaps a portal code for a token with the right verifier only", async () => {
    const person = await addUser(db, "oneclick@example.com");
    const verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const request = parseConnectRequest(
      new URLSearchParams({
        redirect_uri: "http://127.0.0.1:53123",
        state: "state-1234",
        code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        code_challenge_method: "S256",
        client: "serp-finder/0.1.0 macos",
      }),
    )!;
    const code = new URL(await createConnectCode(db, person, request)).searchParams.get("code")!;
    const swap = (body: unknown) =>
      app.request("/v1/connect/token", { method: "POST", body: JSON.stringify(body), headers: { "content-type": "application/json" } });
    expect((await swap({ code })).status).toBe(400);
    const granted = await swap({ code, code_verifier: verifier });
    expect(granted.status).toBe(200);
    const { access_token: token } = (await granted.json()) as { access_token: string };
    expect(await (await call("/v1/me", {}, token)).json()).toMatchObject({ email: "oneclick@example.com" });
    expect(await (await swap({ code, code_verifier: verifier })).json()).toEqual({ error: "invalid_grant" });
  });
});
