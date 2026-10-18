import type { CrawlConfig, Job } from "../api/types";
import { number, useI18n } from "../i18n";
import { pageTimings } from "../lib/livePerformance";
import { effectiveBrowserConcurrency } from "../lib/performance";

export function LivePerformance({ job, config }: { job: Job; config?: CrawlConfig }) {
  const { t, lang } = useI18n();
  const timings = pageTimings(job.activity);
  const mode = job.rendering ?? config?.rendering.mode;
  const browsers = config && mode === "javascript" ? effectiveBrowserConcurrency(config) : null;
  return <section className="live-performance" aria-label={t("livePerformance")}>
    <h3>{t("livePerformance")}</h3>
    <dl>
      <div><dt>{t("renderMode")}</dt><dd>{mode ? t(mode === "javascript" ? "javascript" : "rawHtml") : "—"}</dd></div>
      {config && <>
        <div><dt>{t("threads")}</dt><dd>{number(config.speed.threads, lang)}</dd></div>
        <div><dt>{t("requestsPerSecond")}</dt><dd>{config.speed.max_requests_per_second === 0 ? t("repeatUnlimited") : number(config.speed.max_requests_per_second, lang)}</dd></div>
        {browsers !== null && <div><dt>{t("liveBrowserLimit")}</dt><dd>{number(browsers, lang)}</dd></div>}
        <div><dt>{t("scope")}</dt><dd>{t(config.mode === "list" ? "list" : config.urls.start_folder_only ? "liveFolderScope" : "liveFolderUnrestricted")}</dd></div>
      </>}
      <div><dt>{t("liveHttpAverage")}</dt><dd>{timings ? `${number(timings.responseMs, lang)} ms` : "—"}</dd></div>
      <div><dt>{t("liveProcessingAverage")}</dt><dd>{timings ? `${number(timings.processingMs, lang)} ms` : "—"}</dd></div>
    </dl>
    <p>{t(timings ? "liveTimingNote" : "liveTimingWaiting").replace("{count}", number(timings?.count ?? 0, lang))}</p>
    {config?.mode === "spider" && !config.urls.start_folder_only && <p>{t("liveFolderNote")}</p>}
  </section>;
}
