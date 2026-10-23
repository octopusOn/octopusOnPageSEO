// Labels the shared table needs, in the reader's language.

import type { TableLabels } from "@octopusseo/ui";
import { formatNumber } from "./format";

/** Page sizes the larger tables offer. */
export const PAGE_SIZES = [50, 100, 250, 500];

export function tableLabels(t: (key: string, values?: Record<string, string | number>) => string, locale: string): TableLabels {
  return {
    previous: t("common.pagePrevious"),
    next: t("common.pageNext"),
    page: (from: number, to: number, total: number) =>
      t("common.pageRange", { from: formatNumber(from, locale), to: formatNumber(to, locale), total: formatNumber(total, locale) }),
    sortBy: (label: string) => t("common.sortBy", { label }),
    columns: t("common.columns"),
    rowsPerPage: t("common.rowsPerPage"),
    total: t("common.total"),
    help: (label: string) => t("common.whatIs", { label }),
  };
}

/** A result-page element in words: its translation when there is one, else its name. */
export function featureName(feature: string, t: (key: string) => string): string {
  const key = `overview.feature_${feature.replace(/[^a-z0-9]/g, "")}`;
  const text = t(key);
  return text === key ? feature.replace(/_/g, " ") : text;
}

/** The path of an address, for compact tables. */
export function shortUrl(url: string | null): string {
  if (!url) return "";
  try {
    const parsed = new URL(url);
    const path = `${parsed.pathname}${parsed.search}` || "/";
    // Letters such as ç and ı read as themselves, not as %C3%A7.
    try {
      return decodeURI(path);
    } catch {
      return path;
    }
  } catch {
    return url;
  }
}
