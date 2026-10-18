// One pooled connection per server process (kept across hot reloads in development).

import { connect, type Db } from "@octopusseo/cloud-core";

const cache = globalThis as { octopusDb?: Db };

export function db(): Db {
  cache.octopusDb ??= connect(process.env.DATABASE_URL ?? "postgres://localhost:5432/octopus", 10).db;
  return cache.octopusDb;
}

export function masterKeyBase64(): string | undefined {
  return process.env.OCTOPUS_MASTER_KEY;
}
