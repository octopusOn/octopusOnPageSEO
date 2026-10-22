// The connector store: every change is a new version, our upstream credentials are sealed,
// and every action lands in the audit log. A credential can also come from the environment
// (CONNECTOR_<ID>_<NAME>, e.g. CONNECTOR_DATAFORSEO_CREDENTIALS), which wins over a stored one.

import { and, desc, eq, inArray, max } from "drizzle-orm";

import { audit } from "./audit.ts";
import { type Connector, type ConnectorInput, type ConnectorStatus, connectorSchema, secretNames } from "./connector.ts";
import { open, seal } from "./crypto.ts";
import type { Db } from "./db/client.ts";
import { connectorSecrets, connectorVersions, connectors } from "./db/schema.ts";

export interface ConnectorRecord {
  connector: Connector;
  version: number;
  updatedAt: Date;
  /** Secret names that are set. */
  secrets: string[];
  /** Secret names the configuration needs but that are not set. */
  missingSecrets: string[];
  /** Secret names the environment provides (these win over stored ones). */
  environment: string[];
}

type Environment = Record<string, string | undefined>;

/** The environment variable that can hold a connector's secret: CONNECTOR_DATAFORSEO_CREDENTIALS. */
export function secretVariable(connectorId: string, name: string): string {
  return `CONNECTOR_${connectorId}_${name}`.toUpperCase().replace(/[^A-Z0-9]/g, "_");
}

/** The connector's secrets set in the environment, by secret name (lower case). */
export function environmentSecrets(connectorId: string, env: Environment = process.env): Record<string, string> {
  const prefix = secretVariable(connectorId, "");
  return Object.fromEntries(
    Object.entries(env)
      .filter((entry): entry is [string, string] => entry[0].startsWith(prefix) && entry[0].length > prefix.length && Boolean(entry[1]?.trim()))
      .map(([variable, value]) => [variable.slice(prefix.length).toLowerCase(), value.trim()]),
  );
}

/** Validates and saves a configuration as the connector's new version. */
export async function saveConnector(db: Db, input: ConnectorInput, actorId: string): Promise<Connector> {
  const connector = connectorSchema.parse(input);
  await db.transaction(async (tx) => {
    const [last] = await tx
      .select({ version: max(connectorVersions.version) })
      .from(connectorVersions)
      .where(eq(connectorVersions.connectorId, connector.id));
    const version = (last?.version ?? 0) + 1;
    if (version === 1) {
      await tx.insert(connectors).values({ id: connector.id, status: connector.status, currentVersion: 1 });
    } else {
      await tx
        .update(connectors)
        .set({ status: connector.status, currentVersion: version, updatedAt: new Date() })
        .where(eq(connectors.id, connector.id));
    }
    await tx.insert(connectorVersions).values({ connectorId: connector.id, version, config: connector, createdBy: actorId });
    await audit(tx, actorId, "connector.save", { type: "connector", id: connector.id }, { version, status: connector.status });
  });
  return connector;
}

/** Changes only the status; the result is a new version, validated like any other. */
export async function setConnectorStatus(db: Db, id: string, status: ConnectorStatus, actorId: string): Promise<Connector> {
  const current = await getConnector(db, id);
  if (!current) throw new Error(`no connector ${id}`);
  return saveConnector(db, { ...current, status }, actorId);
}

/** The current configuration of a connector. */
export async function getConnector(db: Db, id: string): Promise<Connector | null> {
  const [row] = await db
    .select({ config: connectorVersions.config })
    .from(connectors)
    .innerJoin(
      connectorVersions,
      and(eq(connectorVersions.connectorId, connectors.id), eq(connectorVersions.version, connectors.currentVersion)),
    )
    .where(eq(connectors.id, id));
  return row ? connectorSchema.parse(row.config) : null;
}

/** Every connector with its secret state, for the admin panel. */
export async function listConnectors(db: Db, env: Environment = process.env): Promise<ConnectorRecord[]> {
  const rows = await db
    .select({ config: connectorVersions.config, version: connectors.currentVersion, updatedAt: connectors.updatedAt })
    .from(connectors)
    .innerJoin(
      connectorVersions,
      and(eq(connectorVersions.connectorId, connectors.id), eq(connectorVersions.version, connectors.currentVersion)),
    )
    .orderBy(connectors.id);
  const stored = await db.select({ connectorId: connectorSecrets.connectorId, name: connectorSecrets.name }).from(connectorSecrets);
  return rows.map((row) => {
    const connector = connectorSchema.parse(row.config);
    const secrets = stored.filter((secret) => secret.connectorId === connector.id).map((secret) => secret.name);
    const environment = secretNames(connector).filter((name) => name.toLowerCase() in environmentSecrets(connector.id, env));
    return {
      connector,
      version: row.version,
      updatedAt: row.updatedAt,
      secrets,
      missingSecrets: secretNames(connector).filter((name) => !secrets.includes(name) && !environment.includes(name)),
      environment,
    };
  });
}

/** Connectors that can be used: active, plus those in test (for admins). */
export async function usableConnectors(db: Db): Promise<Connector[]> {
  const rows = await db
    .select({ config: connectorVersions.config })
    .from(connectors)
    .innerJoin(
      connectorVersions,
      and(eq(connectorVersions.connectorId, connectors.id), eq(connectorVersions.version, connectors.currentVersion)),
    )
    .where(inArray(connectors.status, ["active", "test"]))
    .orderBy(connectors.id);
  return rows.map((row) => connectorSchema.parse(row.config));
}

/** Earlier versions, newest first. */
export async function connectorHistory(db: Db, id: string) {
  return db
    .select({
      version: connectorVersions.version,
      config: connectorVersions.config,
      createdBy: connectorVersions.createdBy,
      createdAt: connectorVersions.createdAt,
    })
    .from(connectorVersions)
    .where(eq(connectorVersions.connectorId, id))
    .orderBy(desc(connectorVersions.version));
}

/** Seals and stores one of our upstream credentials. The value is never logged. */
export async function setConnectorSecret(db: Db, key: Buffer, connectorId: string, name: string, value: string, actorId: string): Promise<void> {
  const trimmed = value.trim();
  if (!trimmed) throw new Error("empty secret");
  const sealed = seal(key, trimmed);
  await db
    .insert(connectorSecrets)
    .values({ connectorId, name, sealed })
    .onConflictDoUpdate({ target: [connectorSecrets.connectorId, connectorSecrets.name], set: { sealed, updatedAt: new Date() } });
  await audit(db, actorId, "connector.secret.set", { type: "connector", id: connectorId }, { name });
}

/** Removes one of our upstream credentials. */
export async function removeConnectorSecret(db: Db, connectorId: string, name: string, actorId: string): Promise<void> {
  await db
    .delete(connectorSecrets)
    .where(and(eq(connectorSecrets.connectorId, connectorId), eq(connectorSecrets.name, name)));
  await audit(db, actorId, "connector.secret.remove", { type: "connector", id: connectorId }, { name });
}

/** The opened credentials of a connector, by name; those in the environment win. */
export async function connectorSecretValues(db: Db, key: Buffer, connectorId: string, env: Environment = process.env): Promise<Record<string, string>> {
  const rows = await db
    .select({ name: connectorSecrets.name, sealed: connectorSecrets.sealed })
    .from(connectorSecrets)
    .where(eq(connectorSecrets.connectorId, connectorId));
  return { ...Object.fromEntries(rows.map((row) => [row.name, open(key, row.sealed)])), ...environmentSecrets(connectorId, env) };
}
