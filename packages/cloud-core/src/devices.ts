// Device sign-in (as in RFC 8628): the app asks for a code, the person approves it at
// my.octopuson.com/cihaz, and the app picks up a token of its own once.

import { and, eq, gt, sql } from "drizzle-orm";

import { digest, newDeviceCode, normalizeUserCode, open, seal } from "./crypto.ts";
import type { Db } from "./db/client.ts";
import { deviceCodes } from "./db/schema.ts";
import { createToken } from "./tokens.ts";

/** How long a code may be approved. */
export const DEVICE_CODE_TTL_SECONDS = 900;
/** How often the app may ask. */
export const DEVICE_POLL_SECONDS = 5;

export interface DeviceStart {
  deviceCode: string;
  userCode: string;
  expiresIn: number;
  interval: number;
}

/** Starts a sign-in for a program (`keyword-miner/0.1.0 macOS`). */
export async function startDevice(db: Db, client: string): Promise<DeviceStart> {
  const { deviceCode, userCode } = newDeviceCode();
  await db.insert(deviceCodes).values({
    deviceCodeHash: digest(deviceCode),
    userCode,
    client: client.slice(0, 120),
    expiresAt: new Date(Date.now() + DEVICE_CODE_TTL_SECONDS * 1000),
    intervalSeconds: DEVICE_POLL_SECONDS,
  });
  return { deviceCode, userCode, expiresIn: DEVICE_CODE_TTL_SECONDS, interval: DEVICE_POLL_SECONDS };
}

/** A pending code as typed by the person, for the approval page. */
export async function pendingDevice(db: Db, typed: string): Promise<{ client: string; expiresAt: Date } | null> {
  const [row] = await db
    .select({ client: deviceCodes.client, expiresAt: deviceCodes.expiresAt, status: deviceCodes.status })
    .from(deviceCodes)
    .where(eq(deviceCodes.userCode, normalizeUserCode(typed)));
  if (!row || row.status !== "pending" || row.expiresAt.getTime() < Date.now()) return null;
  return { client: row.client, expiresAt: row.expiresAt };
}

/** Approves a pending code for a signed-in user: a token is made for the program. */
export async function approveDevice(db: Db, key: Buffer, typed: string, userId: string): Promise<boolean> {
  const code = normalizeUserCode(typed);
  return db.transaction(async (tx) => {
    // Claiming the row first means a second approval (a double click) finds nothing to approve.
    const [claimed] = await tx
      .update(deviceCodes)
      .set({ status: "approved", userId })
      .where(and(eq(deviceCodes.userCode, code), eq(deviceCodes.status, "pending"), gt(deviceCodes.expiresAt, sql`now()`)))
      .returning({ id: deviceCodes.id, client: deviceCodes.client });
    if (!claimed) return false;
    const token = await createToken(tx, userId, claimed.client, claimed.client);
    await tx
      .update(deviceCodes)
      .set({ tokenId: token.id, tokenSealed: seal(key, token.value) })
      .where(eq(deviceCodes.id, claimed.id));
    return true;
  });
}

/** Denies a pending code. */
export async function denyDevice(db: Db, typed: string): Promise<void> {
  await db
    .update(deviceCodes)
    .set({ status: "denied" })
    .where(and(eq(deviceCodes.userCode, normalizeUserCode(typed)), eq(deviceCodes.status, "pending")));
}

export type DevicePoll =
  | { status: "pending" | "slow_down" | "expired" | "denied" | "unknown" }
  | { status: "approved"; token: string };

/** What the app learns when it asks: the token exactly once, after approval. */
export async function pollDevice(db: Db, key: Buffer, deviceCode: string): Promise<DevicePoll> {
  const [row] = await db.select().from(deviceCodes).where(eq(deviceCodes.deviceCodeHash, digest(deviceCode)));
  if (!row) return { status: "unknown" };
  const now = Date.now();
  if (row.status === "denied") return { status: "denied" };
  if (row.status === "delivered" || row.expiresAt.getTime() < now) return { status: "expired" };
  const tooSoon = row.lastPolledAt && now - row.lastPolledAt.getTime() < (row.intervalSeconds - 1) * 1000;
  await db.update(deviceCodes).set({ lastPolledAt: sql`now()` }).where(eq(deviceCodes.id, row.id));
  if (row.status === "pending") return { status: tooSoon ? "slow_down" : "pending" };
  if (row.status === "approved" && row.tokenSealed) {
    const delivered = await db
      .update(deviceCodes)
      .set({ status: "delivered", tokenSealed: null })
      .where(and(eq(deviceCodes.id, row.id), eq(deviceCodes.status, "approved")))
      .returning({ id: deviceCodes.id });
    if (delivered.length === 0) return { status: "expired" };
    return { status: "approved", token: open(key, row.tokenSealed) };
  }
  return { status: "expired" };
}
