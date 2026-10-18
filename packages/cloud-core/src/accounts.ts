// What the apps and the portal show about a signed-in person.

import { eq } from "drizzle-orm";

import type { Db } from "./db/client.ts";
import { user } from "./db/schema.ts";

export async function accountOf(db: Db, userId: string): Promise<{ email: string; name: string } | null> {
  const [row] = await db.select({ email: user.email, name: user.name }).from(user).where(eq(user.id, userId));
  return row ?? null;
}
