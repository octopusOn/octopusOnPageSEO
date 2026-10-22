// Credit purchases: prices in lira (VAT included), credits from the day's dollar rate, orders
// that are credited exactly once however often the payment provider reports them.

import { randomBytes } from "node:crypto";

import { and, desc, eq, gte, lt, type SQL } from "drizzle-orm";

import { audit } from "./audit.ts";
import type { Db } from "./db/client.ts";
import { billingProfiles, invoices, orders, user } from "./db/schema.ts";
import { credit } from "./ledger.ts";
import type { CloudSettings } from "./settings.ts";

export interface Quote {
  /** What the buyer pays, VAT included. */
  amountKurus: number;
  vatKurus: number;
  netKurus: number;
  /** TRY per USD used: the bank's rate plus the exchange margin. */
  rate: number;
  creditsMicros: number;
}

/** The credits an amount buys: VAT out, then lira to dollars at the rate plus margin. */
export function quote(amountKurus: number, settings: CloudSettings, bankRate: number): Quote {
  if (!Number.isInteger(amountKurus) || amountKurus <= 0) throw new RangeError("an amount in whole kuruş");
  const netKurus = Math.round(amountKurus / (1 + settings.vatRate));
  const rate = Math.round(bankRate * (1 + settings.fxBufferPercent / 100) * 10_000) / 10_000;
  return {
    amountKurus,
    vatKurus: amountKurus - netKurus,
    netKurus,
    rate,
    creditsMicros: Math.floor((netKurus / 100 / rate) * 1_000_000),
  };
}

/**
 * The credits a dollar amount buys (VAT included): VAT out, then dollars to credits with no
 * exchange margin. The lira fields are its equivalent at the bank's rate, for invoices and the
 * books; `rate` is that bank rate.
 */
export function quoteUsd(amountCents: number, settings: CloudSettings, bankRate: number): Quote {
  if (!Number.isInteger(amountCents) || amountCents <= 0) throw new RangeError("an amount in whole cents");
  const amountKurus = Math.round(amountCents * bankRate);
  const netKurus = Math.round(amountKurus / (1 + settings.vatRate));
  return {
    amountKurus,
    vatKurus: amountKurus - netKurus,
    netKurus,
    rate: Math.round(bankRate * 10_000) / 10_000,
    creditsMicros: Math.floor((amountCents / (1 + settings.vatRate)) * 10_000),
  };
}

/** The `packageId` of a dollar order: its amount in cents, `usd:2500`. */
export function usdPackageId(amountCents: number): string {
  return `usd:${amountCents}`;
}

/** The cents of a dollar order, or null for one priced in lira. */
export function orderUsdCents(order: { packageId: string | null }): number | null {
  const match = /^usd:(\d+)$/.exec(order.packageId ?? "");
  return match ? Number(match[1]) : null;
}

export type AmountProblem = "below_minimum" | "over_first_purchase";

/** Why an amount cannot be bought, if it cannot. */
export function amountProblem(amountKurus: number, settings: CloudSettings, firstPurchase: boolean): AmountProblem | null {
  if (amountKurus < settings.minPurchaseKurus) return "below_minimum";
  if (firstPurchase && amountKurus > settings.firstPurchaseCapKurus) return "over_first_purchase";
  return null;
}

/** Why a dollar amount cannot be bought, if it cannot. */
export function usdAmountProblem(amountCents: number, settings: CloudSettings, firstPurchase: boolean): AmountProblem | null {
  if (amountCents < settings.minPurchaseUsdCents) return "below_minimum";
  if (firstPurchase && amountCents > settings.firstPurchaseCapUsdCents) return "over_first_purchase";
  return null;
}

export type Order = typeof orders.$inferSelect;
export type BillingProfile = typeof billingProfiles.$inferSelect;
export type BillingProfileInput = Omit<typeof billingProfiles.$inferInsert, "userId" | "updatedAt" | "externalContactId">;

export async function billingProfile(db: Db, userId: string): Promise<BillingProfile | null> {
  const [row] = await db.select().from(billingProfiles).where(eq(billingProfiles.userId, userId));
  return row ?? null;
}

/** Saves the invoice details; a changed name or tax number needs a new customer at the provider. */
export async function saveBillingProfile(db: Db, userId: string, input: BillingProfileInput): Promise<void> {
  const current = await billingProfile(db, userId);
  const sameParty =
    current &&
    current.kind === input.kind &&
    current.taxNumber === (input.taxNumber ?? null) &&
    current.fullName === input.fullName &&
    current.companyName === (input.companyName ?? null);
  const values = { ...input, updatedAt: new Date(), externalContactId: sameParty ? current.externalContactId : null };
  await db
    .insert(billingProfiles)
    .values({ userId, ...values })
    .onConflictDoUpdate({ target: billingProfiles.userId, set: values });
}

export async function hasPaidOrder(db: Db, userId: string): Promise<boolean> {
  const [row] = await db
    .select({ id: orders.id })
    .from(orders)
    .where(and(eq(orders.userId, userId), eq(orders.status, "paid")))
    .limit(1);
  return Boolean(row);
}

