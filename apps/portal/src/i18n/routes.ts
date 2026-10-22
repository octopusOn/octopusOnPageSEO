// The portal's pages by route: the folders under app/[lang], named in English. Each language's
// messages translate the segments (`routes` in messages/<code>.json), so an address reads in its
// language: /tr/kredi/onay and /en/credits/confirm are the same page. The proxy corrects the
// address people open; next.config rewrites the corrected address to the folder.

export const ROUTES = [
  "",
  "sign-in",
  "sign-in/two-step",
  "sign-up",
  "password",
  "password/new",
  "verify",
  "credits",
  "credits/confirm",
  "credits/result",
  "credits/payment/[id]",
  "usage",
  "account",
  "tokens",
  "device",
  "invoices",
  "invoices/[id]/pdf",
  "admin",
  "admin/connectors",
  "admin/connectors/new",
  "admin/connectors/[id]",
  "admin/users",
  "admin/users/[id]",
  "admin/orders",
  "admin/orders/csv",
  "admin/settings",
  "admin/reports",
  "admin/audit",
  "admin/support",
  "legal/[slug]",
] as const;

export type Route = (typeof ROUTES)[number];

/** Pages people open without signing in. */
export const PUBLIC_ROUTES: readonly Route[] = ["sign-in", "sign-in/two-step", "sign-up", "password", "password/new", "verify", "legal/[slug]"];

/** A language's names for route segments: `{ "credits": "kredi" }`. Missing ones keep the English name. */
export type SegmentNames = Record<string, string>;

export type Params = Record<string, string>;

export interface RouteMatch {
  route: Route;
  params: Params;
}

const paramName = (segment: string) => /^\[(\w+)\]$/.exec(segment)?.[1];

/** The address of `route` in a language: `/tr/kredi/odeme/42?tutar=5000`. */
export function routeUrl(code: string, names: SegmentNames, route: Route, params: Params = {}, query?: Record<string, string | number | undefined>): string {
  const segments = route === "" ? [] : route.split("/").map((segment) => {
    const param = paramName(segment);
    return param ? encodeURIComponent(params[param] ?? "") : (names[segment] ?? segment);
  });
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) if (value !== undefined && value !== "") search.set(key, String(value));
  const tail = search.size ? `?${search}` : "";
  return `/${[code, ...segments].join("/")}${tail}`;
}

/**
 * next.config rewrites from every language's addresses to the page folders, with parameters as
 * `:id`: `/tr/kredi/odeme/:id` → `/tr/credits/payment/:id`. More specific routes come first.
 */
export function localizedRewrites(languages: Iterable<[string, SegmentNames]>): { source: string; destination: string }[] {
  const rewrites: { source: string; destination: string }[] = [];
  for (const [code, names] of languages) {
    for (const route of ROUTES) {
      if (route === "") continue;
      const path = (translate: boolean) =>
        `/${code}/${route
          .split("/")
          .map((segment) => {
            const param = paramName(segment);
            return param ? `:${param}` : translate ? (names[segment] ?? segment) : segment;
          })
          .join("/")}`;
      const source = path(true);
      const destination = path(false);
      if (source !== destination) rewrites.push({ source, destination });
    }
  }
  return rewrites;
}

/**
 * The page `segments` (the address after the language) point at. A segment may be named in
 * `names` (the address's language), in any of `others` (an old or switched address) or by its
 * English route name.
 */
export function matchRoute(segments: string[], names: SegmentNames, others: SegmentNames[] = []): RouteMatch | null {
  for (const route of ROUTES) {
    const pattern = route === "" ? [] : route.split("/");
    if (pattern.length !== segments.length) continue;
    const params: Params = {};
    const fits = pattern.every((segment, i) => {
      const given = segments[i] ?? "";
      const param = paramName(segment);
      if (param) {
        try {
          params[param] = decodeURIComponent(given);
        } catch {
          return false;
        }
        return given !== "";
      }
      return given === segment || names[segment] === given || others.some((table) => table[segment] === given);
    });
    if (fits) return { route, params };
  }
  return null;
}
