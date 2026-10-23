// Snapshots without the page: how each number is written and how it moved, which two snapshots
// are compared by default, and the numbers shown on a project's card.

import type { SnapshotHeadline, SnapshotKpi, SnapshotRow, SnapshotSetup } from "../../api/types";
import { formatNumber, formatPercent } from "../../lib/format";

export const TABS = ["summary", "keywords", "pages", "backlinks", "rank", "audit", "ai", "local", "competitors"] as const;
export type Tab = (typeof TABS)[number];

/** Headline numbers of a comparison, in the order shown. */
export const HEADLINE_KPIS = ["traffic", "keywords", "top10", "referring_domains", "oa", "visibility", "health", "mention_rate", "clicks"] as const;

/** Numbers of a snapshot in the timeline. */
export const TIMELINE_NUMBERS: (keyof Omit<SnapshotHeadline, "missing">)[] = ["traffic", "keywords", "oa", "health", "mention_rate"];

type Kind = "count" | "rate" | "score" | "money";

/** How a number is written: counts whole, rates as percentages, scores with one decimal. */
export function kindOf(key: string): Kind {
  if (["visibility", "mention_rate", "citation_rate", "share_of_voice", "ctr", "local_solv", "solv"].includes(key)) return "rate";
  if (["average_position", "gsc_position", "health", "geo", "local_arp", "local_rating"].includes(key)) return "score";
  return key === "traffic_value" ? "money" : "count";
}

/** A number of `key` as the reader writes it. */
export function formatKpi(key: string, value: number | null, locale: string): string {
  if (value === null) return "—";
  switch (kindOf(key)) {
    case "rate":
      return formatPercent(value, locale);
    case "score":
      return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
    case "money":
      return new Intl.NumberFormat(locale, { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(value);
    default:
      return formatNumber(value, locale);
  }
}

const signed = (text: string, value: number) => (value > 0 ? `+${text}` : text);

/** How much a number moved: in percent for counts, in points for rates and scores; `null` when it did not. */
export function deltaText(kpi: SnapshotKpi, locale: string): string | null {
  if (kpi.change === null || Math.abs(kpi.change) < 1e-9) return null;
  const kind = kindOf(kpi.key);
  if ((kind === "count" || kind === "money") && kpi.percent !== null) {
    return signed(new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: Math.abs(kpi.percent) < 10 ? 1 : 0 }).format(kpi.percent / 100), kpi.percent);
  }
  const digits = kind === "count" || kind === "money" ? 0 : 1;
  return signed(new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(kpi.change), kpi.change);
}

/** Whether a number moved the good way; `undefined` when it did not move. */
export function toneOf(kpi: SnapshotKpi): "good" | "bad" | undefined {
  if (kpi.change === null || Math.abs(kpi.change) < 1e-9) return undefined;
  return kpi.change > 0 === kpi.higher_better ? "good" : "bad";
}

/** Places a keyword moved, signed (+8 is up). */
export function placesText(change: number | null): string {
  if (change === null || change === 0) return "";
  return change > 0 ? `+${change}` : String(change);
}

/** Snapshots that can be compared, newest first. */
export function finishedOf(rows: readonly SnapshotRow[]): SnapshotRow[] {
  return rows.filter((row) => row.status === "done" || row.status === "partial");
}

/** The pair compared first: where the work began against the latest. */
export function defaultPair(setup: SnapshotSetup): { from: number; to: number } | null {
  const finished = finishedOf(setup.snapshots);
  if (finished.length < 2) return null;
  const to = finished[0]!.id;
  const from = setup.baseline_id !== null && setup.baseline_id !== to ? setup.baseline_id : finished[finished.length - 1]!.id;
  return { from, to };
}

/** The competitors a snapshot keeps: the saved choice among the project's own, in its order. */
export function chosenCompetitors(setup: SnapshotSetup): string[] {
  return setup.competitors.filter((domain) => setup.settings.competitors.includes(domain));
}

/** `domain` added to or taken out of a choice, in the project's order. */
export function toggled(choice: readonly string[], domain: string, all: readonly string[]): string[] {
  const next = choice.includes(domain) ? choice.filter((known) => known !== domain) : [...choice, domain];
  return all.filter((known) => next.includes(known));
}

/** The last path of an address, short enough for a table. */
export function pathOf(url: string | null): string {
  if (!url) return "";
  try {
    const parsed = new URL(url);
    return `${parsed.pathname}${parsed.search}` || "/";
  } catch {
    return url;
  }
}
