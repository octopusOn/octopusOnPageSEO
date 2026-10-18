// API tokens the desktop apps send to the gateway. Only their SHA-256 digest is stored.

import { and, desc, eq, gte, isNull, sql } from "drizzle-orm";

import { digest, newToken, TOKEN_PREFIX } from "./crypto.ts";
import type { Db, Tx } from "./db/client.ts";
import { apiTokens, usageEvents, user } from "./db/schema.ts";

export interface TokenOwner {
  tokenId: string;
  userId: string;
  role: string;
  monthlyCapMicros: number | null;
}

/** Creates a token; the returned value is shown once and never stored. */
export async function createToken(
  db: Db | Tx,
  userId: string,
  name: string,
  client: string | null = null,
): Promise<{ id: string; value: string; prefix: string }> {
  const token = newToken();
  const [row] = await db
    .insert(apiTokens)
    .values({ userId, name: name.slice(0, 80), prefix: token.prefix, hash: token.hash, client })
    .returning({ id: apiTokens.id });
  if (!row) throw new Error("token not created");
  return { id: row.id, value: token.value, prefix: token.prefix };
}

/** The owner of a valid token (not revoked, user not banned), or null. */
export async function verifyToken(db: Db, value: string): Promise<TokenOwner | null> {
  if (!value.startsWith(TOKEN_PREFIX) || value.length > 100) return null;
  const [row] = await db
    .select({
      tokenId: apiTokens.id,
      userId: apiTokens.userId,
      role: user.role,
      banned: user.banned,
      monthlyCapMicros: apiTokens.monthlyCapMicros,
      lastUsedAt: apiTokens.lastUsedAt,
    })
    .from(apiTokens)
    .innerJoin(user, eq(user.id, apiTokens.userId))
    .where(and(eq(apiTokens.hash, digest(value)), isNull(apiTokens.revokedAt)));
  if (!row || row.banned) return null;
  // Recorded at most every five minutes, so busy tokens do not write on every request.
  if (!row.lastUsedAt || Date.now() - row.lastUsedAt.getTime() > 5 * 60_000) {
    await db.update(apiTokens).set({ lastUsedAt: sql`now()` }).where(eq(apiTokens.id, row.tokenId));
  }
  return { tokenId: row.tokenId, userId: row.userId, role: row.role ?? "user", monthlyCapMicros: row.monthlyCapMicros };
}

/** Revokes a token of a user; false when there is no such token. */
export async function revokeToken(db: Db, userId: string, tokenId: string): Promise<boolean> {
  const rows = await db
    .update(apiTokens)
    .set({ revokedAt: sql`now()` })
    .where(and(eq(apiTokens.id, tokenId), eq(apiTokens.userId, userId), isNull(apiTokens.revokedAt)))
    .returning({ id: apiTokens.id });
  return rows.length > 0;
}

/** Sets or removes a token's monthly spending limit. */
export async function setTokenCap(db: Db, userId: string, tokenId: string, capMicros: number | null): Promise<void> {
  await db
    .update(apiTokens)
    .set({ monthlyCapMicros: capMicros })
    .where(and(eq(apiTokens.id, tokenId), eq(apiTokens.userId, userId)));
}

/** The tokens of a user, newest first, without digests. */
export async function listTokens(db: Db, userId: string) {
  return db
    .select({
      id: apiTokens.id,
      name: apiTokens.name,
      prefix: apiTokens.prefix,
      client: apiTokens.client,
      monthlyCapMicros: apiTokens.monthlyCapMicros,
      createdAt: apiTokens.createdAt,
      lastUsedAt: apiTokens.lastUsedAt,
      revokedAt: apiTokens.revokedAt,
    })
    .from(apiTokens)
    .where(eq(apiTokens.userId, userId))
    .orderBy(desc(apiTokens.createdAt));
}

/** What a token was charged since the start of the month (UTC). */
export async function monthSpend(db: Db, tokenId: string): Promise<number> {
  const now = new Date();
  const start = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
  const [row] = await db
    .select({ total: sql<string>`coalesce(sum(${usageEvents.chargedMicros}), 0)` })
    .from(usageEvents)
    .where(and(eq(usageEvents.tokenId, tokenId), gte(usageEvents.createdAt, start)));
  return Number(row?.total ?? 0);
}
