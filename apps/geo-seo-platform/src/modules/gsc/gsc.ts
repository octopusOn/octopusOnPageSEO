// What the Search Console page computes on its own: changes, the two periods of the daily
// chart, query filters, how the latest audit saw a page, and the rows of its CSVs.

import type { GscDay, GscPage, GscQuery, PageAudit } from "../../api/types";
import { fold } from "../../lib/report";

export const DAYS = [7, 14, 28, 90, 180, 240] as const;

/** Longest period compared with the same days a year before (Search Console keeps 16 months). */
export const YEAR_DAYS = 90;

/** Change from `before` to `now` in percent; `null` without a base. */
export function percentChange(now: number, before: number): number | null {
  if (!Number.isFinite(before) || before === 0) return null;
  return Math.round(((now - before) / before) * 1000) / 10;
}

export interface DailySeries {
  dates: string[];
  clicks: number[];
  impressions: number[];
  /** The same day of the period before; `null` where Google had no data. */
  clicksBefore: (number | null)[];
  sessions: (number | null)[];
}

const DAY_MS = 86_400_000;

function shiftDate(date: string, days: number): string {
  return new Date(Date.parse(`${date}T00:00:00Z`) - days * DAY_MS).toISOString().slice(0, 10);
}

/** The current days, each with the same day of the period before (`previousStart` on). */
export function dailySeries(days: readonly GscDay[], periodStart: string, previousStart: string): DailySeries {
  const gap = Math.round((Date.parse(periodStart) - Date.parse(previousStart)) / DAY_MS);
  const now = days.filter((day) => day.period === "now");
  const before = new Map(days.filter((day) => day.period === "before").map((day) => [day.date, day.clicks]));
  return {
    dates: now.map((day) => day.date),
    clicks: now.map((day) => day.clicks),
    impressions: now.map((day) => day.impressions),
    clicksBefore: now.map((day) => before.get(shiftDate(day.date, gap)) ?? null),
    sessions: now.map((day) => day.sessions),
  };
}

export type BrandFilter = "all" | "brand" | "other";

export interface QueryFilters {
  query: string;
  /** Only queries within reach of the first places. */
  nearTop: boolean;
  /** Only queries the rank tracker does not follow. */
  untracked: boolean;
  /** Queries naming the brand, the others, or all. */
  brand: BrandFilter;
}

export const ALL_QUERIES: QueryFilters = { query: "", nearTop: false, untracked: false, brand: "all" };

/** The queries a filter keeps; text is folded. */
export function filterQueries(rows: readonly GscQuery[], filters: QueryFilters): GscQuery[] {
  const wanted = fold(filters.query.trim());
  return rows.filter(
    (row) =>
      (!wanted || fold(row.query).includes(wanted)) &&
      (!filters.nearTop || row.near_top) &&
      (!filters.untracked || !row.tracked) &&
      (filters.brand === "all" || row.brand === (filters.brand === "brand")),
  );
}

/** A full export as CSV text: values written plainly, booleans as 1 and 0. */
export function exportRows(rows: readonly (string | number | boolean | null)[][]): string[][] {
  return rows.map((row) => row.map((value) => (value === null ? "" : typeof value === "boolean" ? (value ? "1" : "0") : String(value))));
}

export type AuditState = "ok" | "findings" | "noindex" | "redirect" | "error";

/** How the latest audit saw a page; `null` when it did not crawl it. */
export function auditState(audit: PageAudit | null): AuditState | null {
  if (!audit) return null;
  if (audit.status === null || audit.status >= 400) return "error";
  if (audit.status >= 300) return "redirect";
  if (audit.indexability !== "indexable") return "noindex";
  return audit.findings > 0 ? "findings" : "ok";
}

/** CSV rows of the queries. */
export function queryCsv(rows: readonly GscQuery[]): string[][] {
  return rows.map((row) => [
    row.query,
    String(row.clicks),
    row.clicks_change === null ? "" : String(row.clicks_change),
    String(row.impressions),
    String(row.ctr),
    String(row.position),
    row.position_change === null ? "" : String(row.position_change),
    String(row.pages),
    row.tracked ? "1" : "0",
  ]);
}

/** CSV rows of the pages. */
export function pageCsv(rows: readonly GscPage[]): string[][] {
  return rows.map((row) => [
    row.page,
    String(row.clicks),
    row.clicks_change === null ? "" : String(row.clicks_change),
    String(row.impressions),
    String(row.ctr),
    String(row.position),
    row.sessions === null ? "" : String(row.sessions),
    row.key_events === null ? "" : String(row.key_events),
    row.revenue === null ? "" : String(row.revenue),
  ]);
}
