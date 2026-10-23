// What the project page computes on its own: opportunity texts' values, filters, and a
// series' change.

import type { DatedValue, Effort, Opportunity, OpportunityRule } from "../../api/types";
import { PAGES, type PageId } from "../../lib/route";

/** The modules opportunities come from, as filters. */
export const AREAS = ["all", "rank", "gsc", "audit", "ai", "content", "local", "backlinks", "gap"] as const;
export type Area = (typeof AREAS)[number];

export interface OpportunityFilter {
  area: Area;
  effort: Effort | "all";
}

export const ALL_OPPORTUNITIES: OpportunityFilter = { area: "all", effort: "all" };

export function filterOpportunities(items: readonly Opportunity[], filter: OpportunityFilter): Opportunity[] {
  return items.filter((item) => (filter.area === "all" || item.page === filter.area) && (filter.effort === "all" || item.effort === filter.effort));
}

/** An opportunity's values for its text: numbers written in the reader's way, the subject as
 * `subject`. */
export function textValues(item: Opportunity, locale: string): Record<string, string> {
  const values: Record<string, string> = { subject: item.subject };
  for (const [name, value] of Object.entries(item.detail)) {
    if (value === null || value === undefined) values[name] = "—";
    else if (typeof value === "number") values[name] = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
    else values[name] = String(value);
  }
  return values;
}

/** The text key of an opportunity: dropped keywords read differently when they are gone. */
export function titleKey(item: Opportunity): string {
  if (item.rule === "dropped" && item.detail.lost === true) return "opportunities.title_lost";
  return `opportunities.title_${item.rule}`;
}

/** The page a spending record's module belongs to (its label names it). */
export function spendPage(module: string): PageId {
  const pages: Record<string, PageId> = { research: "keywords", ai_mentions: "ai", content: "content", snapshot: "snapshots", snapshots: "snapshots" };
  return pages[module] ?? ((PAGES as readonly string[]).includes(module) ? (module as PageId) : "dashboard");
}

/** Rules a content brief helps with. */
export const BRIEF_RULES: OpportunityRule[] = ["near_top", "low_content", "dropped"];

/** Change from the first to the last value of a series, in percent; `null` without both. */
export function seriesChange(points: readonly DatedValue[]): number | null {
  const values = points.map((point) => point.value).filter((value): value is number => value !== null);
  if (values.length < 2) return null;
  const first = values[0]!;
  const latest = values[values.length - 1]!;
  return first === 0 ? null : Math.round(((latest - first) / Math.abs(first)) * 1000) / 10;
}
