// Amounts are integers of micro-US-dollars (1 USD = 1,000,000), so sums never round.

/** Micro-dollars in one US dollar. */
export const MICROS_PER_USD = 1_000_000;

/** US dollars to micro-dollars, rounded up so a cost is never under-charged. */
export function usdToMicros(usd: number): number {
  if (!Number.isFinite(usd) || usd < 0) throw new RangeError(`invalid amount ${usd}`);
  return Math.ceil(Math.round(usd * MICROS_PER_USD * 1000) / 1000);
}

/** Micro-dollars to US dollars. */
export function microsToUsd(micros: number): number {
  return micros / MICROS_PER_USD;
}

/** Micro-dollars to credits of `unitMicros` each (two decimals). */
export function microsToCredits(micros: number, unitMicros: number): number {
  return Math.round((micros / unitMicros) * 100) / 100;
}

/** Credits to micro-dollars. */
export function creditsToMicros(credits: number, unitMicros: number): number {
  return Math.round(credits * unitMicros);
}

/** Turkish lira in kuruş, rounded to the nearest kuruş. */
export function tryToKurus(lira: number): number {
  return Math.round(lira * 100);
}
