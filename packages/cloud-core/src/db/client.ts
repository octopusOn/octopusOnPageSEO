// Database connections: PostgreSQL in production, PGlite (PostgreSQL in WebAssembly) in tests.

import { drizzle as drizzlePg } from "drizzle-orm/node-postgres";
import type { PgDatabase, PgQueryResultHKT } from "drizzle-orm/pg-core";
import pg from "pg";

import * as schema from "./schema.ts";

/** A database with this schema, whatever the driver. */
export type Db = PgDatabase<PgQueryResultHKT, typeof schema>;

/** A transaction of such a database. */
export type Tx = Parameters<Parameters<Db["transaction"]>[0]>[0];

/** A pooled connection to `url` (`postgres://…`). */
export function connect(url: string, max = 10): { db: Db; close: () => Promise<void> } {
  const pool = new pg.Pool({ connectionString: url, max });
  return { db: drizzlePg(pool, { schema }) as unknown as Db, close: () => pool.end() };
}

export { schema };
