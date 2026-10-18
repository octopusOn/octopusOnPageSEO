// The language: an earlier choice (cookie), else the browser's, else Turkish.

import { cookies, headers } from "next/headers";

import { en } from "./en";
import { type Dictionary, tr } from "./tr";

import { LOCALE_COOKIE, type Locale, pickLocale } from "./pick";

export { LOCALE_COOKIE, type Locale, pickLocale };

export async function getLocale(): Promise<Locale> {
  const [jar, list] = await Promise.all([cookies(), headers()]);
  return pickLocale(jar.get(LOCALE_COOKIE)?.value, list.get("accept-language"));
}

export async function getDictionary(): Promise<{ t: Dictionary; locale: Locale }> {
  const locale = await getLocale();
  return { t: locale === "en" ? en : tr, locale };
}

/** Fills `{name}` placeholders. */
export function fill(text: string, values: Record<string, string | number>): string {
  return text.replace(/\{(\w+)\}/g, (match, key: string) => (key in values ? String(values[key]) : match));
}

export type { Dictionary };
