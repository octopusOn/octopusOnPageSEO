// Wallets: every change is one transaction that locks the wallet row, updates it and appends a
// ledger entry, so the balance always equals the sum of the ledger and two requests can never
// spend the same money.

import { and, eq, gte, lt, sql } from "drizzle-orm";

import type { Db, Tx } from "./db/client.ts";
import { holds, ledger, meterEvents, usageEvents, wallets } from "./db/schema.ts";

/** Not enough available balance for a hold. */
export class InsufficientFunds extends Error {
  readonly availableMicros: number;
  constructor(availableMicros: number) {
    super("insufficient credits");
    this.availableMicros = availableMicros;
  }
}

/** A hold with an idempotency key that was used before: the request was already sent. */
export class DuplicateRequest extends Error {
  constructor() {
    super("this request was already sent");
  }
}

export interface Wallet {
  balanceMicros: number;
  heldMicros: number;
  /** Balance minus holds: what new requests may reserve. */
  availableMicros: number;
}

/** Ledger entry kinds that add or remove money. */
export type CreditKind = "purchase" | "refund" | "adjustment" | "bonus" | "chargeback";

async function lockWallet(tx: Tx, userId: string): Promise<{ balanceMicros: number; heldMicros: number }> {
  await tx.insert(wallets).values({ userId }).onConflictDoNothing();
  const [row] = await tx
    .select({ balanceMicros: wallets.balanceMicros, heldMicros: wallets.heldMicros })
    .from(wallets)
    .where(eq(wallets.userId, userId))
    .for("update");
  if (!row) throw new Error(`no wallet for ${userId}`);
  return row;
}

async function write(
  tx: Tx,
  userId: string,
  entry: {
    kind: string;
    amountMicros: number;
    heldDeltaMicros?: number;
    refType?: string;
    refId?: string;
    idempotencyKey?: string;
    note?: string;
    actorId?: string;
  },
  before: { balanceMicros: number; heldMicros: number },
): Promise<Wallet> {
  const balanceMicros = before.balanceMicros + entry.amountMicros;
  const heldMicros = before.heldMicros + (entry.heldDeltaMicros ?? 0);
  await tx
    .update(wallets)
    .set({ balanceMicros, heldMicros, updatedAt: sql`now()` })
    .where(eq(wallets.userId, userId));
  await tx.insert(ledger).values({
    userId,
    kind: entry.kind,
    amountMicros: entry.amountMicros,
    heldDeltaMicros: entry.heldDeltaMicros ?? 0,
    balanceAfter: balanceMicros,
    heldAfter: heldMicros,
    refType: entry.refType,
    refId: entry.refId,
    idempotencyKey: entry.idempotencyKey,
    note: entry.note,
    actorId: entry.actorId,
  });
  return { balanceMicros, heldMicros, availableMicros: balanceMicros - heldMicros };
}

/** The wallet of a user (zero when it has none yet). */
export async function wallet(db: Db, userId: string): Promise<Wallet> {
  const [row] = await db
    .select({ balanceMicros: wallets.balanceMicros, heldMicros: wallets.heldMicros })
    .from(wallets)
    .where(eq(wallets.userId, userId));
  const balanceMicros = row?.balanceMicros ?? 0;
  const heldMicros = row?.heldMicros ?? 0;
  return { balanceMicros, heldMicros, availableMicros: balanceMicros - heldMicros };
}

/**
 * Adds (or, with a negative amount, removes) money. An idempotency key makes a repeated call
 * (a payment callback delivered twice) a no-op that returns the current wallet.
 */
export async function credit(
  db: Db,
  userId: string,
  kind: CreditKind,
  amountMicros: number,
  options: { refType?: string; refId?: string; idempotencyKey?: string; note?: string; actorId?: string } = {},
): Promise<Wallet> {
  if (!Number.isInteger(amountMicros)) throw new RangeError("amounts are whole micro-dollars");
  return db.transaction(async (tx) => {
    const before = await lockWallet(tx, userId);
    if (options.idempotencyKey) {
      const [seen] = await tx.select({ id: ledger.id }).from(ledger).where(eq(ledger.idempotencyKey, options.idempotencyKey));
      if (seen) return { ...before, availableMicros: before.balanceMicros - before.heldMicros };
    }
    return write(tx, userId, { kind, amountMicros, ...options }, before);
  });
}

