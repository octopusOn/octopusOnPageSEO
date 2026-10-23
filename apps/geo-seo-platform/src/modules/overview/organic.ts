// What a domain's kept keywords and pages say beyond their rows: its folders, the intent and
// result-page elements of its keywords, and how much of its traffic comes from its own name.
// All free: they read answers already fetched.

import type { PageRow, RankedKeyword } from "../../api/types";
import { fold } from "../../lib/report";

/** A folder of a site and what its pages bring. */
export interface FolderRow {
  /** `/blog/`, or `/` for pages at the root. */
  folder: string;
  pages: number;
  traffic: number;
  keywords: number;
  top10: number;
  value: number;
}

/** The first folder of an address's path: `/blog/` of `/blog/post`, `/` of `/post`. */
export function folderOf(url: string): string {
  let path = url;
  try {
    path = new URL(url).pathname;
  } catch {
    // A bare path stays as it is.
  }
  const parts = path.split("/").filter(Boolean);
  return parts.length > 1 ? `/${parts[0]}/` : "/";
}

/** Pages summed by their first folder, most traffic first. */
export function subfolders(pages: readonly PageRow[]): FolderRow[] {
  const folders = new Map<string, FolderRow>();
  for (const page of pages) {
    const folder = folderOf(page.url);
    const row = folders.get(folder) ?? { folder, pages: 0, traffic: 0, keywords: 0, top10: 0, value: 0 };
    row.pages += 1;
    row.traffic += page.organic.traffic;
    row.keywords += page.organic.keywords;
    row.top10 += page.organic.pos_1 + page.organic.pos_2_3 + page.organic.pos_4_10;
    row.value += page.organic.traffic_value_usd;
    folders.set(folder, row);
  }
  return [...folders.values()].sort((left, right) => right.traffic - left.traffic || left.folder.localeCompare(right.folder));
}

/** Keywords and their traffic under one heading. */
export interface Share {
  key: string;
  keywords: number;
  traffic: number;
}

export const INTENT_ORDER = ["informational", "navigational", "commercial", "transactional"] as const;

/** Keywords and traffic by main intent, in a fixed order; keywords without one under `unknown`. */
export function intentShares(rows: readonly RankedKeyword[]): Share[] {
  const shares = new Map<string, Share>([...INTENT_ORDER, "unknown"].map((key) => [key, { key, keywords: 0, traffic: 0 }]));
  for (const row of rows) {
    const share = shares.get(row.intent && shares.has(row.intent) ? row.intent : "unknown")!;
    share.keywords += 1;
    share.traffic += row.traffic;
  }
  return [...shares.values()].filter((share) => share.key !== "unknown" || share.keywords > 0);
}

/** Keywords and traffic by each result-page element they show, most common first. */
export function featureShares(rows: readonly RankedKeyword[]): Share[] {
  const shares = new Map<string, Share>();
  for (const row of rows) {
    for (const feature of new Set(row.serp_features)) {
      if (feature === "organic" || feature === "paid") continue;
      const share = shares.get(feature) ?? { key: feature, keywords: 0, traffic: 0 };
      share.keywords += 1;
      share.traffic += row.traffic;
      shares.set(feature, share);
    }
  }
  return [...shares.values()].sort((left, right) => right.keywords - left.keywords || left.key.localeCompare(right.key));
}

/**
 * The words that make a search one for the brand: the project's brand names and the domain's
 * name (`ornekmagaza` of `ornekmagaza.com`), folded, at least three letters long.
 */
export function brandTerms(domain: string, brands: readonly string[]): string[] {
  const name = domain.replace(/^www\./, "").split(".")[0] ?? "";
  const terms = [...brands, name].map((term) => fold(term.trim())).filter((term) => term.length >= 3);
  return [...new Set(terms)];
}

/** Whether a keyword names the brand, with or without spaces (`örnek mağaza` for `ornekmagaza`). */
export function isBranded(keyword: string, terms: readonly string[]): boolean {
  const folded = fold(keyword);
  const joined = folded.replace(/\s+/g, "");
  return terms.some((term) => folded.includes(term) || joined.includes(term.replace(/\s+/g, "")));
}

/** Keywords and traffic that name the brand, and the rest. */
export function brandSplit(rows: readonly RankedKeyword[], terms: readonly string[]): { branded: Share; other: Share } {
  const branded: Share = { key: "branded", keywords: 0, traffic: 0 };
  const other: Share = { key: "other", keywords: 0, traffic: 0 };
  for (const row of rows) {
    const share = terms.length > 0 && isBranded(row.keyword, terms) ? branded : other;
    share.keywords += 1;
    share.traffic += row.traffic;
  }
  return { branded, other };
}
