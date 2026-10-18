// The Log Analyser's modules as they appear in the sidebar.

import { BarChart3, Bot, CalendarRange, FileUp, GitCompare, LayoutDashboard, Settings2, ShieldBan } from "lucide-react";

import type { MessageKey } from "../i18n/en";
import type { Page } from "../state";

export interface NavItem {
  page: Page;
  label: MessageKey;
  icon: typeof BarChart3;
  /** Icon tile hue (`oc-tile <hue>`). */
  tile: "violet" | "blue" | "teal" | "orange" | "pink" | "cyan" | "amber" | "slate";
}

/** Modules that work on the selected site, in sidebar order. */
export const SITE_PAGES: NavItem[] = [
  { page: "overview", label: "nav.overview", icon: LayoutDashboard, tile: "violet" },
  { page: "reports", label: "nav.reports", icon: BarChart3, tile: "blue" },
  { page: "import", label: "nav.import", icon: FileUp, tile: "teal" },
  { page: "bots", label: "nav.bots", icon: Bot, tile: "orange" },
  { page: "robots", label: "nav.robots", icon: ShieldBan, tile: "pink" },
  { page: "urls", label: "nav.urls", icon: GitCompare, tile: "cyan" },
  { page: "periods", label: "nav.periods", icon: CalendarRange, tile: "amber" },
];

export const SETTINGS_PAGE: NavItem = { page: "settings", label: "nav.settings", icon: Settings2, tile: "slate" };

export const ALL_PAGES = [...SITE_PAGES, SETTINGS_PAGE];
