import { expect, it } from "vitest";
import type { CrawlActivity, UrlActivity } from "../api/types";
import { pageTimings } from "./livePerformance";

const page: UrlActivity = { id: 1, url: "https://example.com/tr/", kind: "page", state: "completed", started_ms: 0, elapsed_ms: 1200, status: 200, title: "Page", error: null, rendered: false, render_error: null, response_ms: 800, size_bytes: 100 };
const activity = (recent: UrlActivity[]): CrawlActivity => ({ active: [], recent, rendered: 0, render_errors: 0, request_errors: 0, blocked: 0 });

it("separates page response headers from operation time and retains HTTP success with a JS failure", () => {
  const sample = pageTimings(activity([page, { ...page, id: 2, state: "error", render_error: "timeout", response_ms: 200, elapsed_ms: 10000 }]));
  expect(sample).toEqual({ count: 2, responseMs: 500, processingMs: 5600 });
});

it("does not mix resources, HTTP errors, blocked, cancelled, missing or invalid timings into page averages", () => {
  const rows: UrlActivity[] = [
    ...(["external", "resource", "robots", "sitemap", "site_file"] as const).map(kind => ({ ...page, kind })),
    ...(["processing", "blocked", "cancelled"] as const).map(state => ({ ...page, state })),
    ...[null, 301, 404, 500].map(status => ({ ...page, status })),
    ...[null, NaN, Infinity, -1, 1201].map(response_ms => ({ ...page, response_ms })),
    ...[NaN, Infinity, -1].map(elapsed_ms => ({ ...page, elapsed_ms })),
    { ...page, error: "connection" },
  ];
  expect(pageTimings(activity(rows))).toBeNull();
  expect(pageTimings(activity([...rows, page]))).toEqual({ count: 1, responseMs: 800, processingMs: 1200 });
  expect(pageTimings()).toBeNull();
});

it("keeps the sample in the latest 100 operations without mutating the feed", () => {
  const rows = Array.from({ length: 101 }, (_, id) => ({ ...page, id, response_ms: id < 100 ? 0 : 900 }));
  const before = structuredClone(rows);
  expect(pageTimings(activity(rows))).toEqual({ count: 100, responseMs: 0, processingMs: 1200 });
  expect(rows).toEqual(before);
});