/** Reserves an amount for a request; throws {@link InsufficientFunds} when it is not available. */
export async function hold(
  db: Db,
  userId: string,
  amountMicros: number,
  refs: { connectorId: string; tokenId?: string | null; idempotencyKey?: string },
): Promise<{ holdId: string; wallet: Wallet }> {
  if (!Number.isInteger(amountMicros) || amountMicros <= 0) throw new RangeError("a hold is a positive whole amount");
  return db.transaction(async (tx) => {
    const before = await lockWallet(tx, userId);
    if (refs.idempotencyKey) {
      const [seen] = await tx
        .select({ id: holds.id, status: holds.status })
        .from(holds)
        .where(eq(holds.idempotencyKey, refs.idempotencyKey));
      if (seen) throw new DuplicateRequest();
    }
    const available = before.balanceMicros - before.heldMicros;
    if (available < amountMicros) throw new InsufficientFunds(available);
    const [created] = await tx
      .insert(holds)
      .values({
        userId,
        connectorId: refs.connectorId,
        tokenId: refs.tokenId ?? null,
        amountMicros,
        idempotencyKey: refs.idempotencyKey,
      })
      .returning({ id: holds.id });
    if (!created) throw new Error("hold not created");
    const after = await write(
      tx,
      userId,
      { kind: "hold", amountMicros: 0, heldDeltaMicros: amountMicros, refType: "hold", refId: created.id },
      before,
    );
    return { holdId: created.id, wallet: after };
  });
}

/**
 * Settles a hold: charges `chargeMicros` (possibly more than was held, possibly zero) and frees
 * the reservation. Settling twice is a no-op.
 */
export async function settleHold(db: Db, holdId: string, chargeMicros: number): Promise<Wallet> {
  if (!Number.isInteger(chargeMicros) || chargeMicros < 0) throw new RangeError("a charge is a whole amount of zero or more");
  return db.transaction(async (tx) => {
    const [row] = await tx.select().from(holds).where(eq(holds.id, holdId)).for("update");
    if (!row) throw new Error(`no hold ${holdId}`);
    const before = await lockWallet(tx, row.userId);
    if (row.status !== "open") return { ...before, availableMicros: before.balanceMicros - before.heldMicros };
    await tx
      .update(holds)
      .set({ status: chargeMicros > 0 ? "captured" : "released", settledAt: sql`now()` })
      .where(eq(holds.id, holdId));
    return write(
      tx,
      row.userId,
      {
        kind: chargeMicros > 0 ? "capture" : "release",
        amountMicros: -chargeMicros,
        heldDeltaMicros: -row.amountMicros,
        refType: "hold",
        refId: holdId,
      },
      before,
    );
  });
}

/** What one metered batch used and cost. */
export interface MeterReceipt {
  sku: string;
  units: number;
  /** Units taken from the month's free allowance. */
  freeUnits: number;
  chargedMicros: number;
  /** Free units left this month after this batch. */
  freeLeft: number;
  wallet: Wallet;
  /** The idempotency key was seen before: nothing was charged now. */
  duplicate: boolean;
}

/** What a metered unit costs and how many are free each month. */
export interface MeterTerms {
  microsPerUnit: number;
  freePerMonth: number;
}

/** The first moment of the current UTC month. */
function monthStart(now: Date): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
}

/**
 * Charges a batch of metered server work (pages crawled on the server): the month's free
 * allowance first, the rest from the wallet, in one transaction that locks the wallet so two
 * batches cannot use the same free units or money. A repeated idempotency key returns the first
 * batch's figures without charging again. Throws {@link InsufficientFunds} when the wallet cannot
 * pay; nothing is recorded then.
 */
