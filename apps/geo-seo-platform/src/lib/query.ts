// Filters and order of a list report. Asked of DataForSEO they reach every row it holds; when
// every row is loaded already they run here, free. Field names are the platform's own, the same
// ones the Rust side maps to DataForSEO's paths (crates/octopuson-dataforseo/src/query.rs).

import type { BacklinkRow, KeywordData, RankedKeyword, ReferringDomainRow } from "../api/types";
import { fold } from "./report";

export type QueryOp =
  | "eq"
  | "ne"
  | "lt"
  | "le"
  | "gt"
  | "ge"
  | "in"
  | "not_in"
  | "contains"
  | "not_contains"
  | "starts_with"
  | "regex"
  | "not_regex"
  | "has"
  | "has_not";

export type FieldKind = "number" | "text" | "flag" | "list";

export type QueryValue = number | string | boolean | string[];

export interface QueryFilter {
  field: string;
  op: QueryOp;
  value: QueryValue;
}

export interface QueryOrder {
  field: string;
  direction: "asc" | "desc";
}

/** A list question's filters, order and first row, as the commands take it. */
export interface ListQuery {
  filters: QueryFilter[];
  order: QueryOrder[];
  offset: number;
}

export const EMPTY_QUERY: ListQuery = { filters: [], order: [], offset: 0 };

/** Most filters and sorting rules DataForSEO takes in one question. */
export const MAX_FILTERS = 8;
export const MAX_ORDER = 3;

/** A field a list can be filtered by; `label` is an i18n key. */
export interface FilterField {
  name: string;
  kind: FieldKind;
  label: string;
}

/** Operators each kind of field takes, the first one offered first. */
export const OPS: Record<FieldKind, QueryOp[]> = {
  number: ["ge", "le", "gt", "lt", "eq", "ne"],
  text: ["contains", "not_contains", "starts_with", "eq", "ne", "in", "not_in", "regex", "not_regex"],
  flag: ["eq"],
  list: ["has", "has_not"],
};

const f = (name: string, kind: FieldKind, label: string): FilterField => ({ name, kind, label });

/** Fields of a domain's keywords (Site Explorer). */
export const RANKED_KEYWORD_FILTERS: FilterField[] = [
  f("keyword", "text", "overview.colKeyword"),
  f("position", "number", "overview.colPosition"),
  f("volume", "number", "overview.colVolume"),
  f("kd", "number", "overview.colKd"),
  f("traffic", "number", "overview.colTraffic"),
  f("value", "number", "overview.colValue"),
  f("cpc", "number", "overview.colCpc"),
  f("competition", "number", "overview.colCompetition"),
  f("intent", "text", "overview.colIntent"),
  f("features", "list", "overview.colFeatures"),
  f("url", "text", "overview.colUrl"),
  f("previous", "number", "query.fieldPrevious"),
  f("new", "flag", "query.fieldNew"),
  f("up", "flag", "query.fieldUp"),
  f("down", "flag", "query.fieldDown"),
];

/** Fields of keyword ideas, related keywords and suggestions. */
export const KEYWORD_FILTERS: FilterField[] = [
  f("keyword", "text", "overview.colKeyword"),
  f("volume", "number", "overview.colVolume"),
  f("kd", "number", "overview.colKd"),
  f("cpc", "number", "overview.colCpc"),
  f("competition", "number", "overview.colCompetition"),
  f("intent", "text", "overview.colIntent"),
  f("features", "list", "overview.colFeatures"),
  f("words", "number", "query.fieldWords"),
];

/** Fields of a target's backlinks. */
export const BACKLINK_FILTERS: FilterField[] = [
  f("domain", "text", "query.fieldDomain"),
  f("url", "text", "query.fieldSource"),
  f("target", "text", "query.fieldTarget"),
  f("anchor", "text", "query.fieldAnchor"),
  f("dofollow", "flag", "query.fieldDofollow"),
  f("rank", "number", "query.fieldPageRank"),
  f("domain_rank", "number", "query.fieldDomainRank"),
  f("spam", "number", "query.fieldSpam"),
  f("first_seen", "text", "query.fieldFirstSeen"),
  f("last_seen", "text", "query.fieldLastSeen"),
  f("country", "text", "query.fieldCountry"),
];

