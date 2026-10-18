// Admin panel queries and actions on accounts. Every change is audited.

import { and, desc, eq, gte, ilike, or, sql } from "drizzle-orm";

import { audit } from "./audit.ts";
import type { Db } from "./db/client.ts";
import { apiTokens, auditLog, holds, ledger, orders, session, usageEvents, user, wallets } from "./db/schema.ts";
import { credit, type Wallet } from "./ledger.ts";

/** Accounts matching an e-mail or name fragment, newest first, with their balances. */
export async function searchUsers(db: Db, query: string, limit = 50) {
  const pattern = `%${query.trim().replace(/[\\%_]/g, (match) => `\\${match}`)}%`;
  const rows = await db
    .select({
      id: user.id,
      email: user.email,
      name: user.name,
      role: user.role,
      banned: user.banned,
      emailVerified: user.emailVerified,
      twoFactorEnabled: user.twoFactorEnabled,
      createdAt: user.createdAt,
      balanceMicros: wallets.balanceMicros,
    })
    .from(user)
    .leftJoin(wallets, eq(wallets.userId, user.id))
    .where(query.trim() ? or(ilike(user.email, pattern), ilike(user.name, pattern)) : undefined)
    .orderBy(desc(user.createdAt))
    .limit(limit);
  return rows.map((row) => ({ ...row, balanceMicros: row.balanceMicros ?? 0 }));
}

/** One account with its tokens, latest ledger entries and orders. */
export async function userDetail(db: Db, userId: string) {
  const [person] = await db.select().from(user).where(eq(user.id, userId));
  if (!person) return null;
  const [walletRow] = await db.select().from(wallets).where(eq(wallets.userId, userId));
  const [tokens, entries, purchases] = await Promise.all([
    db
      .select({
        id: apiTokens.id,
        name: apiTokens.name,
        prefix: apiTokens.prefix,
        client: apiTokens.client,
        lastUsedAt: apiTokens.lastUsedAt,
        revokedAt: apiTokens.revokedAt,
        createdAt: apiTokens.createdAt,
      })
      .from(apiTokens)
      .where(eq(apiTokens.userId, userId))
      .orderBy(desc(apiTokens.createdAt)),
    recentLedger(db, userId, 50),
    db.select().from(orders).where(eq(orders.userId, userId)).orderBy(desc(orders.createdAt)).limit(50),
  ]);
  const balanceMicros = walletRow?.balanceMicros ?? 0;
  const heldMicros = walletRow?.heldMicros ?? 0;
  const current: Wallet = { balanceMicros, heldMicros, availableMicros: balanceMicros - heldMicros };
  return { user: person, wallet: current, tokens, ledger: entries, orders: purchases };
}

/** The latest ledger entries of an account, without hold bookkeeping noise when asked. */
export async function recentLedger(db: Db, userId: string, limit = 50, moneyOnly = false) {
  return db
    .select()
    .from(ledger)
    .where(
      moneyOnly
        ? and(eq(ledger.userId, userId), sql`${ledger.kind} not in ('hold', 'release')`)
        : eq(ledger.userId, userId),
    )
    .orderBy(desc(ledger.id))
    .limit(limit);
}

/** Adds or removes credits by hand; the reason is required and kept. */
export async function adjustWallet(db: Db, userId: string, amountMicros: number, reason: string, actorId: string): Promise<Wallet> {
  const note = reason.trim();
  if (!note) throw new Error("a reason is required");
  if (!Number.isInteger(amountMicros) || amountMicros === 0) throw new RangeError("a non-zero whole amount");
  const after = await credit(db, userId, "adjustment", amountMicros, { note, actorId });
  await audit(db, actorId, "wallet.adjust", { type: "user", id: userId }, { amountMicros, reason: note });
  return after;
}

/** Suspends or restores an account: a suspended account cannot sign in or spend. */
export async function setBanned(db: Db, userId: string, banned: boolean, reason: string, actorId: string): Promise<void> {
  await db
    .update(user)
    .set({ banned, banReason: banned ? reason.trim() || null : null, banExpires: null })
    .where(eq(user.id, userId));
  if (banned) await db.delete(session).where(eq(session.userId, userId));
  await audit(db, actorId, banned ? "user.ban" : "user.unban", { type: "user", id: userId }, { reason });
}

/** Revokes every token of an account (a lost laptop, a suspended account). */
export async function revokeAllTokens(db: Db, userId: string, actorId: string): Promise<number> {
  const rows = await db
    .update(apiTokens)
    .set({ revokedAt: sql`now()` })
    .where(and(eq(apiTokens.userId, userId), sql`${apiTokens.revokedAt} is null`))
    .returning({ id: apiTokens.id });
  await audit(db, actorId, "tokens.revoke_all", { type: "user", id: userId }, { count: rows.length });
  return rows.length;
}

/** Figures for the admin overview since a moment. */
export async function overview(db: Db, since: Date) {
  const [[accounts], [money], [paid], [spent], [open]] = await Promise.all([
    db.select({ count: sql<string>`count(*)` }).from(user),
    db
      .select({
        balance: sql<string>`coalesce(sum(${wallets.balanceMicros}), 0)`,
        held: sql<string>`coalesce(sum(${wallets.heldMicros}), 0)`,
      })
      .from(wallets),
    db
      .select({
        count: sql<string>`count(*)`,
        kurus: sql<string>`coalesce(sum(${orders.amountKurus}), 0)`,
        credits: sql<string>`coalesce(sum(${orders.creditsMicros}), 0)`,
      })
      .from(orders)
      .where(and(eq(orders.status, "paid"), gte(orders.paidAt, since))),
    db
      .select({
        requests: sql<string>`count(*)`,
        charged: sql<string>`coalesce(sum(${usageEvents.chargedMicros}), 0)`,
        upstream: sql<string>`coalesce(sum(${usageEvents.upstreamCostMicros}), 0)`,
      })
      .from(usageEvents)
      .where(gte(usageEvents.createdAt, since)),
    db.select({ count: sql<string>`count(*)` }).from(holds).where(eq(holds.status, "open")),
  ]);
  return {
    accounts: Number(accounts?.count ?? 0),
    balancesMicros: Number(money?.balance ?? 0),
    heldMicros: Number(money?.held ?? 0),
    paidOrders: Number(paid?.count ?? 0),
    paidKurus: Number(paid?.kurus ?? 0),
    soldMicros: Number(paid?.credits ?? 0),
    requests: Number(spent?.requests ?? 0),
    chargedMicros: Number(spent?.charged ?? 0),
    upstreamMicros: Number(spent?.upstream ?? 0),
    openHolds: Number(open?.count ?? 0),
  };
}

/** The latest audit entries, with the actor's e-mail. */
export async function auditEntries(db: Db, limit = 200) {
  return db
    .select({
      id: auditLog.id,
      action: auditLog.action,
      targetType: auditLog.targetType,
      targetId: auditLog.targetId,
      data: auditLog.data,
      createdAt: auditLog.createdAt,
      actorEmail: user.email,
      actorId: auditLog.actorId,
    })
    .from(auditLog)
    .leftJoin(user, eq(user.id, auditLog.actorId))
    .orderBy(desc(auditLog.id))
    .limit(limit);
}
