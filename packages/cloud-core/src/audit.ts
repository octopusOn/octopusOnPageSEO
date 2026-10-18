// Admin and security actions, kept for review.

import type { Db, Tx } from "./db/client.ts";
import { auditLog } from "./db/schema.ts";

export async function audit(
  db: Db | Tx,
  actorId: string | null,
  action: string,
  target: { type?: string; id?: string } = {},
  data: Record<string, unknown> = {},
): Promise<void> {
  await db.insert(auditLog).values({ actorId, action, targetType: target.type, targetId: target.id, data });
}
