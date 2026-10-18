// An in-memory database for tests: PGlite with every migration applied.

import { PGlite } from "@electric-sql/pglite";
import { drizzle } from "drizzle-orm/pglite";
import { migrate } from "drizzle-orm/pglite/migrator";

import type { Db } from "./db/client.ts";
import { migrationsFolder } from "./db/migrate.ts";

/** The query helpers of this package's drizzle-orm, for tests in other packages. */
export { and, eq } from "drizzle-orm";
import * as schema from "./db/schema.ts";

/** A fresh database and a function that closes it. */
export async function testDb(): Promise<{ db: Db; close: () => Promise<void> }> {
  const client = new PGlite();
  const db = drizzle(client, { schema });
  await migrate(db, { migrationsFolder: migrationsFolder() });
  return { db: db as unknown as Db, close: () => client.close() };
}

/** Adds a user (as Better Auth would) and returns its id. */
export async function addUser(db: Db, email: string, role: "user" | "admin" = "user"): Promise<string> {
  const id = `user-${email}`;
  await db.insert(schema.user).values({ id, name: email.split("@")[0] ?? email, email, emailVerified: true, role });
  return id;
}
