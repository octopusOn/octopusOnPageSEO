import { eq } from "drizzle-orm";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { connect, type Db } from "./db/client.ts";
import { migrate } from "./db/migrate.ts";
import { holds, ledger, meterEvents, usageEvents } from "./db/schema.ts";
import { credit, DuplicateRequest, hold, InsufficientFunds, ledgerBalances, meterUsage, releaseStaleHolds, settleHold, wallet } from "./ledger.ts";
import { DEFAULT_SETTINGS } from "./settings.ts";
import { isMeterSku, meterCatalog, meterTerms } from "./meter.ts";
import { addUser, testDb } from "./testing.ts";

let db: Db;
let close: () => Promise<void>;
let users = 0;

beforeAll(async () => {
  ({ db, close } = await testDb());
});
afterAll(async () => close());

async function funded(micros: number): Promise<string> {
  users += 1;
  const id = await addUser(db, `ledger${users}@example.com`);
  if (micros > 0) await credit(db, id, "purchase", micros, { refType: "order", refId: `order-${users}` });
  return id;
}

describe("ledger", () => {
  it("starts empty and adds purchases", async () => {
    const id = await funded(0);
    expect(await wallet(db, id)).toEqual({ balanceMicros: 0, heldMicros: 0, availableMicros: 0 });
    const after = await credit(db, id, "purchase", 5_000_000);
    expect(after).toEqual({ balanceMicros: 5_000_000, heldMicros: 0, availableMicros: 5_000_000 });
    expect(await ledgerBalances(db, id)).toBe(true);
  });

  it("applies a payment callback delivered twice once", async () => {
    const id = await funded(0);
    await credit(db, id, "purchase", 1_000_000, { idempotencyKey: "iyzico:abc" });
    const again = await credit(db, id, "purchase", 1_000_000, { idempotencyKey: "iyzico:abc" });
    expect(again.balanceMicros).toBe(1_000_000);
    const entries = await db.select().from(ledger).where(eq(ledger.userId, id));
    expect(entries).toHaveLength(1);
  });

  it("refuses fractional amounts", async () => {
    const id = await funded(0);
    await expect(credit(db, id, "purchase", 1.5)).rejects.toThrow(RangeError);
    await expect(hold(db, id, 0, { connectorId: "x" })).rejects.toThrow(RangeError);
  });

  it("holds, captures the real charge and frees the rest", async () => {
    const id = await funded(1_000_000);
    const { holdId, wallet: held } = await hold(db, id, 500_000, { connectorId: "dataforseo" });
    expect(held).toEqual({ balanceMicros: 1_000_000, heldMicros: 500_000, availableMicros: 500_000 });
    const settled = await settleHold(db, holdId, 2_700);
    expect(settled).toEqual({ balanceMicros: 997_300, heldMicros: 0, availableMicros: 997_300 });
    const [row] = await db.select().from(holds).where(eq(holds.id, holdId));
    expect(row?.status).toBe("captured");
    expect(row?.settledAt).toBeInstanceOf(Date);
    expect(await ledgerBalances(db, id)).toBe(true);
  });

  it("settles a hold once", async () => {
    const id = await funded(1_000_000);
    const { holdId } = await hold(db, id, 100_000, { connectorId: "serper" });
    await settleHold(db, holdId, 1_000);
    const again = await settleHold(db, holdId, 1_000);
    expect(again.balanceMicros).toBe(999_000);
    expect(await ledgerBalances(db, id)).toBe(true);
  });

  it("releases failed requests without a charge", async () => {
    const id = await funded(1_000_000);
    const { holdId } = await hold(db, id, 100_000, { connectorId: "serper" });
    expect(await settleHold(db, holdId, 0)).toEqual({ balanceMicros: 1_000_000, heldMicros: 0, availableMicros: 1_000_000 });
    const [row] = await db.select().from(holds).where(eq(holds.id, holdId));
    expect(row?.status).toBe("released");
  });

  it("refuses holds beyond the available balance", async () => {
    const id = await funded(600_000);
    await hold(db, id, 500_000, { connectorId: "dataforseo" });
    const refused = hold(db, id, 500_000, { connectorId: "dataforseo" });
    await expect(refused).rejects.toBeInstanceOf(InsufficientFunds);
    await expect(refused).rejects.toMatchObject({ availableMicros: 100_000 });
  });

  it("refuses a request sent twice", async () => {
    const id = await funded(1_000_000);
    await hold(db, id, 10_000, { connectorId: "serper", idempotencyKey: "req-1" });
    await expect(hold(db, id, 10_000, { connectorId: "serper", idempotencyKey: "req-1" })).rejects.toBeInstanceOf(DuplicateRequest);
  });

  it("lets a charge go over the hold and the balance below zero", async () => {
    const id = await funded(10_000);
    const { holdId } = await hold(db, id, 10_000, { connectorId: "dataforseo" });
    const settled = await settleHold(db, holdId, 25_000);
    expect(settled.balanceMicros).toBe(-15_000);
    await expect(hold(db, id, 1, { connectorId: "dataforseo" })).rejects.toBeInstanceOf(InsufficientFunds);
    expect(await ledgerBalances(db, id)).toBe(true);
  });

  it("releases holds left open", async () => {
    const id = await funded(1_000_000);
    const { holdId } = await hold(db, id, 200_000, { connectorId: "serper" });
    expect(await releaseStaleHolds(db, 60_000)).toBe(0);
    await db.update(holds).set({ createdAt: new Date(Date.now() - 3_600_000) }).where(eq(holds.id, holdId));
    expect(await releaseStaleHolds(db, 60_000)).toBe(1);
    expect(await wallet(db, id)).toEqual({ balanceMicros: 1_000_000, heldMicros: 0, availableMicros: 1_000_000 });
  });

  it("never spends the same money twice under parallel requests", async () => {
    const id = await funded(50 * 10_000);
    const results = await Promise.allSettled(
      Array.from({ length: 120 }, async () => {
        const { holdId } = await hold(db, id, 10_000, { connectorId: "serper" });
        await settleHold(db, holdId, 10_000);
      }),
    );
    expect(results.filter((result) => result.status === "fulfilled")).toHaveLength(50);
    for (const result of results) {
      if (result.status === "rejected") expect(result.reason).toBeInstanceOf(InsufficientFunds);
    }
    expect(await wallet(db, id)).toEqual({ balanceMicros: 0, heldMicros: 0, availableMicros: 0 });
    expect(await ledgerBalances(db, id)).toBe(true);
  });
  it("meters server work: free units first, then the wallet, each batch once", async () => {
    const id = await funded(1_000_000);
    const terms = { microsPerUnit: 200, freePerMonth: 1000 };
    const october = new Date(Date.UTC(2026, 9, 21));
    const first = await meterUsage(db, id, { sku: "crawl.page", units: 800, idempotencyKey: "audit-1:0", app: "geo-seo-platform-web" }, terms, october);
    expect(first).toMatchObject({ freeUnits: 800, chargedMicros: 0, freeLeft: 200, duplicate: false });
    const second = await meterUsage(db, id, { sku: "crawl.page", units: 500, idempotencyKey: "audit-1:1" }, terms, october);
    expect(second).toMatchObject({ freeUnits: 200, chargedMicros: 300 * 200, freeLeft: 0 });
    expect(second.wallet.balanceMicros).toBe(1_000_000 - 60_000);
    // A batch sent twice is charged once.
    const again = await meterUsage(db, id, { sku: "crawl.page", units: 500, idempotencyKey: "audit-1:1" }, terms, october);
    expect(again).toMatchObject({ duplicate: true, chargedMicros: 60_000 });
    expect((await wallet(db, id)).balanceMicros).toBe(940_000);
    // Beyond the balance: refused, nothing recorded.
    await expect(meterUsage(db, id, { sku: "crawl.page", units: 10_000, idempotencyKey: "audit-1:2" }, terms, october)).rejects.toBeInstanceOf(InsufficientFunds);
    const batches = await db.select().from(meterEvents).where(eq(meterEvents.userId, id));
    expect(batches.map((batch) => batch.units)).toEqual([800, 500]);
    const usage = await db.select().from(usageEvents).where(eq(usageEvents.userId, id));
    expect(usage.map((row) => [row.connectorId, row.route, row.chargedMicros])).toEqual([
      ["server", "crawl.page", 0],
      ["server", "crawl.page", 60_000],
    ]);
    expect(await ledgerBalances(db, id)).toBe(true);
    // Another user's identical key is a different batch.
    const other = await funded(0);
    expect((await meterUsage(db, other, { sku: "crawl.page", units: 1, idempotencyKey: "audit-1:1" }, terms, october)).duplicate).toBe(false);
  });

  it("prices metered work from the settings", () => {
    expect(isMeterSku("crawl.page")).toBe(true);
    expect(isMeterSku("render.page")).toBe(false);
    expect(isMeterSku("report.pdf")).toBe(true);
    expect(meterTerms("crawl.page", DEFAULT_SETTINGS)).toEqual({ microsPerUnit: 200, freePerMonth: 1000 });
    expect(meterTerms("report.pdf", DEFAULT_SETTINGS)).toEqual({ microsPerUnit: 20_000, freePerMonth: 30 });
    expect(meterCatalog({ ...DEFAULT_SETTINGS, crawlPageMicros: 500 })).toEqual([
      { sku: "crawl.page", micros_per_unit: 500, free_per_month: 1000 },
      { sku: "report.pdf", micros_per_unit: 20_000, free_per_month: 30 },
    ]);
  });
});

// PGlite runs one transaction at a time, so row locks are only exercised on a real server:
// CI sets TEST_DATABASE_URL to its PostgreSQL service.
const url = process.env.TEST_DATABASE_URL;
describe.runIf(url)("ledger on PostgreSQL", () => {
  it("keeps 1,000 parallel requests consistent", async () => {
    const server = connect(url!, 20);
    try {
      await migrate(server.db);
      const id = await addUser(server.db, `pg-${Date.now()}@example.com`);
      await credit(server.db, id, "purchase", 400 * 1_000);
      const results = await Promise.allSettled(
        Array.from({ length: 1000 }, async (_, i) => {
          const { holdId } = await hold(server.db, id, 1_000, { connectorId: "serper" });
          await settleHold(server.db, holdId, i % 2 === 0 ? 1_000 : 0);
        }),
      );
      const spent = results.filter((result) => result.status === "fulfilled").length;
      expect(spent).toBeGreaterThanOrEqual(400);
      const final = await wallet(server.db, id);
      expect(final.heldMicros).toBe(0);
      expect(final.balanceMicros).toBeGreaterThanOrEqual(0);
      expect(await ledgerBalances(server.db, id)).toBe(true);
    } finally {
      await server.close();
    }
  }, 60_000);

});
