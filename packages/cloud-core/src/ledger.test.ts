import { eq } from "drizzle-orm";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { connect, type Db } from "./db/client.ts";
import { migrate } from "./db/migrate.ts";
import { holds, ledger } from "./db/schema.ts";
import { credit, DuplicateRequest, hold, InsufficientFunds, ledgerBalances, releaseStaleHolds, settleHold, wallet } from "./ledger.ts";
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
