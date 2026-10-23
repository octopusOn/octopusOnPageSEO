// The page, project and what the page is looking at in the address
// (`#/overview?project=2&domain=example.com&tab=keywords`), so the web version can be bookmarked
// and reloaded, any keyword, domain or address can link to another module's report on it, and
// the browser's back button goes back to the previous one. The desktop window uses the same.

import { useCallback, useEffect, useState } from "react";

export const PAGES = [
  "dashboard",
  "project",
  "overview",
  "keywords",
  "backlinks",
  "gap",
  "content",
  "rank",
  "local",
  "audit",
  "ai",
  "gsc",
  "snapshots",
  "reports",
  "sources",
  "settings",
] as const;

export type PageId = (typeof PAGES)[number];

/** What a page is looking at, beside the project. */
export interface RouteParams {
  /** A domain (Site Explorer, Backlinks). */
  domain?: string;
  /** A keyword (Keyword Explorer). */
  keyword?: string;
  /** An address (a page's keywords or backlinks). */
  url?: string;
  /** The tab shown. */
  tab?: string;
  /** Another market than the project's: `country-language` (`de-de`). */
  market?: string;
  /** A linking site whose links to the target are shown (Backlinks). */
  source?: string;
  /** A content brief shown (Content). */
  brief?: string;
}

/** Address fields kept, in the order they are written. */
const PARAMS: (keyof RouteParams)[] = ["domain", "keyword", "url", "tab", "market", "source", "brief"];

/** Longest value kept from the address. */
const MAX_PARAM = 500;

export interface Route {
  page: PageId;
  project: number | null;
  params: RouteParams;
}

export function parseHash(hash: string): Route {
  const [path = "", query = ""] = hash.replace(/^#\/?/, "").split("?");
  const page = (PAGES as readonly string[]).includes(path) ? (path as PageId) : "dashboard";
  const search = new URLSearchParams(query);
  const project = Number(search.get("project"));
  const params: RouteParams = {};
  for (const name of PARAMS) {
    const value = search.get(name)?.trim();
    if (value) params[name] = value.slice(0, MAX_PARAM);
  }
  return { page, project: Number.isInteger(project) && project > 0 ? project : null, params };
}

export function hashOf(route: Route): string {
  const search = new URLSearchParams();
  if (route.project !== null) search.set("project", String(route.project));
  for (const name of PARAMS) {
    const value = route.params[name];
    if (value) search.set(name, value);
  }
  const query = search.toString();
  return query ? `#/${route.page}?${query}` : `#/${route.page}`;
}

/** Where to go: a page change starts with no parameters unless they are given. */
export interface RouteChange {
  page?: PageId;
  project?: number | null;
  params?: RouteParams;
}

/** Pages whose reports can be asked for another market; a chosen one goes along between them. */
const MARKET_PAGES: readonly PageId[] = ["overview", "keywords", "gap"];

/**
 * The route after a change: parameters are replaced as given, kept only on the same page. A
 * chosen market goes along to another market page of the same project unless the change says
 * otherwise.
 */
export function nextRoute(current: Route, next: RouteChange): Route {
  const page = next.page ?? current.page;
  const sameProject = next.project === undefined || next.project === current.project;
  const samePage = page === current.page && sameProject;
  const params = next.params ?? (samePage ? current.params : {});
  const market = current.params.market;
  const carried = market && sameProject && !("market" in params) && MARKET_PAGES.includes(page) && MARKET_PAGES.includes(current.page) ? { ...params, market } : params;
  return {
    page,
    project: next.project === undefined ? current.project : next.project,
    params: carried,
  };
}

export function useRoute(): [Route, (next: RouteChange) => void] {
  const [route, setRoute] = useState<Route>(() => parseHash(window.location.hash));
  useEffect(() => {
    const changed = () => setRoute(parseHash(window.location.hash));
    window.addEventListener("hashchange", changed);
    return () => window.removeEventListener("hashchange", changed);
  }, []);
  const go = useCallback((next: RouteChange) => {
    const target = nextRoute(parseHash(window.location.hash), next);
    const hash = hashOf(target);
    if (hash !== window.location.hash) window.location.hash = hash;
    setRoute(target);
  }, []);
  return [route, go];
}
