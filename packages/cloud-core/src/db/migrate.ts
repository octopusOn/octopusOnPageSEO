// Migrations, kept apart from the client so bundlers (the portal) never see the folder lookup.

import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { migrate as migratePg } from "drizzle-orm/node-postgres/migrator";

import type { Db } from "./client.ts";

/** The migrations folder of this package. */
export function migrationsFolder(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "..", "..", "migrations");
}

/** Applies the migrations to a database opened with `connect`. */
export async function migrate(db: Db): Promise<void> {
  await migratePg(db as never, { migrationsFolder: migrationsFolder() });
}
