"use client";

// Page addresses in the reader's language for client components.

import { createContext, type ReactNode, useContext } from "react";

import type { Url } from "./index";
import { routeUrl, type SegmentNames } from "./routes";

const I18n = createContext<{ locale: string; routes: SegmentNames }>({ locale: "", routes: {} });

export function I18nProvider({ locale, routes, children }: { locale: string; routes: SegmentNames; children: ReactNode }) {
  return <I18n.Provider value={{ locale, routes }}>{children}</I18n.Provider>;
}

export function useUrl(): Url {
  const { locale, routes } = useContext(I18n);
  return (route, params, query) => routeUrl(locale, routes, route, params, query);
}
