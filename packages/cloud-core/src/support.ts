// Messages people send to OctopusOn support from the programs, and our providers' refusals of
// our own account (a spent balance, a closed API), kept in the audit log for the admin panel's
// support page.

import { and, desc, eq, gte, inArray, sql } from "drizzle-orm";

import { audit } from "./audit.ts";
import type { Db } from "./db/client.ts";
import { auditLog, user } from "./db/schema.ts";

export const SUPPORT_ACTION = "support.message";
export const PROVIDER_PROBLEM_ACTION = "connector.upstream_problem";

/** What a program sends with a message: which program and page, and the error shown. */
export interface SupportContext {
  program?: string;
  version?: string;
  page?: string;
  error?: string;
}

const clip = (text: unknown, length: number) => (typeof text === "string" ? text.trim().slice(0, length) : "");

/**
 * Keeps a support message of an account; at most `perHour` an hour, so a stuck button cannot
 * flood the inbox.
 */
export async function recordSupportMessage(
  db: Db,
  userId: string,
  input: { message?: unknown; context?: SupportContext | null },
  perHour = 10,
): Promise<"saved" | "empty" | "too_many"> {
  const message = clip(input.message, 2000);
  const context = Object.fromEntries(
    (["program", "version", "page", "error"] as const)
      .map((field) => [field, clip(input.context?.[field], 500)] as const)
      .filter(([, value]) => value !== ""),
  );
  if (!message && !context.error) return "empty";
  const [sent] = await db
    .select({ count: sql<number>`count(*)::int` })
    .from(auditLog)
    .where(and(eq(auditLog.action, SUPPORT_ACTION), eq(auditLog.actorId, userId), gte(auditLog.createdAt, new Date(Date.now() - 3_600_000))));
  if ((sent?.count ?? 0) >= perHour) return "too_many";
  await audit(db, userId, SUPPORT_ACTION, { type: "user", id: userId }, { message, ...context });
  return "saved";
}

/**
 * Notes that a provider refused our account on a connector, once per `quietMinutes` a connector;
 * true when it was noted.
 */
export async function noteProviderProblem(
  db: Db,
  connectorId: string,
  details: { route: string; status: number; message?: string },
  quietMinutes = 30,
): Promise<boolean> {
  const [recent] = await db
    .select({ id: auditLog.id })
    .from(auditLog)
    .where(
      and(
        eq(auditLog.action, PROVIDER_PROBLEM_ACTION),
        eq(auditLog.targetId, connectorId),
        gte(auditLog.createdAt, new Date(Date.now() - quietMinutes * 60_000)),
      ),
    )
    .limit(1);
  if (recent) return false;
  await audit(db, null, PROVIDER_PROBLEM_ACTION, { type: "connector", id: connectorId }, { ...details });
  return true;
}

/** Support messages and provider problems, newest first. */
export async function supportEntries(db: Db, limit = 200) {
  return db
    .select({
      id: auditLog.id,
      action: auditLog.action,
      targetId: auditLog.targetId,
      data: auditLog.data,
      createdAt: auditLog.createdAt,
      userId: auditLog.actorId,
      email: user.email,
      name: user.name,
    })
    .from(auditLog)
    .leftJoin(user, eq(user.id, auditLog.actorId))
    .where(inArray(auditLog.action, [SUPPORT_ACTION, PROVIDER_PROBLEM_ACTION]))
    .orderBy(desc(auditLog.id))
    .limit(limit);
}