/** A new order; its conversation id goes to the payment provider (alphanumeric for PayTR). */
export async function createOrder(
  db: Db,
  input: { userId: string; provider: string; packageId?: string | null; quote: Quote; buyerIp?: string | null },
): Promise<Order> {
  const [row] = await db
    .insert(orders)
    .values({
      userId: input.userId,
      provider: input.provider,
      packageId: input.packageId ?? null,
      amountKurus: input.quote.amountKurus,
      vatKurus: input.quote.vatKurus,
      creditsMicros: input.quote.creditsMicros,
      fxRate: String(input.quote.rate),
      conversationId: `OCT${randomBytes(12).toString("hex").toUpperCase()}`,
      consentedAt: new Date(),
      buyerIp: input.buyerIp ?? null,
    })
    .returning();
  if (!row) throw new Error("order not created");
  return row;
}

export async function orderById(db: Db, id: string): Promise<Order | null> {
  const [row] = await db.select().from(orders).where(eq(orders.id, id));
  return row ?? null;
}

export async function orderByConversation(db: Db, conversationId: string): Promise<Order | null> {
  const [row] = await db.select().from(orders).where(eq(orders.conversationId, conversationId));
  return row ?? null;
}

/** Records the provider's reference (the checkout token) on a waiting order. */
export async function setOrderReference(db: Db, id: string, providerRef: string): Promise<void> {
  await db.update(orders).set({ providerRef }).where(eq(orders.id, id));
}

export type PaidOutcome = "credited" | "already_paid" | "amount_mismatch" | "not_found";

/**
 * Marks an order paid and adds its credits, once: a callback delivered twice, or a callback and
 * a check racing, credit a single time. The paid amount must match the order.
 */
export async function markOrderPaid(
  db: Db,
  id: string,
  payment: { paidKurus: number; providerRef?: string | null },
): Promise<PaidOutcome> {
  return db.transaction(async (tx) => {
    const [order] = await tx.select().from(orders).where(eq(orders.id, id)).for("update");
    if (!order) return "not_found";
    if (order.status === "paid") return "already_paid";
    if (payment.paidKurus !== order.amountKurus) {
      await tx
        .update(orders)
        .set({ status: "failed", failureReason: `paid ${payment.paidKurus} kuruş, expected ${order.amountKurus}` })
        .where(eq(orders.id, id));
      await audit(tx, null, "order.amount_mismatch", { type: "order", id }, { paidKurus: payment.paidKurus });
      return "amount_mismatch";
    }
    await tx
      .update(orders)
      .set({ status: "paid", paidAt: new Date(), providerRef: payment.providerRef ?? order.providerRef, failureReason: null })
      .where(eq(orders.id, id));
    await credit(tx as unknown as Db, order.userId, "purchase", order.creditsMicros, {
      refType: "order",
      refId: id,
      idempotencyKey: `order:${id}`,
      note: `${order.provider} ${order.amountKurus / 100} TRY`,
    });
    await tx.insert(invoices).values({ orderId: id, provider: "pending" }).onConflictDoNothing();
    return "credited";
  });
}

/** Marks a waiting order failed (a declined card, a cancelled form). Paid orders stay paid. */
export async function markOrderFailed(db: Db, id: string, reason: string): Promise<void> {
  await db
    .update(orders)
    .set({ status: "failed", failureReason: reason.slice(0, 500) })
    .where(and(eq(orders.id, id), eq(orders.status, "created")));
}

/**
 * Takes back the credits of a paid order (a refund made at the provider, or a chargeback);
 * the balance may go below zero. A chargeback also suspends the account.
 */
export async function reverseOrder(db: Db, id: string, kind: "refund" | "chargeback", actorId: string, note: string): Promise<void> {
  const order = await orderById(db, id);
  if (!order || order.status !== "paid") throw new Error("only paid orders can be reversed");
  await credit(db, order.userId, kind, -order.creditsMicros, {
    refType: "order",
    refId: id,
    idempotencyKey: `${kind}:${id}`,
    note,
    actorId,
  });
  await db.update(orders).set({ status: kind === "refund" ? "refunded" : "chargeback" }).where(eq(orders.id, id));
  if (kind === "chargeback") {
    await db.update(user).set({ banned: true, banReason: "chargeback" }).where(eq(user.id, order.userId));
  }
  await audit(db, actorId, `order.${kind}`, { type: "order", id }, { note });
}

/** A user's orders with their invoices, newest first. */
export async function userOrders(db: Db, userId: string, limit = 50) {
  return db
    .select({ order: orders, invoice: invoices })
    .from(orders)
    .leftJoin(invoices, eq(invoices.orderId, orders.id))
    .where(eq(orders.userId, userId))
    .orderBy(desc(orders.createdAt))
    .limit(limit);
}

/** Orders for the admin panel; with `paidFrom`/`paidTo`, those paid in that period (for accounting). */
export async function listOrders(
  db: Db,
  options: { status?: string; paidFrom?: Date; paidTo?: Date; limit?: number } = {},
) {
  const conditions: SQL[] = [];
  if (options.status) conditions.push(eq(orders.status, options.status));
  if (options.paidFrom) conditions.push(gte(orders.paidAt, options.paidFrom));
  if (options.paidTo) conditions.push(lt(orders.paidAt, options.paidTo));
  return db
    .select({ order: orders, invoice: invoices, email: user.email })
    .from(orders)
    .innerJoin(user, eq(user.id, orders.userId))
    .leftJoin(invoices, eq(invoices.orderId, orders.id))
    .where(conditions.length > 0 ? and(...conditions) : undefined)
    .orderBy(desc(orders.createdAt))
    .limit(options.limit ?? 500);
}
