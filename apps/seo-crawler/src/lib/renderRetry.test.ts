import { describe, expect, it, vi } from "vitest";
import { defaultConfig, type CrawlMeta, type TablePage } from "../api/types";
import { prepareRenderingRetry } from "./renderRetry";

const meta = (): CrawlMeta => ({
  id: "source", name: "example.com", start_urls: ["https://example.com/"],
  status: "completed", created_at_ms: 0, finished_at_ms: 1, limit_reached: true, error: null,
  progress: { crawled: 1, queued: 0, errors: 0, elapsed_ms: 1, discovered: 1, in_flight: 0, sitemaps: 0, paused: false },
  config: { ...structuredClone(defaultConfig), limits: { max_urls: 1, max_depth: 1, timeout_seconds: 20 } },
});
const page = (urls: string[], total = urls.length): TablePage => ({
  columns: [{ name: "address", kind: "text" }], rows: urls.map((url) => [url]), total,
});

describe("rendering retry preparation", () => {
  it("collects all pages in stable order and preserves the original crawl settings", async () => {
    const original = meta();
    original.config.rendering.viewport={width:390,height:844,mobile:true};
    const urls = Array.from({ length: 2504 }, (_, i) => `https://example.com/${String(i).padStart(4, "0")}`);
    const read = vi.fn(async (request) => page(urls.slice(request.offset, request.offset + request.limit), urls.length));
    const retry = await prepareRenderingRetry(original, read);
    expect(read.mock.calls.map(([request]) => request.offset)).toEqual([0, 1000, 2000]);
    expect(read.mock.calls.every(([request]) => request.issue_id === "render-error" && request.sort === "address")).toBe(true);
    expect(retry?.start_urls).toEqual(urls);
    expect(retry?.mode).toBe("list");
    expect(retry?.rendering.mode).toBe("javascript");
    expect(retry?.limits).toMatchObject({ max_urls: 0, max_depth: null });
    expect(retry?.speed).toEqual(original.config.speed);
    expect(retry?.rendering.viewport).toEqual(original.config.rendering.viewport);
    retry!.rendering.viewport!.width=768;
    expect(original.config.rendering.viewport.width).toBe(390);
    retry!.speed.threads = 1;
    expect(original.config.speed.threads).toBe(5);
    expect(original.config.limits.max_urls).toBe(1);
  });
  it("does not read an active crawl or create a retry when no failures remain", async () => {
    const read = vi.fn(async () => page([]));
    await expect(prepareRenderingRetry({ ...meta(), status: "running" }, read)).rejects.toThrow("still running");
    expect(read).not.toHaveBeenCalled();
    expect(await prepareRenderingRetry(meta(), read)).toBeNull();
  });
  it("rejects incomplete pages instead of silently preparing a partial retry", async () => {
    const read = vi.fn().mockResolvedValueOnce(page(["https://example.com/a"], 2)).mockResolvedValueOnce(page([], 2));
    await expect(prepareRenderingRetry(meta(), read)).rejects.toThrow("completely");
  });
  it("stops loading pages when preparation is cancelled", async () => {
    const controller = new AbortController();
    const read = vi.fn(async () => {
      controller.abort();
      return page(["https://example.com/a"], 2);
    });
    await expect(prepareRenderingRetry(meta(), read, controller.signal)).rejects.toMatchObject({ name: "AbortError" });
    expect(read).toHaveBeenCalledTimes(1);
  });
  it("rejects changed totals, duplicate rows and invalid addresses", async () => {
    const read = vi.fn().mockResolvedValueOnce(page(["https://example.com/a"], 2)).mockResolvedValueOnce(page(["https://example.com/b"], 3));
    await expect(prepareRenderingRetry(meta(), read)).rejects.toThrow("changed");
    await expect(prepareRenderingRetry(meta(), async () => page(["https://example.com/a", "https://example.com/a"]))).rejects.toThrow("duplicate");
    await expect(prepareRenderingRetry(meta(), async () => page(["javascript:alert(1)"]))).rejects.toThrow("invalid URL");
  });
});
