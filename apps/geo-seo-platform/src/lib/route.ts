// The page and project in the address (`#/overview?project=2`), so the web version can be
// bookmarked and reloaded; the desktop window uses the same.

import { useCallback, useEffect, useState } from "react";

export const PAGES = [
  "dashboard",
  "overview",
  "keywords",
  "backlinks",
  "gap",
  "rank",
  "audit",
  "ai",
  "gsc",
  "reports",
  "sources",
  "settings",
] as const;

export type PageId = (typeof PAGES)[number];

export interface Route {
  page: PageId;
  project: number | null;
}

export function parseHash(hash: string): Route {
  const [path = "", query = ""] = hash.replace(/^#\/?/, "").split("?");
  const page = (PAGES as readonly string[]).includes(path) ? (path as PageId) : "dashboard";
  const project = Number(new URLSearchParams(query).get("project"));
  return { page, project: Number.isInteger(project) && project > 0 ? project : null };
}

export function hashOf(route: Route): string {
  return route.project === null ? `#/${route.page}` : `#/${route.page}?project=${route.project}`;
}

export function useRoute(): [Route, (next: Partial<Route>) => void] {
  const [route, setRoute] = useState<Route>(() => parseHash(window.location.hash));
  useEffect(() => {
    const changed = () => setRoute(parseHash(window.location.hash));
    window.addEventListener("hashchange", changed);
    return () => window.removeEventListener("hashchange", changed);
  }, []);
  const go = useCallback((next: Partial<Route>) => {
    const target = { ...parseHash(window.location.hash), ...next };
    const hash = hashOf(target);
    if (hash !== window.location.hash) window.location.hash = hash;
    setRoute(target);
  }, []);
  return [route, go];
}
