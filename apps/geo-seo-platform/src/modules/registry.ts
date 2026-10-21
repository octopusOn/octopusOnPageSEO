// Every module of the platform in the sidebar. A module that is not built yet shows "soon"; each
// milestone switches its own entry on.

import {
  ChartLine,
  FileText,
  GitCompareArrows,
  Globe,
  KeyRound,
  LayoutDashboard,
  Link2,
  PlugZap,
  ScanSearch,
  Settings2,
  Sparkles,
  TrendingUp,
  type LucideIcon,
} from "lucide-react";
import type { PageId } from "../lib/route";

export type Group = "main" | "research" | "tracking" | "output" | "setup";

export interface ModuleEntry {
  id: PageId;
  /** `common.nav…` */
  label: string;
  /** `common.lead…` */
  lead: string;
  icon: LucideIcon;
  /** Glossy tile colour (`oc-tile <tone>`). */
  tone: string;
  group: Group;
  /** Built in this version. */
  ready: boolean;
  /** Works on a project. */
  project: boolean;
}

export const MODULES: ModuleEntry[] = [
  { id: "dashboard", label: "common.navDashboard", lead: "common.leadDashboard", icon: LayoutDashboard, tone: "slate", group: "main", ready: true, project: false },
  { id: "overview", label: "common.navOverview", lead: "common.leadOverview", icon: Globe, tone: "blue", group: "research", ready: true, project: true },
  { id: "keywords", label: "common.navKeywords", lead: "common.leadKeywords", icon: KeyRound, tone: "teal", group: "research", ready: true, project: true },
  { id: "backlinks", label: "common.navBacklinks", lead: "common.leadBacklinks", icon: Link2, tone: "violet", group: "research", ready: true, project: true },
  { id: "gap", label: "common.navGap", lead: "common.leadGap", icon: GitCompareArrows, tone: "orange", group: "research", ready: true, project: true },
  { id: "rank", label: "common.navRank", lead: "common.leadRank", icon: TrendingUp, tone: "cyan", group: "tracking", ready: true, project: true },
  { id: "audit", label: "common.navAudit", lead: "common.leadAudit", icon: ScanSearch, tone: "amber", group: "tracking", ready: true, project: true },
  { id: "ai", label: "common.navAi", lead: "common.leadAi", icon: Sparkles, tone: "pink", group: "tracking", ready: true, project: true },
  { id: "gsc", label: "common.navGsc", lead: "common.leadGsc", icon: ChartLine, tone: "blue", group: "tracking", ready: true, project: true },
  { id: "reports", label: "common.navReports", lead: "common.leadReports", icon: FileText, tone: "violet", group: "output", ready: true, project: true },
  { id: "sources", label: "common.navSources", lead: "common.leadSources", icon: PlugZap, tone: "amber", group: "setup", ready: true, project: false },
  { id: "settings", label: "common.navSettings", lead: "common.leadSettings", icon: Settings2, tone: "slate", group: "setup", ready: true, project: false },
];

export const GROUPS: { id: Group; label: string | null }[] = [
  { id: "main", label: null },
  { id: "research", label: "common.groupResearch" },
  { id: "tracking", label: "common.groupTracking" },
  { id: "output", label: "common.groupOutput" },
  { id: "setup", label: "common.groupSetup" },
];

export function moduleOf(id: PageId): ModuleEntry {
  return MODULES.find((entry) => entry.id === id) ?? MODULES[0]!;
}
