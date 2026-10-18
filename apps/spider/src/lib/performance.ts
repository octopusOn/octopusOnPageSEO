import type { CrawlConfig } from "../api/types";

export const speedProfiles = {
  gentle: { threads: 2, max_requests_per_second: 2, browsers: 1 },
  balanced: { threads: 5, max_requests_per_second: 5, browsers: 2 },
  fast: { threads: 10, max_requests_per_second: 20, browsers: 4 },
} as const;
export type SpeedProfile = keyof typeof speedProfiles;

/** Only an explicit profile choice changes these limits; preserve the rest of the plan. */
export function applySpeedProfile(config: CrawlConfig, profile: SpeedProfile): CrawlConfig {
  const { browsers, ...speed } = speedProfiles[profile];
  return {
    ...config,
    speed: { ...config.speed, ...speed },
    rendering: config.rendering.mode === "javascript"
      ? { ...config.rendering, concurrency: browsers }
      : { ...config.rendering },
  };
}

export function activeSpeedProfile(config: CrawlConfig): SpeedProfile | null {
  return (Object.keys(speedProfiles) as SpeedProfile[]).find(key => {
    const profile = speedProfiles[key];
    return config.speed.threads === profile.threads
      && config.speed.max_requests_per_second === profile.max_requests_per_second
      && (config.rendering.mode !== "javascript" || config.rendering.concurrency === profile.browsers);
  }) ?? null;
}

/** The worker pool can also limit how many pages reach the browser. */
export function effectiveBrowserConcurrency(config: CrawlConfig): number | null {
  const { threads } = config.speed;
  const { concurrency } = config.rendering;
  return Number.isInteger(threads) && threads >= 1 && threads <= 64
    && Number.isInteger(concurrency) && concurrency >= 1 && concurrency <= 8
    ? Math.min(threads, concurrency) : null;
}

/** A cleared number field is invalid, rather than silently selecting an unlimited rate. */
export function performanceNumber(value: string): number {
  return value.trim() === "" ? Number.NaN : Number(value);
}
