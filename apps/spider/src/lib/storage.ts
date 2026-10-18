import type { CrawlMeta, CrawlStorage } from "../api/types";
import type { Lang } from "../i18n";

export type CrawlSort = "newest" | "largest";

function validBytes(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

/** Missing or invalid measurements stay unknown; never count stale deleted archives. */
export function storageSummary(crawls: CrawlMeta[], snapshot: CrawlStorage[] = []) {
  const sizes = new Map(snapshot.map(item => [item.id, validBytes(item.bytes) ? item.bytes : null]));
  let knownBytes = 0, measured = 0;
  for (const crawl of crawls) {
    const size = sizes.get(crawl.id);
    if (validBytes(size)) { knownBytes += size; measured++; }
  }
  return {
    sizes,
    bytes: Number.isSafeInteger(knownBytes) ? knownBytes : null,
    measured,
    unknown: crawls.length - measured,
  };
}

export function sortCrawls(crawls: CrawlMeta[], sizes: Map<string, number | null>, sort: CrawlSort) {
  return [...crawls].sort((a, b) => {
    if (sort === "largest") {
      const left = sizes.get(a.id), right = sizes.get(b.id);
      if (validBytes(left) && validBytes(right) && left !== right) return right - left;
      if (validBytes(left) !== validBytes(right)) return validBytes(left) ? -1 : 1;
    }
    return b.created_at_ms - a.created_at_ms || b.id.localeCompare(a.id);
  });
}

/** SI file-size units. Reject unsafe values and avoid displaying a positive size as zero. */
export function fileSize(bytes: number | null | undefined, lang: Lang): string {
  if (!validBytes(bytes)) return "—";
  const units = ["B", "kB", "MB", "GB", "TB", "PB"];
  const unit = bytes === 0 ? 0 : Math.min(Math.floor(Math.log10(bytes) / 3), units.length - 1);
  return `${new Intl.NumberFormat(lang === "tr" ? "tr-TR" : "en-US", { maximumFractionDigits: 1 }).format(bytes / 1000 ** unit)} ${units[unit]}`;
}
