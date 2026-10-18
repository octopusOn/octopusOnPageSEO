// Every page lives at /<language>/<address in that language>. The proxy:
// - sends addresses without a language (old links, the programs' /cihaz) to the remembered or
//   default language, translating their segments;
// - corrects segments written in another language or in English (/tr/credits → /tr/kredi);
// - tells the page its language and route in request headers (next.config rewrites the
//   address to the page's folder, app/[lang]/credits);
// - sends people without a session cookie to sign-in (the pages check the session themselves;
//   this only saves a round trip) and remembers the language of the address in a cookie.

import { getSessionCookie } from "better-auth/cookies";
import { NextResponse, type NextRequest } from "next/server";

import { catalog } from "./i18n/catalog";
import { LOCALE_COOKIE, LOCALE_HEADER, ROUTE_HEADER } from "./i18n/constants";
import { languages, preferredLanguage } from "./i18n/languages";
import { matchRoute, PUBLIC_ROUTES, routeUrl, type SegmentNames } from "./i18n/routes";

const YEAR = 60 * 60 * 24 * 365;

export async function proxy(request: NextRequest) {
  const known = await languages();
  const tables = new Map<string, SegmentNames>([...catalog()].map(([code, messages]) => [code, messages.routes]));
  const segments = request.nextUrl.pathname.split("/").filter(Boolean);
  // The programs' one-click sign-in (app/baglan) keeps its address in every language.
  if (segments.length === 1 && segments[0] === "baglan") return;
  const prefixed = known.list.some((language) => language.code === segments[0]);
  const lang = prefixed ? segments[0]! : preferredLanguage(request.cookies.get(LOCALE_COOKIE)?.value, known);
  const rest = prefixed ? segments.slice(1) : segments;
  const names = tables.get(lang) ?? {};
  const match = matchRoute(rest, names, [...tables.values()]);

  const redirect = (pathname: string, search = request.nextUrl.search) => {
    const url = request.nextUrl.clone();
    url.pathname = pathname;
    url.search = search;
    const response = NextResponse.redirect(url);
    response.headers.set("Vary", "Cookie");
    return response;
  };

  if (!match) return prefixed ? undefined : redirect(`/${[lang, ...rest].join("/")}`);
  const canonical = routeUrl(lang, names, match.route, match.params);
  if (!prefixed || canonical !== request.nextUrl.pathname) return redirect(canonical);

  if (!PUBLIC_ROUTES.includes(match.route) && !getSessionCookie(request)) {
    const next = match.route === "" ? "" : `?next=${encodeURIComponent(canonical + request.nextUrl.search)}`;
    return redirect(routeUrl(lang, names, "sign-in"), next);
  }

  const forwarded = new Headers(request.headers);
  forwarded.set(LOCALE_HEADER, lang);
  forwarded.set(ROUTE_HEADER, JSON.stringify(match));
  // Not NextResponse.rewrite: behind nginx (and in the standalone server) the URL the proxy
  // sees has another origin than the server's, so Next would fetch the rewrite as an external
  // address, through this proxy again.
  const response = NextResponse.next({ request: { headers: forwarded } });
  if (request.cookies.get(LOCALE_COOKIE)?.value !== lang) {
    response.cookies.set(LOCALE_COOKIE, lang, { path: "/", maxAge: YEAR, sameSite: "lax", secure: request.nextUrl.protocol === "https:" });
  }
  return response;
}

export const config = {
  // Pages only: not Next.js internals, API routes or files (icon.svg…).
  matcher: ["/((?!_next/|api/|.*\\.[a-z0-9]+$).*)"],
};
