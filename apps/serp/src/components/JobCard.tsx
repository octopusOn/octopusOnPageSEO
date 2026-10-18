import { AlertTriangle, CircleStop, ShieldAlert, X } from "lucide-react";
import type { Attempt, Job } from "../api/types";
import { number, useI18n, type Key } from "../i18n";
import { Button } from "./ui";

/** What a job is doing, in words. */
export function stepText(job: Job, t: (key: Key, values?: Record<string, string | number>) => string) {
  const [, detail = ""] = (job.detail ?? "").split(":");
  switch (job.step) {
    case "waiting":
      return t("stepWaiting", { seconds: detail });
    case "page":
      return t("stepPage", { page: detail.split("/")[0] ?? "1" });
    case "crawl":
      return t("stepCrawl");
    case "domains":
      return t("stepDomains");
    case "volume":
      return t("stepVolume");
    case "scoring":
      return t("stepScoring");
    case "keyword":
      return t("stepKeyword", { keyword: job.detail ?? "" });
    case "account":
      return t("accountBrowser");
    default:
      return t("stepSearch");
  }
}

function attemptText(attempt: Attempt, skipped: string) {
  if (attempt.outcome === "success") return "✓";
  if (attempt.outcome === "skipped") return `${skipped}${attempt.message ? ` · ${attempt.message}` : ""}`;
  return attempt.outcome.failed + (attempt.message ? ` · ${attempt.message}` : "");
}

/** A running or failed job: progress, a check page waiting for the person, or what failed. */
export function JobCard({ job, onStop, onDismiss }: { job: Job; onStop: (id: string) => void; onDismiss?: () => void }) {
  const { t, lang } = useI18n();
  const active = job.status === "running" || job.status === "challenge";
  const share = job.total > 0 ? Math.min(1, job.done / job.total) : 0;
  if (job.status === "failed")
    return (
      <div className="oc-banner error job-card" role="alert">
        <AlertTriangle size={18} />
        <div>
          <strong>
            {t("jobFailed")} · {job.label}
          </strong>
          <p>{job.error}</p>
          {job.attempts && job.attempts.length > 0 && (
            <ul className="attempts">
              {job.attempts.map((attempt, index) => (
                <li key={`${attempt.provider}-${index}`}>
                  <b>{attempt.provider}</b> {attemptText(attempt, t("skipped"))}
                </li>
              ))}
            </ul>
          )}
          {job.diagnostic && <p className="faint">{t("diagnosticSaved", { path: job.diagnostic })}</p>}
        </div>
        {onDismiss && (
          <Button small className="icon ghost dismiss" aria-label={t("close")} onClick={onDismiss}>
            <X size={15} />
          </Button>
        )}
      </div>
    );
  return (
    <div className="job-card oc-card" aria-live="polite">
      <div className="row between">
        <div className="job-title">
          <span className={`oc-radar ${active ? "scanning" : ""}`} aria-hidden="true" />
          <div>
            <b>{job.label}</b>
            <small>
              {job.status === "cancelled" ? t("jobCancelled") : job.status === "done" ? t("jobDone") : stepText(job, t)}
              {job.step === "crawl" || job.step === "keyword"
                ? ` · ${number(job.done, lang)}/${number(job.total, lang)}`
                : ""}
            </small>
          </div>
        </div>
        {active && (
          <Button small className="ghost" onClick={() => onStop(job.id)}>
            <CircleStop size={15} />
            {t("stop")}
          </Button>
        )}
      </div>
      <div className={`oc-progress ${job.total ? "" : "indeterminate"}`}>
        <div style={job.total ? { width: `${share * 100}%` } : undefined} />
      </div>
      {job.status === "challenge" && job.challenge && (
        <div className="oc-banner warning challenge" role="alert">
          <ShieldAlert size={18} />
          <div>
            <strong>{t("challengeTitle")}</strong>
            <p>
              {t(
                job.challenge.kind === "consent"
                  ? "challengeConsent"
                  : job.challenge.kind === "captcha"
                    ? "challengeCaptcha"
                    : "challengeBlocked",
              )}
            </p>
            <code className="oc-code">{job.challenge.url}</code>
          </div>
        </div>
      )}
    </div>
  );
}
