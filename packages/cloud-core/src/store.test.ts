import { eq } from "drizzle-orm";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { connectorSchema } from "./connector.ts";
import type { Db } from "./db/client.ts";
import { apiTokens, auditLog, connectorSecrets, deviceCodes, user } from "./db/schema.ts";
import { approveDevice, denyDevice, pendingDevice, pollDevice, startDevice } from "./devices.ts";
import {
  connectorHistory,
  connectorSecretValues,
  getConnector,
  listConnectors,
  removeConnectorSecret,
  saveConnector,
  setConnectorSecret,
  setConnectorStatus,
  usableConnectors,
} from "./plugins.ts";
import { SEED_CONNECTORS, seedConnectors } from "./seeds.ts";
import { DEFAULT_SETTINGS, getSettings, updateSettings } from "./settings.ts";
import { addUser, testDb } from "./testing.ts";
import { adjustWallet, auditEntries, overview, revokeAllTokens, searchUsers, setBanned, userDetail } from "./admin.ts";
import { createToken, listTokens, monthSpend, revokeToken, setTokenCap, verifyToken } from "./tokens.ts";
import { chargedSince, marginReport, recordUsage, userUsage } from "./usage.ts";

const KEY = Buffer.alloc(32, 3);
let db: Db;
let close: () => Promise<void>;
let admin: string;

beforeAll(async () => {
  ({ db, close } = await testDb());
  admin = await addUser(db, "admin@example.com", "admin");
});
afterAll(async () => close());

describe("connector store", () => {
  it("saves every change as a new version", async () => {
    const [serper] = SEED_CONNECTORS.filter((seed) => seed.id === "serper");
    await saveConnector(db, serper!, admin);
    await saveConnector(db, { ...serper!, name: "Serper.dev" }, admin);
    expect((await getConnector(db, "serper"))?.name).toBe("Serper.dev");
    const history = await connectorHistory(db, "serper");
    expect(history.map((version) => version.version)).toEqual([2, 1]);
    expect(connectorSchema.parse(history[1]!.config).name).toBe("Serper");
    expect(await getConnector(db, "missing")).toBeNull();
  });

  it("refuses invalid configurations and activation before the terms are confirmed", async () => {
    const [serpapi] = SEED_CONNECTORS.filter((seed) => seed.id === "serpapi");
    await saveConnector(db, serpapi!, admin);
    await expect(saveConnector(db, { ...serpapi!, upstream: "http://serpapi.com" }, admin)).rejects.toThrow();
    await expect(setConnectorStatus(db, "serpapi", "active", admin)).rejects.toThrow(/terms/);
    expect((await connectorHistory(db, "serpapi")).length).toBe(1);
    await saveConnector(db, { ...serpapi!, terms: { confirmed: true, date: "2026-10-18", note: "test" } }, admin);
    const active = await setConnectorStatus(db, "serpapi", "active", admin);
    expect(active.status).toBe("active");
    expect((await usableConnectors(db)).map((connector) => connector.id)).toEqual(["serpapi"]);
    await setConnectorStatus(db, "serpapi", "disabled", admin);
    expect(await usableConnectors(db)).toEqual([]);
  });

  it("keeps our credentials sealed and reports missing ones", async () => {
    const [wordstat] = SEED_CONNECTORS.filter((seed) => seed.id === "wordstat");
    await saveConnector(db, wordstat!, admin);
    await setConnectorSecret(db, KEY, "wordstat", "api_key", "  AQVN-secret-value  ", admin);
    const [stored] = await db.select().from(connectorSecrets).where(eq(connectorSecrets.connectorId, "wordstat"));
    expect(stored?.sealed).not.toContain("AQVN");
    const record = (await listConnectors(db)).find((item) => item.connector.id === "wordstat");
    expect(record?.secrets).toEqual(["api_key"]);
    expect(record?.missingSecrets).toEqual(["folder_id"]);
    await setConnectorSecret(db, KEY, "wordstat", "folder_id", "b1g-folder", admin);
    await setConnectorSecret(db, KEY, "wordstat", "api_key", "AQVN-rotated", admin);
    expect(await connectorSecretValues(db, KEY, "wordstat")).toEqual({ api_key: "AQVN-rotated", folder_id: "b1g-folder" });
    await expect(setConnectorSecret(db, KEY, "wordstat", "api_key", "   ", admin)).rejects.toThrow(/empty/);
    await removeConnectorSecret(db, "wordstat", "folder_id", admin);
    expect(Object.keys(await connectorSecretValues(db, KEY, "wordstat"))).toEqual(["api_key"]);
  });

  it("audits changes without the secret values", async () => {
    const entries = await db.select().from(auditLog).where(eq(auditLog.targetId, "wordstat"));
    expect(entries.map((entry) => entry.action)).toContain("connector.secret.set");
    expect(JSON.stringify(entries)).not.toContain("AQVN");
  });
});

