// What the Site Audit page computes on its own: filters, totals, changes and prices.

import type { CategoryReport, IssueKind, IssueReport, MeterInfo } from "../../api/types";
import { fold } from "../../lib/report";

export const KINDS: IssueKind[] = ["issue", "warning", "opportunity"];

/** Page limits offered, up to what the host allows. */
export function pageChoices(limit: number): number[] {
  return [100, 500, 1000, 5000, 10_000, 50_000, 100_000].filter((pages) => pages <= limit);
}

export interface IssueFilters {
  kind: IssueKind | "all";
  query: string;
  /** Only checks found now (not those fixed since the audit before). */
  openOnly: boolean;
}

export const ALL_ISSUES: IssueFilters = { kind: "all", query: "", openOnly: true };

/** The categories a filter keeps, without empty ones; text is folded. */
export function filterCategories(categories: readonly CategoryReport[], filters: IssueFilters): CategoryReport[] {
  const query = fold(filters.query.trim());
  return categories
    .map((category) => ({
      ...category,
      issues: category.issues.filter(
        (issue) =>
          (filters.kind === "all" || issue.kind === filters.kind) &&
          (!filters.openOnly || issue.count > 0) &&
          (!query || fold(issue.title).includes(query) || fold(category.name).includes(query)),
      ),
    }))
    .filter((category) => category.issues.length > 0);
}

/** Checks with findings now, by kind. */
export function totals(categories: readonly CategoryReport[]): Record<IssueKind, number> {
  const counts: Record<IssueKind, number> = { issue: 0, warning: 0, opportunity: 0 };
  for (const issue of categories.flatMap((category) => category.issues)) if (issue.count > 0) counts[issue.kind] += 1;
  return counts;
}

/** Findings gained (positive) or fixed (negative) since the audit before; `null` without one. */
export function changeOf(issue: IssueReport): number | null {
  return issue.previous === null ? null : issue.count - issue.previous;
}

/** Credits a thousand pages cost after the free ones. */
export function creditsPerThousand(meter: MeterInfo): number {
  return (meter.micros_per_unit * 1000) / Math.max(1, meter.credit_unit_micros);
}

/** A score change rounded to one decimal; `null` without both scores. */
export function scoreChange(now: number | null, before: number | null | undefined): number | null {
  if (now === null || before === null || before === undefined) return null;
  return Math.round((now - before) * 10) / 10;
}
