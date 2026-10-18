// The reader's language and its texts for server components, actions and route handlers. The
// proxy (src/proxy.ts) passes the language of the address and the page in request headers.

import { cookies, headers } from "next/headers";

import { messagesFor, type Messages } from "./catalog";
import { LOCALE_COOKIE, LOCALE_HEADER, ROUTE_HEADER } from "./constants";
import { type Language, languages, preferredLanguage } from "./languages";
import { type Params, type Route, type RouteMatch, routeUrl } from "./routes";

export { LOCALE_COOKIE, LOCALE_HEADER, ROUTE_HEADER };

export type Dictionary = Messages;
/** A language code from the CMS: "tr", "en", "de"… */
export type Locale = string;
export type Query = Record<string, string | number | undefined>;
/** Builds a page's address in the reader's language. */
export type Url = (route: Route, params?: Params, query?: Query) => string;

export interface DictionaryContext {
  t: Dictionary;
  locale: Locale;
  /** BCP 47 tag: "tr-TR". */
  tag: string;
  languages: Language[];
  url: Url;
}

/** The address's language; outside the pages (API routes), the remembered or default one. */
export async function getLocale(): Promise<Locale> {
  const known = await languages();
  const fromAddress = (await headers()).get(LOCALE_HEADER);
  if (fromAddress && known.list.some((language) => language.code === fromAddress)) return fromAddress;
  return preferredLanguage((await cookies()).get(LOCALE_COOKIE)?.value, known);
}

export async function getDictionary(): Promise<DictionaryContext> {
  return dictionaryFor(await getLocale());
}

export async function dictionaryFor(locale: Locale): Promise<DictionaryContext> {
  const known = await languages();
  const t = messagesFor(locale, known.fallback);
  const tag = known.list.find((language) => language.code === locale)?.tag ?? t.language.tag;
  return {
    t,
    locale,
    tag,
    languages: known.list,
    url: (route, params, query) => routeUrl(locale, t.routes, route, params, query),
  };
}

/** The page being shown, as the proxy matched it, for the language menu. */
export async function currentRoute(): Promise<RouteMatch | null> {
  const value = (await headers()).get(ROUTE_HEADER);
  if (!value) return null;
  try {
    return JSON.parse(value) as RouteMatch;
  } catch {
    return null;
  }
}

/** Fills `{name}` placeholders. */
export function fill(text: string, values: Record<string, string | number>): string {
  return text.replace(/\{(\w+)\}/g, (match, key: string) => (key in values ? String(values[key]) : match));
}

export type { Language, Params, Route };
