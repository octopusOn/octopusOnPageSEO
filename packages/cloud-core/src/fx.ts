// The USD/TRY rate credits are priced at: the Central Bank of Turkey's daily selling rate,
// cached in the settings table for a few hours.

import { eq } from "drizzle-orm";

import type { Db } from "./db/client.ts";
import { settings } from "./db/schema.ts";

export const TCMB_URL = "https://www.tcmb.gov.tr/kurlar/today.xml";
const FX_KEY = "fx";
const FRESH_MS = 6 * 3_600_000;
/** An older rate is still used when the bank cannot be reached (weekends, outages). */
const STALE_MS = 4 * 86_400_000;

export interface FxRate {
  /** TRY per USD (the bank's forex selling rate). */
  rate: number;
  /** The bank's bulletin date, `YYYY-MM-DD`. */
  date: string;
  fetchedAt: string;
}

/** The USD selling rate and date of a TCMB `today.xml` bulletin. */
export function parseTcmb(xml: string): { rate: number; date: string } {
  const usd = /<Currency[^>]*CurrencyCode="USD"[^>]*>([\s\S]*?)<\/Currency>/.exec(xml)?.[1];
  const selling = usd && /<ForexSelling>\s*([\d.]+)\s*<\/ForexSelling>/.exec(usd)?.[1];
  const rate = Number(selling);
  if (!selling || !Number.isFinite(rate) || rate <= 0) throw new Error("no USD selling rate in the TCMB bulletin");
  // Tarih_Date Tarih="18.10.2026" Date="10/18/2026"
  const day = /Tarih="(\d{2})\.(\d{2})\.(\d{4})"/.exec(xml);
  const date = day ? `${day[3]}-${day[2]}-${day[1]}` : new Date().toISOString().slice(0, 10);
  return { rate, date };
}

export async function storedRate(db: Db): Promise<FxRate | null> {
  const [row] = await db.select({ value: settings.value }).from(settings).where(eq(settings.key, FX_KEY));
  return (row?.value as FxRate | undefined) ?? null;
}

/** The rate to price with: cached when fresh, else fetched; an older one when the bank is down. */
export async function currentRate(
  db: Db,
  fetchFn: (url: string) => Promise<Response> = (url) => fetch(url, { signal: AbortSignal.timeout(10_000) }),
  now: number = Date.now(),
): Promise<FxRate> {
  const cached = await storedRate(db);
  const age = cached ? now - Date.parse(cached.fetchedAt) : Infinity;
  if (cached && age < FRESH_MS) return cached;
  try {
    const response = await fetchFn(TCMB_URL);
    if (!response.ok) throw new Error(`TCMB answered ${response.status}`);
    const parsed = parseTcmb(await response.text());
    const fresh: FxRate = { ...parsed, fetchedAt: new Date(now).toISOString() };
    await db
      .insert(settings)
      .values({ key: FX_KEY, value: fresh, updatedBy: "tcmb" })
      .onConflictDoUpdate({ target: settings.key, set: { value: fresh, updatedBy: "tcmb", updatedAt: new Date(now) } });
    return fresh;
  } catch (error) {
    if (cached && age < STALE_MS) return cached;
    throw error;
  }
}
