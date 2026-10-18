// Service settings edited in the admin panel, with safe defaults.

import { eq } from "drizzle-orm";
import { z } from "zod";

import { audit } from "./audit.ts";
import type { Db } from "./db/client.ts";
import { settings } from "./db/schema.ts";

export const settingsSchema = z.object({
  /** Markup on upstream costs unless a connector sets its own (0.35 = 35%). */
  markup: z.number().min(0).max(10).default(0.35),
  /** Micro-dollars in one credit as shown to users (1000 = 0.001 USD). */
  creditUnitMicros: z.number().int().min(1).max(1_000_000).default(1000),
  /** Added to the day's USD/TRY rate when credits are priced in lira (3 = 3%). */
  fxBufferPercent: z.number().min(0).max(50).default(3),
  /** VAT on credit purchases. */
  vatRate: z.number().min(0).max(1).default(0.2),
  /** Credit packages in kuruş (VAT included). */
  packagesKurus: z.array(z.number().int().positive()).default([25_000, 50_000, 100_000, 250_000]),
  minPurchaseKurus: z.number().int().positive().default(10_000),
  /** The largest first purchase of a new account. */
  firstPurchaseCapKurus: z.number().int().positive().default(250_000),
  /** Credits given at sign-up (0 = none). */
  signupBonusMicros: z.number().int().min(0).default(0),
  /** An e-mail goes out when the balance falls below this. */
  lowBalanceMicros: z.number().int().min(0).default(1_000_000),
  /** Holds older than this are released (a gateway that stopped mid-request). */
  holdTtlMinutes: z.number().int().min(1).max(1440).default(15),
  /** Spending limit per user and day, 0 for none. */
  dailyLimitMicros: z.number().int().min(0).default(50_000_000),
});

export type CloudSettings = z.infer<typeof settingsSchema>;
export const DEFAULT_SETTINGS: CloudSettings = settingsSchema.parse({});

/** Current settings: stored values over the defaults. */
export async function getSettings(db: Db): Promise<CloudSettings> {
  const rows = await db.select().from(settings);
  const stored = Object.fromEntries(rows.map((row) => [row.key, row.value]));
  return settingsSchema.parse({ ...DEFAULT_SETTINGS, ...stored });
}

/** Changes settings; unknown keys and invalid values are refused. */
export async function updateSettings(db: Db, changes: Partial<CloudSettings>, actorId: string): Promise<CloudSettings> {
  const next = settingsSchema.parse({ ...(await getSettings(db)), ...changes });
  for (const key of Object.keys(changes) as (keyof CloudSettings)[]) {
    const value = next[key];
    const [row] = await db.select({ key: settings.key }).from(settings).where(eq(settings.key, key));
    if (row) await db.update(settings).set({ value, updatedBy: actorId }).where(eq(settings.key, key));
    else await db.insert(settings).values({ key, value, updatedBy: actorId });
  }
  await audit(db, actorId, "settings.update", { type: "settings" }, changes);
  return next;
}
