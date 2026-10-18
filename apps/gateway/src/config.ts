// Gateway settings from the environment.

import { masterKey } from "@octopusseo/cloud-core";

export interface GatewayConfig {
  databaseUrl: string;
  /** Opens our sealed provider credentials and device tokens. */
  masterKey: Buffer;
  port: number;
  /** Where people approve device sign-ins (`/cihaz` is appended). */
  portalUrl: string;
  /** Read the client address from the proxy's X-Forwarded-For (Caddy in front). */
  trustProxy: boolean;
  /** How often connector and settings changes from the admin panel are picked up. */
  refreshSeconds: number;
}

export function readConfig(env: NodeJS.ProcessEnv = process.env): GatewayConfig {
  const databaseUrl = env.DATABASE_URL;
  if (!databaseUrl) throw new Error("DATABASE_URL is not set");
  const port = Number(env.PORT ?? 8787);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) throw new Error(`invalid PORT ${env.PORT}`);
  const refreshSeconds = Number(env.CONNECTOR_REFRESH_SECONDS ?? 10);
  return {
    databaseUrl,
    masterKey: masterKey(env.OCTOPUS_MASTER_KEY),
    port,
    portalUrl: (env.PORTAL_URL ?? "https://my.octopuson.com").replace(/\/+$/, ""),
    trustProxy: env.TRUST_PROXY === "1" || env.TRUST_PROXY === "true",
    refreshSeconds: Number.isFinite(refreshSeconds) && refreshSeconds > 0 ? refreshSeconds : 10,
  };
}
