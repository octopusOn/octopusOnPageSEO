import { useState } from "react";
import { Pause, Play, ScanLine, Square, LoaderCircle } from "lucide-react";
import type { CrawlConfig, Job } from "../api/types";
import { useI18n, number, duration } from "../i18n";
import { Button } from "./ui";
import { LiveActivity } from "./LiveActivity";
import { jobStatusLabel } from "../lib/jobs";
import { LivePerformance } from "./LivePerformance";

export function LiveCrawl({ running, config, onControl }: {
  running: Job;
  config?: CrawlConfig;
  onControl: (action: "pause" | "resume" | "stop") => Promise<void>;
}) {
  const { t, lang } = useI18n();
  const [pending, setPending] = useState(false);
  const phase = running.phase ?? "crawling";
  const finalizing = phase === "finalizing";
  const label = jobStatusLabel(running);
  const disabled = pending || running.status === "stopping" || finalizing;
  async function control(action: "pause" | "resume" | "stop") {
    setPending(true);
    try { await onControl(action); } finally { setPending(false); }
  }
  const percent = Math.min(100, Math.max(0, running.progress.crawled / Math.max(1, running.progress.discovered) * 100));
  return (
    <div className="live-workspace">
      <section className="live-panel oc-panel">
        <header className="live-summary-heading">
          <span className="oc-tile large teal">{finalizing ? <LoaderCircle className="live-spin" size={25} /> : <ScanLine size={25} />}</span>
          <div><span className="oc-badge info" role="status">{t(label)}</span><h2>{running.url}</h2></div>
          <div className="row live-controls">
            <Button disabled={disabled} onClick={() => void control(running.status === "paused" ? "resume" : "pause")}>
              {running.status === "paused" ? <Play size={14} /> : <Pause size={14} />}{t(running.status === "paused" ? "resume" : "pause")}
            </Button>
            <Button disabled={disabled} onClick={() => void control("stop")}><Square size={12} />{t("stop")}</Button>
          </div>
        </header>
        <div className="live-stats">
          {([
            ["crawled", number(running.progress.crawled, lang)],
            ["queued", number(running.progress.queued, lang)],
            ["activeRequests", number(running.progress.in_flight, lang)],
            ["duration", duration(running.progress.elapsed_ms)],
          ] as const).map(([key, value]) => <div key={key}><strong>{value}</strong><span>{t(key)}</span></div>)}
        </div>
        <div className={`oc-progress ${finalizing ? "finalizing" : ""}`} role="progressbar" aria-label={t(finalizing ? "finalizing" : "crawled")} aria-valuemin={0} aria-valuemax={100} aria-valuenow={finalizing ? undefined : percent}>
          <i style={{ width: `${finalizing ? 100 : percent}%` }} />
        </div>
        <div className="live-summary-meta">
          <span>{t("knownURLs")} <b>{number(running.progress.discovered, lang)}</b></span>
          <span>{t("speed")} <b>{number(running.progress.crawled / Math.max(1, running.progress.elapsed_ms / 1000), lang)}</b></span>
          <span className={(running.activity?.request_errors ?? running.progress.errors) ? "live-error-text" : ""}>{t("httpErrors")} <b>{number(running.activity?.request_errors ?? running.progress.errors, lang)}</b></span>
          {running.activity && <span>{t("blocked")} <b>{number(running.activity.blocked, lang)}</b></span>}
          {running.rendering === "javascript" && <>
            <span>{t("rendered")} <b>{running.activity ? number(running.activity.rendered, lang) : "—"}</b></span>
            <span className={running.activity?.render_errors ? "live-error-text" : ""}>{t("jsErrors")} <b>{running.activity ? number(running.activity.render_errors, lang) : "—"}</b></span>
          </>}
        </div>
        <p className="live-phase-note">{t(finalizing ? "finalizingNote" : running.status === "paused" ? "pauseNote" : "progressDynamic")}</p>
        <LivePerformance job={running} config={config} />
      </section>
      <LiveActivity job={running} />
    </div>
  );
}
