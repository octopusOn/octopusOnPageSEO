// Numbers, money and dates in the reader's language.

import { microsToCredits } from "@octopusseo/cloud-core";

import type { Locale } from "@/i18n/pick";

const tag = (locale: Locale) => (locale === "tr" ? "tr-TR" : "en-US");

export function formatCredits(micros: number, unitMicros: number, locale: Locale): string {
  return new Intl.NumberFormat(tag(locale), { maximumFractionDigits: 2 }).format(microsToCredits(micros, unitMicros));
}

export function formatUsd(micros: number, locale: Locale, digits = 2): string {
  return new Intl.NumberFormat(tag(locale), {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: Math.max(digits, 2),
  }).format(micros / 1_000_000);
}

export function formatTry(kurus: number, locale: Locale): string {
  return new Intl.NumberFormat(tag(locale), { style: "currency", currency: "TRY", maximumFractionDigits: 2 }).format(kurus / 100);
}

/** An exchange rate with four decimals. */
export function formatRate(value: number, locale: Locale): string {
  return new Intl.NumberFormat(tag(locale), { minimumFractionDigits: 4, maximumFractionDigits: 4 }).format(value);
}

export function formatNumber(value: number, locale: Locale): string {
  return new Intl.NumberFormat(tag(locale)).format(value);
}

/** The time zone tables use: Turkey unless the server is told otherwise. */
const TIME_ZONE = process.env.PORTAL_TIME_ZONE || "Europe/Istanbul";
const TABLE_TIME = new Intl.DateTimeFormat("sv-SE", {
  timeZone: TIME_ZONE,
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

/** ISO-style date and time to the minute (`2026-10-18 16:37`), as tables show them. */
export function formatTime(value: Date | string | null | undefined): string {
  if (!value) return "—";
  return TABLE_TIME.format(typeof value === "string" ? new Date(value) : value);
}

/** A number as typed in the page's language: Turkish "1.234,5", English "1,234.5". */
export function parseDecimal(text: string, locale: Locale): number | null {
  const compact = text.trim().replace(/\s/g, "");
  if (!compact) return null;
  const normalized = locale === "tr" ? compact.replace(/\./g, "").replace(",", ".") : compact.replace(/,/g, "");
  if (!/^-?\d*\.?\d+$/.test(normalized)) return null;
  return Number(normalized);
}

/** A credits amount as typed to micro-dollars. */
export function parseCredits(text: string, unitMicros: number, locale: Locale): number | null {
  const value = parseDecimal(text, locale);
  return value === null ? null : Math.round(value * unitMicros);
}

/** A number for a form field, written the way {@link parseDecimal} reads it back. */
export function plainNumber(value: number, locale: Locale): string {
  return new Intl.NumberFormat(tag(locale), { useGrouping: false, maximumFractionDigits: 6 }).format(value);
}
