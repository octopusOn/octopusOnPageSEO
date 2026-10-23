// Core Web Vitals as the Site Audit page reads them: Google's good / needs improvement / poor
// thresholds, how each metric is written, and finished audits in order for the trend chart.

import type { AuditRow, VitalName } from "../../api/types";

export type Rating = "good" | "needs" | "poor";

/** Good up to the first number, poor above the second (Google's thresholds). */
const THRESHOLDS: Record<VitalName | "tbt", [number, number]> = {
  lcp: [2_500, 4_000],
  inp: [200, 500],
  cls: [0.1, 0.25],
  fcp: [1_800, 3_000],
  ttfb: [800, 1_800],
  tbt: [200, 600],
};

export const VITALS: VitalName[] = ["lcp", "inp", "cls", "fcp", "ttfb"];

export function ratingOf(name: VitalName | "tbt", value: number): Rating {
  const [good, poor] = THRESHOLDS[name];
  return value <= good ? "good" : value <= poor ? "needs" : "poor";
}

/** A metric as Google writes it: seconds for LCP, FCP and TTFB from a second, milliseconds
 * otherwise, CLS without a unit. */
export function vitalText(name: VitalName | "tbt", value: number | null, locale: string): string {
  if (value === null) return "—";
  if (name === "cls") return new Intl.NumberFormat(locale, { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(value);
  if (name !== "inp" && name !== "tbt" && value >= 1_000) {
    return `${new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(value / 1_000)} s`;
  }
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value)} ms`;
}

/** Finished audits, oldest first, for the trend. */
export function finishedAudits(audits: readonly AuditRow[]): AuditRow[] {
  return audits
    .filter((audit) => audit.status !== "running" && audit.status !== "failed" && audit.health !== null)
    .slice()
    .sort((a, b) => a.started_at.localeCompare(b.started_at));
}

/** The values of `wanted` columns of each row, by column name; missing ones empty. */
export function pick(columns: readonly string[], rows: readonly string[][], wanted: readonly string[]): string[][] {
  const at = wanted.map((name) => columns.indexOf(name));
  return rows.map((row) => at.map((index) => (index >= 0 ? (row[index] ?? "") : "")));
}
