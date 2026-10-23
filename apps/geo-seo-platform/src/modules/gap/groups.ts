// The keyword gap read beyond its rows: which keywords a group and the filters keep, and each
// site's share of the traffic month by month.

import type { BulkRow, GapGroup, KeywordData, MatrixKeyword, TrafficHistory } from "../../api/types";
import { fold } from "../../lib/report";

/** The groups, in the order they are offered. */
export const GAP_GROUPS: GapGroup[] = ["shared", "missing", "weak", "strong", "untapped", "unique"];

export interface MatrixFilters {
  query: string;
  intent: string;
  minVolume: number;
  maxDifficulty: number;
  minCpc: number;
}

export const NO_MATRIX_FILTER: MatrixFilters = { query: "", intent: "", minVolume: 0, maxDifficulty: 100, minCpc: 0 };

/** The keywords of a group (every keyword with `all`) the filters keep. */
export function filterMatrix(rows: readonly MatrixKeyword[], group: GapGroup | "all", filters: MatrixFilters): MatrixKeyword[] {
  const query = fold(filters.query.trim());
  return rows.filter(
    (row) =>
      (group === "all" || row.groups.includes(group)) &&
      (!query || fold(row.keyword).includes(query)) &&
      (!filters.intent || row.intent === filters.intent) &&
      (row.volume ?? 0) >= filters.minVolume &&
      (filters.maxDifficulty >= 100 || (row.difficulty ?? 0) <= filters.maxDifficulty) &&
      (row.cpc ?? 0) >= filters.minCpc,
  );
}

/** A gap keyword as a list or the rank tracker keeps it. */
export function asKeyword(row: Pick<MatrixKeyword, "keyword" | "volume" | "cpc" | "difficulty" | "intent">): KeywordData {
  return {
    keyword: row.keyword,
    volume: row.volume,
    cpc: row.cpc,
    competition: null,
    competition_level: null,
    difficulty: row.difficulty,
    intent: row.intent,
    other_intents: [],
    monthly: [],
    trend: { monthly: null, quarterly: null, yearly: null },
    serp_features: [],
    results: null,
    top_backlinks: null,
    top_referring_domains: null,
  };
}

/** Each site's share of the sites' organic traffic, in percent, month by month. */
export function shareSeries(histories: readonly TrafficHistory[]): { months: string[]; series: { target: string; shares: number[]; traffic: number[] }[] } {
  const keys = [...new Set(histories.flatMap((history) => history.months.map((month) => `${month.year}-${String(month.month).padStart(2, "0")}`)))].sort();
  const traffic = histories.map((history) => {
    const byMonth = new Map(history.months.map((month) => [`${month.year}-${String(month.month).padStart(2, "0")}`, month.traffic]));
    return keys.map((key) => byMonth.get(key) ?? 0);
  });
  const totals = keys.map((_, index) => traffic.reduce((sum, row) => sum + (row[index] ?? 0), 0));
  return {
    months: keys,
    series: histories.map((history, row) => ({
      target: history.target,
      traffic: traffic[row]!,
      shares: traffic[row]!.map((value, index) => (totals[index]! > 0 ? Math.round((value / totals[index]!) * 1000) / 10 : 0)),
    })),
  };
}

/** Each site's share of the traffic of all, in percent. */
export function trafficShares(rows: readonly BulkRow[]): Map<string, number> {
  const total = rows.reduce((sum, row) => sum + row.organic_traffic, 0);
  return new Map(rows.map((row) => [row.target, total > 0 ? (row.organic_traffic / total) * 100 : 0]));
}