describe("seeds", () => {
  it("adds missing connectors as drafts and keeps edited ones", async () => {
    const added = await seedConnectors(db);
    expect(added).toEqual(["dataforseo", "yandex-search"]);
    expect(await seedConnectors(db)).toEqual([]);
    expect((await getConnector(db, "serper"))?.name).toBe("Serper.dev");
    expect((await getConnector(db, "dataforseo"))?.status).toBe("draft");
  });
});

describe("tokens", () => {
  it("verifies tokens by digest and stops revoked ones", async () => {
    const owner = await addUser(db, "tokens@example.com");
    const token = await createToken(db, owner, "MacBook", "keyword-miner/0.1.0");
    const [row] = await db.select().from(apiTokens).where(eq(apiTokens.id, token.id));
    expect(row?.hash).not.toBe(token.value);
    expect(JSON.stringify(row)).not.toContain(token.value);
    expect(await verifyToken(db, token.value)).toMatchObject({ tokenId: token.id, userId: owner, role: "user" });
    expect(await verifyToken(db, `${token.value}x`)).toBeNull();
    expect(await verifyToken(db, "Bearer something")).toBeNull();
    await setTokenCap(db, owner, token.id, 5_000_000);
    expect((await verifyToken(db, token.value))?.monthlyCapMicros).toBe(5_000_000);
    expect(await revokeToken(db, "someone-else", token.id)).toBe(false);
    expect(await revokeToken(db, owner, token.id)).toBe(true);
    expect(await verifyToken(db, token.value)).toBeNull();
    expect((await listTokens(db, owner))[0]?.revokedAt).toBeInstanceOf(Date);
  });

  it("stops the tokens of banned users", async () => {
    const owner = await addUser(db, "banned@example.com");
    const token = await createToken(db, owner, "PC");
    await db.update(user).set({ banned: true }).where(eq(user.id, owner));
    expect(await verifyToken(db, token.value)).toBeNull();
  });
});

describe("device sign-in", () => {
  it("delivers the token once after approval", async () => {
    const owner = await addUser(db, "device@example.com");
    const start = await startDevice(db, "keyword-miner/0.1.0 macOS");
    expect(await pollDevice(db, KEY, start.deviceCode)).toEqual({ status: "pending" });
    expect(await pollDevice(db, KEY, start.deviceCode)).toEqual({ status: "slow_down" });
    expect(await pendingDevice(db, start.userCode.toLowerCase().replace("-", " "))).toMatchObject({
      client: "keyword-miner/0.1.0 macOS",
    });
    expect(await approveDevice(db, KEY, start.userCode, owner)).toBe(true);
    expect(await approveDevice(db, KEY, start.userCode, owner)).toBe(false);
    await db.update(deviceCodes).set({ lastPolledAt: null }).where(eq(deviceCodes.userCode, start.userCode));
    const delivered = await pollDevice(db, KEY, start.deviceCode);
    expect(delivered.status).toBe("approved");
    const token = delivered.status === "approved" ? delivered.token : "";
    expect((await verifyToken(db, token))?.userId).toBe(owner);
    expect(await pollDevice(db, KEY, start.deviceCode)).toEqual({ status: "expired" });
    const [row] = await db.select().from(deviceCodes).where(eq(deviceCodes.userCode, start.userCode));
    expect(row?.tokenSealed).toBeNull();
    expect(await listTokens(db, owner)).toHaveLength(1);
  });

  it("handles denied, expired and unknown codes", async () => {
    const owner = await addUser(db, "device2@example.com");
    const denied = await startDevice(db, "serp-finder");
    await denyDevice(db, denied.userCode);
    expect(await pollDevice(db, KEY, denied.deviceCode)).toEqual({ status: "denied" });
    expect(await approveDevice(db, KEY, denied.userCode, owner)).toBe(false);

    const expired = await startDevice(db, "serp-finder");
    await db
      .update(deviceCodes)
      .set({ expiresAt: new Date(Date.now() - 1000) })
      .where(eq(deviceCodes.userCode, expired.userCode));
    expect(await pendingDevice(db, expired.userCode)).toBeNull();
    expect(await approveDevice(db, KEY, expired.userCode, owner)).toBe(false);
    expect(await pollDevice(db, KEY, expired.deviceCode)).toEqual({ status: "expired" });

    expect(await pollDevice(db, KEY, "no-such-code")).toEqual({ status: "unknown" });
    expect(await listTokens(db, owner)).toHaveLength(0);
  });
});

