import { expect, it } from "vitest";
import { defaultConfig, type CrawlMeta } from "../api/types";
import { fileSize, sortCrawls, storageSummary } from "./storage";

const crawl: CrawlMeta = { id: "saved", name: "Example", start_urls: [], created_at_ms: 0, finished_at_ms: 1, status: "completed", progress: { crawled: 0, queued: 0, errors: 0, elapsed_ms: 0, discovered: 0, in_flight: 0, sitemaps: 0, paused: false }, limit_reached: false, error: null, config: defaultConfig };

it("keeps missing, partial, unsafe and stale measurements out of the total", () => {
  const crawls = [crawl, { ...crawl, id: "empty" }, { ...crawl, id: "new" }, { ...crawl, id: "bad" }];
  const summary = storageSummary(crawls, [{ id: crawl.id, bytes: 500 }, { id: "empty", bytes: 0 }, { id: "deleted", bytes: 9000 }, { id: "bad", bytes: -1 }]);
  expect(summary).toMatchObject({ bytes: 500, measured: 2, unknown: 2 });
  for (const bytes of [null, NaN, Infinity, -1, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
    expect(storageSummary([crawl], [{ id: crawl.id, bytes }])).toMatchObject({ bytes: 0, measured: 0, unknown: 1 });
  }
  expect(storageSummary([crawl])).toMatchObject({ bytes: 0, measured: 0, unknown: 1 });
  expect(storageSummary([])).toMatchObject({ bytes: 0, measured: 0, unknown: 0 });
  expect(storageSummary(crawls.slice(0, 2), [{ id: crawl.id, bytes: Number.MAX_SAFE_INTEGER }, { id: "empty", bytes: 1 }]).bytes).toBeNull();
});

it("sorts largest first with unknown sizes last and a deterministic date fallback without mutating the source", () => {
  const crawls = [crawl, { ...crawl, id: "unknown", created_at_ms: 20 }, { ...crawl, id: "largest", created_at_ms: 10 }, { ...crawl, id: "zero", created_at_ms: 5 }];
  const ids = crawls.map(c => c.id);
  const sizes = new Map([[crawl.id, 5], ["largest", 100], ["zero", 0], ["unknown", null]]);
  expect(sortCrawls(crawls, sizes, "largest").map(c => c.id)).toEqual(["largest", crawl.id, "zero", "unknown"]);
  expect(sortCrawls(crawls, sizes, "newest").map(c => c.id)).toEqual(["unknown", "largest", "zero", crawl.id]);
  expect(crawls.map(c => c.id)).toEqual(ids);
});

it("uses localized SI units, preserves known zero and formats unknown sizes as a dash", () => {
  expect(fileSize(0, "tr")).toBe("0 B");
  expect(fileSize(1, "en")).toBe("1 B");
  expect(fileSize(1_250_000, "tr")).toBe("1,3 MB");
  expect(fileSize(1_250_000, "en")).toBe("1.3 MB");
  expect(fileSize(90_000_000_000, "tr")).toBe("90 GB");
  for (const bytes of [undefined, null, -1, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) expect(fileSize(bytes, "tr")).toBe("—");
});
