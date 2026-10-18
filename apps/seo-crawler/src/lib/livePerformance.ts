import type { CrawlActivity } from "../api/types";

/** A bounded sample of successful page responses, independent of feed filters. */
export function pageTimings(activity?: CrawlActivity) {
  const pages = (activity?.recent ?? []).slice(0, 100).filter(row =>
    row.kind === "page" && ["completed", "error"].includes(row.state)
    && row.error === null && row.status !== null && row.status >= 200 && row.status < 300
    && row.response_ms !== null && Number.isFinite(row.response_ms) && row.response_ms >= 0
    && Number.isFinite(row.elapsed_ms) && row.elapsed_ms >= row.response_ms,
  );
  if (!pages.length) return null;
  return {
    count: pages.length,
    responseMs: pages.reduce((total, row) => total + row.response_ms!, 0) / pages.length,
    processingMs: pages.reduce((total, row) => total + row.elapsed_ms, 0) / pages.length,
  };
}