/** Fields of a target's referring domains. */
export const REFERRING_DOMAIN_FILTERS: FilterField[] = [
  f("domain", "text", "query.fieldDomain"),
  f("rank", "number", "query.fieldDomainRank"),
  f("backlinks", "number", "query.fieldBacklinks"),
  f("spam", "number", "query.fieldSpam"),
  f("first_seen", "text", "query.fieldFirstSeen"),
];

export function isEmptyQuery(query: ListQuery | undefined): boolean {
  return !query || (query.filters.length === 0 && query.order.length === 0 && query.offset === 0);
}

/** The query a command takes: nothing for an empty one, so the question stays the same. */
export function askedQuery(query: ListQuery | undefined): { query?: ListQuery } {
  return isEmptyQuery(query) ? {} : { query };
}

/**
 * A value typed into the filter form, in the shape its field takes: a number, a flag, a list of
 * words for `in`, else trimmed text. `null` when it cannot be one.
 */
export function parseValue(kind: FieldKind, op: QueryOp, text: string): QueryValue | null {
  const trimmed = text.trim();
  if (kind === "flag") return trimmed !== "false";
  if (!trimmed) return null;
  if (kind === "number") {
    const number = Number(trimmed.replace(/\s/g, "").replace(",", "."));
    return Number.isFinite(number) ? number : null;
  }
  if (op === "in" || op === "not_in") {
    const words = trimmed
      .split(/[,\n]/)
      .map((word) => word.trim())
      .filter(Boolean);
    return words.length > 0 ? words : null;
  }
  if (op === "regex" || op === "not_regex") {
    try {
      new RegExp(trimmed);
    } catch {
      return null;
    }
  }
  return trimmed;
}

/** The filters with one more, replacing a filter on the same field and operator. */
export function withFilter(filters: readonly QueryFilter[], next: QueryFilter): QueryFilter[] {
  return [...filters.filter((filter) => !(filter.field === next.field && filter.op === next.op)), next].slice(-MAX_FILTERS);
}

/** How a row's field reads, for filters run here. */
export type Accessors<T> = Record<string, (row: T) => number | string | boolean | string[] | null | undefined>;

function compareText(value: string, op: QueryOp, wanted: QueryValue): boolean {
  const folded = fold(value);
  switch (op) {
    case "eq":
      return folded === fold(String(wanted));
    case "ne":
      return folded !== fold(String(wanted));
    case "in":
      return Array.isArray(wanted) && wanted.some((word) => fold(word) === folded);
    case "not_in":
      return Array.isArray(wanted) && !wanted.some((word) => fold(word) === folded);
    case "contains":
      return folded.includes(fold(String(wanted)));
    case "not_contains":
      return !folded.includes(fold(String(wanted)));
    case "starts_with":
      return folded.startsWith(fold(String(wanted)));
    case "regex":
    case "not_regex": {
      let matched = false;
      try {
        matched = new RegExp(String(wanted), "i").test(value);
      } catch {
        return false;
      }
      return op === "regex" ? matched : !matched;
    }
    default:
      return false;
  }
}

function compareNumber(value: number, op: QueryOp, wanted: number): boolean {
  switch (op) {
    case "eq":
      return value === wanted;
    case "ne":
      return value !== wanted;
    case "lt":
      return value < wanted;
    case "le":
      return value <= wanted;
    case "gt":
      return value > wanted;
    case "ge":
      return value >= wanted;
    default:
      return false;
  }
}

/** Whether a row passes a filter, as DataForSEO would decide; empty values pass nothing. */
export function passes<T>(row: T, filter: QueryFilter, accessors: Accessors<T>): boolean {
  const read = accessors[filter.field];
  if (!read) return true;
  const value = read(row);
  if (Array.isArray(value)) {
    const has = value.includes(String(filter.value));
    return filter.op === "has_not" ? !has : filter.op === "has" ? has : false;
  }
  if (value === null || value === undefined) return false;
  if (typeof value === "boolean") return filter.op === "eq" && value === filter.value;
  if (typeof value === "number") return typeof filter.value === "number" && compareNumber(value, filter.op, filter.value);
  return compareText(value, filter.op, filter.value);
}

