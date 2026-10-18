// Usage records and reports: per user for the portal, per connector and day for admins.

import { and, desc, eq, gte, sql } from "drizzle-orm";

import type { Db } from "./db/client.ts";
import { usageEvents } from "./db/schema.ts";

export type UsageEvent = typeof usageEvents.$inferInsert;

export async function recordUsage(db: Db, event: UsageEvent): Promise<void> {
  await db.insert(usageEvents).values(event);
}

/** Charged micro-dollars of a user since a moment, for the daily limit. */
export async function chargedSince(db: Db, userId: string, since: Date): Promise<number> {
  const [row] = await db
    .select({ total: sql<string>`coalesce(sum(${usageEvents.chargedMicros}), 0)` })
    .from(usageEvents)
    .where(and(eq(usageEvents.userId, userId), gte(usageEvents.createdAt, since)));
  return Number(row?.total ?? 0);
}

/** A user's requests and charges per connector since a moment. */
export async function userUsage(db: Db, userId: string, since: Date) {
  const rows = await db
    .select({
      connectorId: usageEvents.connectorId,
      app: usageEvents.app,
      requests: sql<string>`count(*)`,
      failures: sql<string>`count(*) filter (where ${usageEvents.status} >= 400)`,
      charged: sql<string>`coalesce(sum(${usageEvents.chargedMicros}), 0)`,
    })
    .from(usageEvents)
    .where(and(eq(usageEvents.userId, userId), gte(usageEvents.createdAt, since)))
    .groupBy(usageEvents.connectorId, usageEvents.app)
    .orderBy(usageEvents.connectorId);
  return rows.map((row) => ({
    connectorId: row.connectorId,
    app: row.app,
    requests: Number(row.requests),
    failures: Number(row.failures),
    chargedMicros: Number(row.charged),
  }));
}

/** A user's latest requests (metadata only). */
export async function recentUsage(db: Db, userId: string, limit = 50) {
  return db
    .select()
    .from(usageEvents)
    .where(eq(usageEvents.userId, userId))
    .orderBy(desc(usageEvents.createdAt))
    .limit(limit);
}

/** Revenue, upstream cost and margin per connector and day, for admins. */
export async function marginReport(db: Db, since: Date) {
  const rows = await db
    .select({
      day: sql<string>`to_char(${usageEvents.createdAt} at time zone 'UTC', 'YYYY-MM-DD')`,
      connectorId: usageEvents.connectorId,
      requests: sql<string>`count(*)`,
      upstream: sql<string>`coalesce(sum(${usageEvents.upstreamCostMicros}), 0)`,
      charged: sql<string>`coalesce(sum(${usageEvents.chargedMicros}), 0)`,
    })
    .from(usageEvents)
    .where(gte(usageEvents.createdAt, since))
    .groupBy(sql`1`, usageEvents.connectorId)
    .orderBy(sql`1 desc`, usageEvents.connectorId);
  return rows.map((row) => ({
    day: row.day,
    connectorId: row.connectorId,
    requests: Number(row.requests),
    upstreamMicros: Number(row.upstream),
    chargedMicros: Number(row.charged),
    marginMicros: Number(row.charged) - Number(row.upstream),
  }));
}
