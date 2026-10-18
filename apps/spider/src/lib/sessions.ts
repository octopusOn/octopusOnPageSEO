import type { CrawlConfig, CrawlMeta, Job } from "../api/types";
import { isActiveJob, jobStatusLabel } from "./jobs";
import { defaultConfig } from "../api/types";
import type { Key } from "../i18n";

export type JobsStatus = "loading" | "ready" | "unavailable";

/** Saved metadata alone cannot establish whether a process is still crawling. */
export function savedCrawlState(crawl: CrawlMeta, jobs: Job[], status: JobsStatus): {
  label: Key;
  active?: Job;
  inactive: boolean;
  canRepeat: boolean;
} {
  const active = jobs.find((job) => job.crawl_id === crawl.id && isActiveJob(job));
  if (active) {
    return { label: jobStatusLabel(active), active, inactive: false, canRepeat: false };
  }
  if (status !== "ready") {
    const label = crawl.status === "running"
      ? status === "loading" ? "checkingCrawlStatus" : "crawlStatusUnavailable"
      : crawl.status;
    return { label, inactive: false, canRepeat: false };
  }
  return {
    label: crawl.status === "running" ? "interrupted" : crawl.status,
    inactive: crawl.status === "running",
    canRepeat: true,
  };
}

/** Copy the original plan, preserving limits and scope, without changing saved preferences. */
export function prepareRepeat(crawl: CrawlMeta): CrawlConfig {
  const config = structuredClone(crawl.config);
  // Historical HTML crawls may predate the rendering settings object.
  config.rendering = { ...defaultConfig.rendering, ...config.rendering };
  config.start_urls = [...crawl.start_urls];
  return config;
}