/** The rows every filter passes. */
export function filterRows<T>(rows: readonly T[], filters: readonly QueryFilter[], accessors: Accessors<T>): T[] {
  if (filters.length === 0) return [...rows];
  return rows.filter((row) => filters.every((filter) => passes(row, filter, accessors)));
}

/** Rows `offset` to `offset + limit` of `total`, counted from one. */
export function pageRange(offset: number, rows: number, total: number): { from: number; to: number } {
  if (rows === 0 || total === 0) return { from: 0, to: 0 };
  return { from: offset + 1, to: Math.min(total, offset + rows) };
}

/** Rows sorted by order rules, empty values last. */
export function orderRows<T>(rows: readonly T[], order: readonly QueryOrder[], accessors: Accessors<T>): T[] {
  const rules = order.filter((rule) => accessors[rule.field]);
  if (rules.length === 0) return [...rows];
  return [...rows].sort((left, right) => {
    for (const rule of rules) {
      const read = accessors[rule.field]!;
      const a = read(left);
      const b = read(right);
      const aEmpty = a === null || a === undefined || a === "";
      const bEmpty = b === null || b === undefined || b === "";
      if (aEmpty || bEmpty) {
        if (aEmpty !== bEmpty) return aEmpty ? 1 : -1;
        continue;
      }
      const sign = rule.direction === "asc" ? 1 : -1;
      const compared = typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b));
      if (compared !== 0) return compared * sign;
    }
    return 0;
  });
}

/** A query run over rows held here, as DataForSEO runs it: the page and every matching row's count. */
export function runQuery<T>(rows: readonly T[], query: ListQuery | undefined, limit: number, accessors: Accessors<T>): { rows: T[]; total: number } {
  const matching = orderRows(filterRows(rows, query?.filters ?? [], accessors), query?.order ?? [], accessors);
  const offset = query?.offset ?? 0;
  return { rows: matching.slice(offset, offset + limit), total: matching.length };
}

/** How filters read a domain's ranked keyword. */
export const RANKED_KEYWORD_ACCESSORS: Accessors<RankedKeyword> = {
  keyword: (row) => row.keyword,
  position: (row) => row.position,
  volume: (row) => row.volume,
  kd: (row) => row.difficulty,
  traffic: (row) => row.traffic,
  value: (row) => row.traffic_value_usd,
  cpc: (row) => row.cpc,
  competition: (row) => row.competition,
  intent: (row) => row.intent,
  features: (row) => row.serp_features,
  url: (row) => row.url,
  previous: (row) => row.previous_position,
  new: (row) => row.change === "new",
  up: (row) => row.change === "up",
  down: (row) => row.change === "down",
};

/** How filters read a keyword idea. */
export const KEYWORD_ACCESSORS: Accessors<KeywordData> = {
  keyword: (row) => row.keyword,
  volume: (row) => row.volume,
  kd: (row) => row.difficulty,
  cpc: (row) => row.cpc,
  competition: (row) => row.competition,
  intent: (row) => row.intent,
  features: (row) => row.serp_features,
  words: (row) => row.keyword.trim().split(/\s+/).length,
};

/** How filters read a backlink. */
export const BACKLINK_ACCESSORS: Accessors<BacklinkRow> = {
  domain: (row) => row.domain_from,
  url: (row) => row.url_from,
  target: (row) => row.url_to,
  anchor: (row) => row.anchor,
  dofollow: (row) => row.dofollow,
  rank: (row) => row.page_rank,
  domain_rank: (row) => row.domain_rank,
  spam: (row) => row.spam_score,
  first_seen: (row) => row.first_seen,
  last_seen: (row) => row.last_seen,
  status: (row) => row.url_to_status,
  country: (row) => row.country,
};

/** How filters read a referring domain. */
export const REFERRING_DOMAIN_ACCESSORS: Accessors<ReferringDomainRow> = {
  domain: (row) => row.domain,
  rank: (row) => row.rank,
  backlinks: (row) => row.backlinks,
  spam: (row) => row.spam_score,
  first_seen: (row) => row.first_seen,
};
