// What the Rank Tracker page computes on its own: place names, filters, keyword input and the
// rows of its CSV.

import type { KeywordPlaces, KeywordRank, NewKeyword, TrackerRow } from "../../api/types";
import { fold } from "../../lib/report";

export const ENGINES = ["google", "bing", "yandex"] as const;
export const DEPTHS = [10, 20, 50, 100] as const;
export const PERIODS = [30, 90, 365] as const;

const ENGINE_NAMES: Record<string, string> = { google: "Google", bing: "Bing", yandex: "Yandex" };

/** Whether a place is written as coordinates (`latitude,longitude` and maybe a radius). */
export function isCoordinate(location: string): boolean {
  const parts = location.split(",").map((part) => part.trim());
  return parts.length >= 2 && parts.length <= 3 && parts.every((part) => part !== "" && Number.isFinite(Number(part)));
}

/** A place in a few words: engine, city, postal code, point or country, device. */
export function placeLabel(tracker: TrackerRow, device: (device: string) => string): string {
  const where = !tracker.location
    ? tracker.country.toUpperCase()
    : isCoordinate(tracker.location)
      ? tracker.location
          .split(",")
          .slice(0, 2)
          .map((part) => Number(part).toFixed(3))
          .join(", ")
      : tracker.location.split(",")[0]!;
  return `${ENGINE_NAMES[tracker.engine] ?? tracker.engine} · ${where} · ${device(tracker.device)}`;
}

export type Band = "all" | "top3" | "top10" | "page2" | "none";

export const BANDS: Band[] = ["all", "top3", "top10", "page2", "none"];

export interface RankFilters {
  query: string;
  tag: string;
  band: Band;
  aiOnly: boolean;
}

export const ALL_RANKS: RankFilters = { query: "", tag: "", band: "all", aiOnly: false };

function inBand(position: number | null, band: Band): boolean {
  switch (band) {
    case "top3":
      return position !== null && position <= 3;
    case "top10":
      return position !== null && position <= 10;
    case "page2":
      return position !== null && position > 10 && position <= 20;
    case "none":
      return position === null;
    default:
      return true;
  }
}

/** The rows a filter keeps; text is folded (any case, accents, Turkish ı). */
export function filterRanks(rows: readonly KeywordRank[], filters: RankFilters): KeywordRank[] {
  const query = fold(filters.query.trim());
  return rows.filter(
    (row) =>
      (!query || fold(row.keyword).includes(query)) &&
      (!filters.tag || row.tags.includes(filters.tag)) &&
      inBand(row.position, filters.band) &&
      (!filters.aiOnly || row.ai_overview),
  );
}

/** Every tag of the rows, sorted. */
export function tagsOf(rows: readonly KeywordRank[]): string[] {
  return [...new Set(rows.flatMap((row) => row.tags))].sort((left, right) => left.localeCompare(right));
}

/** Keywords typed one per line (or pasted with tabs: the first column), without repeats. */
export function parseKeywords(text: string): NewKeyword[] {
  const seen = new Set<string>();
  const keywords: NewKeyword[] = [];
  for (const line of text.split(/\r?\n/)) {
    const keyword = (line.split("\t")[0] ?? "").replace(/\s+/g, " ").trim();
    const key = fold(keyword);
    if (!keyword || seen.has(key)) continue;
    seen.add(key);
    keywords.push({ keyword });
  }
  return keywords;
}

/** Tags typed with commas. */
export function parseTags(text: string): string[] {
  return [...new Set(text.split(",").map((tag) => tag.trim()).filter(Boolean))];
}

/** Positions for a small chart, best at the top and scaled to the keyword's own range: the
 * worst position is 1, each place better adds 1, not ranked is 0. */
export function heights(history: KeywordRank["history"]): number[] {
  const ranked = history.map(([, position]) => position).filter((position): position is number => position !== null);
  const worst = Math.max(0, ...ranked);
  return history.map(([, position]) => (position === null ? 0 : worst + 1 - position));
}

/** Places gained since the compared day (negative: lost); `null` when either day has no position. */
export function comparedChange(row: KeywordRank): number | null {
  const then = row.compared?.position ?? null;
  return then === null || row.position === null ? null : then - row.position;
}

/** The CSV rows of the table: the competitors' positions follow, then the compared day's. */
export function csvRows(rows: readonly KeywordRank[], rivals: readonly string[] = [], compared = false): (string | number | null)[][] {
  return rows.map((row) => [
    row.keyword,
    row.tags.join(" "),
    row.position,
    row.change,
    row.best,
    row.volume,
    row.traffic,
    row.url,
    row.ai_overview ? "yes" : "no",
    row.ai_cited ? "yes" : "no",
    row.owned.join(" "),
    row.checked_at,
    ...rivals.map((domain) => row.rivals[domain] ?? null),
    ...(compared ? [row.compared?.position ?? null] : []),
  ]);
}

/** The CSV rows of every place: a position and its change per place, in the places' order. */
export function placesCsvRows(rows: readonly KeywordPlaces[]): (string | number | null)[][] {
  return rows.map((row) => [row.keyword, row.tags.join(" "), row.volume, ...row.places.flatMap((place) => [place.position, place.change])]);
}
