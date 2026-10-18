import { defaultConfig, type CrawlConfig } from "../api/types";
import { stored } from "../i18n";
import { validViewport } from "./viewport";

export function normalizeUrls(text: string, mode: string) {
  const lines = (mode === "list" ? text.split(/\r\n|\n|\r/) : [text])
    .map((s) => s.trim())
    .filter(Boolean);
  if (!lines.length) throw new Error("invalidUrl");
  return [
    ...new Set(
      lines.map((raw) => {
        // URL parsing silently removes tabs and line breaks, potentially joining addresses.
        if (/[\u0000-\u001f\u007f]/.test(raw)) throw new Error("invalidUrl");
        const url = new URL(raw.includes("://") ? raw : `https://${raw}`);
        if (!["http:", "https:"].includes(url.protocol) || !url.hostname)
          throw new Error("invalidUrl");
        url.hash = "";
        return url.href;
      }),
    ),
  ];
}
export function loadConfig(): CrawlConfig {
  const raw = stored<Partial<CrawlConfig>>("spider.config", {});
  return {
    ...defaultConfig,
    ...raw,
    start_urls: [],
    speed: { ...defaultConfig.speed, ...raw.speed },
    limits: { ...defaultConfig.limits, ...raw.limits },
    follow: { ...defaultConfig.follow, ...raw.follow },
    urls: { ...defaultConfig.urls, ...raw.urls },
    storage: { ...defaultConfig.storage, ...raw.storage },
    rendering: { ...defaultConfig.rendering, ...raw.rendering },
  };
}
export function validConfig(config: CrawlConfig) {
  const { threads, max_requests_per_second: rate } = config.speed;
  const {
    max_urls: urls,
    max_depth: depth,
    timeout_seconds: timeout,
  } = config.limits;
  return (
    (config.schema_validation === undefined || typeof config.schema_validation === "boolean") &&
    (config.urls.start_folder_only === undefined || typeof config.urls.start_folder_only === "boolean") &&
    Number.isInteger(threads) &&
    threads >= 1 &&
    threads <= 64 &&
    Number.isFinite(rate) &&
    rate >= 0 &&
    Number.isInteger(urls) &&
    urls >= 0 &&
    urls <= 4294967295 &&
    (depth === null || (Number.isInteger(depth) && depth >= 0)) &&
    Number.isInteger(timeout) &&
    timeout >= 1 &&
    timeout <= 600 &&
    ["html", "javascript"].includes(config.rendering.mode) &&
    Number.isInteger(config.rendering.concurrency) &&
    config.rendering.concurrency >= 1 &&
    config.rendering.concurrency <= 8 &&
    Number.isInteger(config.rendering.timeout_seconds) &&
    config.rendering.timeout_seconds >= 1 &&
    config.rendering.timeout_seconds <= 120 &&
    Number.isInteger(config.rendering.wait_ms) &&
    config.rendering.wait_ms >= 0 &&
    config.rendering.wait_ms < config.rendering.timeout_seconds * 1000 &&
    validViewport(config.rendering.viewport)
  );
}
