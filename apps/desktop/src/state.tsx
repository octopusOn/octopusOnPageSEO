// Application-wide selection: current site and page.

import { createContext, useContext, useMemo, type ReactNode } from "react";

import { useStored } from "./lib/storage";

export type Page = "overview" | "reports" | "import" | "bots" | "robots" | "urls" | "periods" | "settings";

interface AppState {
  site: string | null;
  setSite: (site: string | null) => void;
  page: Page;
  setPage: (page: Page) => void;
}

const AppContext = createContext<AppState | null>(null);

export function AppStateProvider({ children }: { children: ReactNode }) {
  const [site, setSite] = useStored<string | null>("site", null);
  const [page, setPage] = useStored<Page>("page", "overview");
  const value = useMemo(() => ({ site, setSite, page, setPage }), [site, setSite, page, setPage]);
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useAppState(): AppState {
  const context = useContext(AppContext);
  if (!context) throw new Error("useAppState outside AppStateProvider");
  return context;
}

/** React Query keys; everything about a site starts with `["site", name]`. */
export const keys = {
  clients: ["clients"] as const,
  sites: ["sites"] as const,
  templates: ["templates"] as const,
  saved: ["saved-queries"] as const,
  formats: ["formats"] as const,
  appInfo: ["app-info"] as const,
  account: ["account"] as const,
  bots: ["bots"] as const,
  customBots: ["custom-bots"] as const,
  geo: ["geo"] as const,
  ranges: ["bot-ranges"] as const,
  site: (site: string) => ["site", site] as const,
  overview: (site: string) => ["site", site, "overview"] as const,
  settings: (site: string) => ["site", site, "settings"] as const,
  crawl: (site: string, period: string | null) => ["site", site, "crawl", period] as const,
  query: (site: string, text: string) => ["site", site, "query", text] as const,
};
