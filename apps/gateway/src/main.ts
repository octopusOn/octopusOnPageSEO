// The gateway process: `node src/main.ts` with DATABASE_URL and OCTOPUS_MASTER_KEY set.

import { serve } from "@hono/node-server";
import { connect, getSettings, releaseStaleHolds } from "@octopusseo/cloud-core";

import { createApp } from "./app.ts";
import { readConfig } from "./config.ts";
import { Concurrency, RateLimiter } from "./limits.ts";
import { Registry } from "./registry.ts";

const config = readConfig();
const { db, close } = connect(config.databaseUrl, 20);
const log = (event: Record<string, unknown>) => console.log(JSON.stringify({ at: new Date().toISOString(), ...event }));
const concurrency = new Concurrency();
const app = createApp({
  db,
  key: config.masterKey,
  portalUrl: config.portalUrl,
  trustProxy: config.trustProxy,
  registry: new Registry(db, config.masterKey, config.refreshSeconds * 1000),
  fetch: (request) => fetch(request),
  limiter: new RateLimiter(),
  concurrency,
  log,
});

const server = serve({ fetch: app.fetch, port: config.port }, (info) => log({ event: "listening", port: info.port }));

// Holds left by a request that never finished (a crash or a restart) are freed without a charge.
const sweeper = setInterval(async () => {
  try {
    const { holdTtlMinutes } = await getSettings(db);
    const released = await releaseStaleHolds(db, holdTtlMinutes * 60_000);
    if (released > 0) log({ level: "warn", event: "stale_holds_released", count: released });
  } catch (error) {
    log({ level: "error", event: "sweep_failed", message: error instanceof Error ? error.message : String(error) });
  }
}, 60_000);

async function shutdown(signal: string): Promise<void> {
  log({ event: "stopping", signal });
  clearInterval(sweeper);
  server.close();
  const deadline = Date.now() + 30_000;
  while (concurrency.total > 0 && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 200));
  await close();
  process.exit(0);
}
process.on("SIGTERM", () => void shutdown("SIGTERM"));
process.on("SIGINT", () => void shutdown("SIGINT"));
