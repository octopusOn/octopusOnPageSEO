// Usable connectors, our opened credentials and the service settings, reloaded every few
// seconds so admin-panel changes (an emergency switch-off too) reach every request quickly.

import {
  type CloudSettings,
  type Connector,
  connectorSecretValues,
  type Db,
  getSettings,
  usableConnectors,
} from "@octopusseo/cloud-core";

export interface LoadedConnector {
  connector: Connector;
  secrets: Record<string, string>;
}

interface Snapshot {
  connectors: Map<string, LoadedConnector>;
  settings: CloudSettings;
  loadedAt: number;
}

export class Registry {
  readonly #db: Db;
  readonly #key: Buffer;
  readonly #ttlMs: number;
  readonly #now: () => number;
  #snapshot: Snapshot | null = null;
  #loading: Promise<Snapshot> | null = null;

  constructor(db: Db, key: Buffer, ttlMs: number, now: () => number = Date.now) {
    this.#db = db;
    this.#key = key;
    this.#ttlMs = ttlMs;
    this.#now = now;
  }

  async connector(id: string): Promise<LoadedConnector | undefined> {
    return (await this.#current()).connectors.get(id);
  }

  async connectors(): Promise<LoadedConnector[]> {
    return [...(await this.#current()).connectors.values()];
  }

  async settings(): Promise<CloudSettings> {
    return (await this.#current()).settings;
  }

  /** Forgets the snapshot, so the next request reloads. */
  invalidate(): void {
    this.#snapshot = null;
  }

  async #current(): Promise<Snapshot> {
    const snapshot = this.#snapshot;
    if (snapshot && this.#now() - snapshot.loadedAt < this.#ttlMs) return snapshot;
    this.#loading ??= this.#load().finally(() => {
      this.#loading = null;
    });
    try {
      return await this.#loading;
    } catch (error) {
      // A database hiccup keeps the last good snapshot instead of failing every request.
      if (snapshot) return snapshot;
      throw error;
    }
  }

  async #load(): Promise<Snapshot> {
    const [connectors, settings] = await Promise.all([usableConnectors(this.#db), getSettings(this.#db)]);
    const loaded = new Map<string, LoadedConnector>();
    for (const connector of connectors) {
      loaded.set(connector.id, { connector, secrets: await connectorSecretValues(this.#db, this.#key, connector.id) });
    }
    this.#snapshot = { connectors: loaded, settings, loadedAt: this.#now() };
    return this.#snapshot;
  }
}