describe("settings", () => {
  it("returns defaults and validates changes", async () => {
    expect(await getSettings(db)).toEqual(DEFAULT_SETTINGS);
    const next = await updateSettings(db, { markup: 0.4 }, admin);
    expect(next.markup).toBe(0.4);
    expect((await getSettings(db)).markup).toBe(0.4);
    await updateSettings(db, { markup: 0.3 }, admin);
    expect((await getSettings(db)).markup).toBe(0.3);
    await expect(updateSettings(db, { markup: -1 }, admin)).rejects.toThrow();
    expect((await getSettings(db)).markup).toBe(0.3);
  });
});

describe("usage", () => {
  it("sums charges and margins", async () => {
    const owner = await addUser(db, "usage@example.com");
    const base = { userId: owner, method: "POST", route: "/search", status: 200, app: "keyword-miner" };
    await recordUsage(db, { ...base, connectorId: "serper", upstreamCostMicros: 1000, chargedMicros: 1350 });
    await recordUsage(db, { ...base, connectorId: "serper", upstreamCostMicros: 1000, chargedMicros: 1350 });
    await recordUsage(db, { ...base, connectorId: "serper", status: 502, upstreamCostMicros: 0, chargedMicros: 0 });
    await recordUsage(db, { ...base, connectorId: "dataforseo", upstreamCostMicros: 2000, chargedMicros: 2700 });
    const since = new Date(Date.now() - 60_000);
    expect(await chargedSince(db, owner, since)).toBe(5400);
    expect(await chargedSince(db, owner, new Date(Date.now() + 60_000))).toBe(0);
    expect(await userUsage(db, owner, since)).toEqual([
      { connectorId: "dataforseo", app: "keyword-miner", requests: 1, failures: 0, chargedMicros: 2700 },
      { connectorId: "serper", app: "keyword-miner", requests: 3, failures: 1, chargedMicros: 2700 },
    ]);
    const report = await marginReport(db, since);
    expect(report.find((row) => row.connectorId === "serper")).toMatchObject({
      requests: 3,
      upstreamMicros: 2000,
      chargedMicros: 2700,
      marginMicros: 700,
    });
    expect(report[0]?.day).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("counts token spending for the month", async () => {
    const owner = await addUser(db, "cap@example.com");
    const token = await createToken(db, owner, "PC");
    await recordUsage(db, {
      userId: owner,
      tokenId: token.id,
      connectorId: "serper",
      method: "POST",
      route: "/search",
      status: 200,
      chargedMicros: 1350,
    });
    expect(await monthSpend(db, token.id)).toBe(1350);
  });
});

describe("admin", () => {
  it("finds accounts, adjusts wallets with a reason and suspends", async () => {
    const id = await addUser(db, "customer@example.com");
    expect((await searchUsers(db, "customer")).map((row) => row.id)).toEqual([id]);
    expect(await searchUsers(db, "100%_")).toEqual([]);
    await expect(adjustWallet(db, id, 1_000_000, " ", admin)).rejects.toThrow(/reason/);
    expect((await adjustWallet(db, id, 1_000_000, "goodwill", admin)).balanceMicros).toBe(1_000_000);
    const token = await createToken(db, id, "PC");
    await setBanned(db, id, true, "chargeback", admin);
    expect(await verifyToken(db, token.value)).toBeNull();
    await setBanned(db, id, false, "", admin);
    expect(await verifyToken(db, token.value)).not.toBeNull();
    expect(await revokeAllTokens(db, id, admin)).toBe(1);
    const detail = await userDetail(db, id);
    expect(detail?.wallet.balanceMicros).toBe(1_000_000);
    expect(detail?.ledger[0]).toMatchObject({ kind: "adjustment", note: "goodwill", actorId: admin });
    const actions = (await auditEntries(db)).map((entry) => entry.action);
    expect(actions).toEqual(expect.arrayContaining(["wallet.adjust", "user.ban", "user.unban", "tokens.revoke_all"]));
    expect((await overview(db, new Date(0))).accounts).toBeGreaterThan(1);
  });
});
