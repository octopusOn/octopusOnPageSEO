import { AlertTriangle, CheckCircle2, CircleStop, Hourglass, ShieldAlert, X } from "lucide-react";
import type { CheckPage, Job } from "../api/types";
import { number, time, usd, useI18n, type Key } from "../i18n";
import { Button } from "./ui";

/** What a job is doing, in words. */
export function stepText(job: Job, t: (key: Key, values?: Record<string, string | number>) => string, lang: "tr" | "en") {
  if (job.check) return t("stepCheck");
  if (job.waiting_until) return t("waitingQuota", { time: time(job.waiting_until, lang) });
  if (job.kind === "cluster") return t("stepCluster", { method: t(`method.${job.step}` as Key) });
  if (job.kind === "google-login") return t("stepLogin");
  if (job.kind === "account-login") return job.step && job.step !== "browser" ? t("accountCode", { code: job.step }) : t("stepLogin");
  if (job.kind === "search-console") return t("stepSearchConsole");
  if (job.kind === "serp") return t("stepSerp", { done: job.done, total: job.total });
  return job.step ? t("stepSource", { source: t(`source.${job.step}` as Key) }) : t("starting");
}

/** A check page in the browser window: what the person does there, and how long it waits. */
export function CheckBanner({ check, label }: { check: CheckPage; label?: string }) {
  const { t, lang } = useI18n();
  return (
    <div className="oc-banner warning check-page" role="alert">
      <ShieldAlert size={18} />
      <div>
        <strong>
          {label && `${label} · `}
          {t("checkTitle")}
        </strong>
        <p>{t(`check.${check.kind}` as Key)}</p>
        <p className="small">{t("checkUntil", { time: time(check.until, lang) })}</p>
        <code className="oc-code">{check.url}</code>
      </div>
    </div>
  );
}

/** A running, finished or failed job. */
export function JobCard({ job, onStop, onDismiss }: { job: Job; onStop: (id: string) => void; onDismiss?: () => void }) {
  const { t, lang } = useI18n();
  const share = job.total > 0 ? Math.min(1, job.done / job.total) : 0;
  if (job.status === "running")
    return (
      <div className="oc-panel job-card" role="status">
        <div className="row between">
          <strong className="row">
            <Hourglass size={15} />
            {t(`job.${job.kind}` as Key)} · {job.label}
          </strong>
          <Button small onClick={() => onStop(job.id)}>
            <CircleStop size={14} />
            {t("stop")}
          </Button>
        </div>
        <p className="faint">{stepText(job, t, lang)}</p>
        {job.check && <CheckBanner check={job.check} />}
        {job.total > 0 && (
          <div className="oc-progress" aria-label={t("progress")}>
            <span style={{ width: `${share * 100}%` }} />
          </div>
        )}
        {job.kind !== "cluster" && job.kind !== "google-login" && job.kind !== "account-login" && (
          <p className="faint small">
            {job.total > 0 && `${job.done}/${job.total} · `}
            {t("foundSoFar", { count: number(job.found, lang, 0) })}
          </p>
        )}
      </div>
    );
  const failed = job.status === "failed";
  const partial = job.status === "partial";
  return (
    <div className={`oc-banner ${failed ? "error" : partial ? "warning" : "success"} job-card`} role={failed ? "alert" : "status"}>
      {failed || partial ? <AlertTriangle size={18} /> : <CheckCircle2 size={18} />}
      <div>
        <strong>
          {t(`job.${job.kind}` as Key)} · {t(`status.${job.status}` as Key)}
        </strong>
        {job.status !== "cancelled" && job.kind !== "cluster" && job.kind !== "google-login" && job.kind !== "account-login" && (
          <p>
            {t("jobSummary", { found: number(job.found, lang, 0), cost: usd(job.cost_usd, lang) })}
          </p>
        )}
        {job.error && <p>{job.error}</p>}
        {job.failures && job.failures.length > 1 && (
          <ul className="failures">
            {job.failures.slice(1).map((failure) => (
              <li key={`${failure.provider}-${failure.error}`}>
                <b>{failure.provider}</b> · {failure.error}
              </li>
            ))}
          </ul>
        )}
      </div>
      {onDismiss && (
        <Button className="icon ghost" onClick={onDismiss} aria-label={t("close")}>
          <X size={16} />
        </Button>
      )}
    </div>
  );
}