export async function meterUsage(
  db: Db,
  userId: string,
  batch: { sku: string; units: number; idempotencyKey: string; tokenId?: string | null; app?: string | null },
  terms: MeterTerms,
  now: Date = new Date(),
): Promise<MeterReceipt> {
  if (!Number.isInteger(batch.units) || batch.units <= 0) throw new RangeError("units are a positive whole number");
  const key = `meter:${userId}:${batch.idempotencyKey}`;
  return db.transaction(async (tx) => {
    const before = await lockWallet(tx, userId);
    const since = monthStart(now);
    const [usedRow] = await tx
      .select({ units: sql<string>`coalesce(sum(${meterEvents.units}), 0)` })
      .from(meterEvents)
      .where(and(eq(meterEvents.userId, userId), eq(meterEvents.sku, batch.sku), gte(meterEvents.createdAt, since)));
    const used = Number(usedRow?.units ?? 0);
    const [seen] = await tx.select().from(meterEvents).where(eq(meterEvents.idempotencyKey, key));
    if (seen) {
      return {
        sku: seen.sku,
        units: seen.units,
        freeUnits: seen.freeUnits,
        chargedMicros: seen.chargedMicros,
        freeLeft: Math.max(0, terms.freePerMonth - used),
        wallet: { ...before, availableMicros: before.balanceMicros - before.heldMicros },
        duplicate: true,
      };
    }
    const freeUnits = Math.min(batch.units, Math.max(0, terms.freePerMonth - used));
    const chargedMicros = (batch.units - freeUnits) * terms.microsPerUnit;
    const available = before.balanceMicros - before.heldMicros;
    if (chargedMicros > available) throw new InsufficientFunds(available);
    const after =
      chargedMicros > 0
        ? await write(
            tx,
            userId,
            { kind: "capture", amountMicros: -chargedMicros, refType: "meter", refId: batch.sku, idempotencyKey: key, note: `${batch.units} ${batch.sku}` },
            before,
          )
        : { ...before, availableMicros: available };
    await tx.insert(meterEvents).values({
      userId,
      tokenId: batch.tokenId ?? null,
      sku: batch.sku,
      units: batch.units,
      freeUnits,
      chargedMicros,
      idempotencyKey: key,
      app: batch.app ?? null,
    });
    await tx.insert(usageEvents).values({
      userId,
      tokenId: batch.tokenId ?? null,
      connectorId: "server",
      method: "METER",
      route: batch.sku,
      status: 200,
      chargedMicros,
      app: batch.app ?? null,
    });
    return {
      sku: batch.sku,
      units: batch.units,
      freeUnits,
      chargedMicros,
      freeLeft: Math.max(0, terms.freePerMonth - used - freeUnits),
      wallet: after,
      duplicate: false,
    };
  });
}

/** Frees holds left open longer than `olderThanMs` (a gateway that stopped mid-request). */
export async function releaseStaleHolds(db: Db, olderThanMs: number): Promise<number> {
  const cutoff = new Date(Date.now() - olderThanMs);
  const stale = await db
    .select({ id: holds.id })
    .from(holds)
    .where(and(eq(holds.status, "open"), lt(holds.createdAt, cutoff)));
  for (const { id } of stale) await settleHold(db, id, 0);
  return stale.length;
}

/** Whether the ledger of a user adds up to its wallet. */
export async function ledgerBalances(db: Db, userId: string): Promise<boolean> {
  const [sum] = await db
    .select({
      amount: sql<string>`coalesce(sum(${ledger.amountMicros}), 0)`,
      held: sql<string>`coalesce(sum(${ledger.heldDeltaMicros}), 0)`,
    })
    .from(ledger)
    .where(eq(ledger.userId, userId));
  const current = await wallet(db, userId);
  return Number(sum?.amount ?? 0) === current.balanceMicros && Number(sum?.held ?? 0) === current.heldMicros;
}
