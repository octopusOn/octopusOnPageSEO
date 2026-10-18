import { useMemo, useState } from "react";
import { Activity, Search } from "lucide-react";
import type { ActivityKind, Job, UrlActivity } from "../api/types";
import { number, useI18n, type Key } from "../i18n";
import { filterActivity, type ActivityFilter } from "../lib/jobs";

const kinds: Record<ActivityKind, Key> = {
  page: "page", resource: "liveResource", external: "external",
  robots: "robots", sitemap: "sitemaps", site_file: "liveSiteFile",
};
const states: Record<UrlActivity["state"], Key> = {
  processing: "processing", completed: "completed", error: "activityError",
  blocked: "blocked", cancelled: "activityCancelled",
};

export function LiveActivity({ job }: { job: Job }) {
  const { t, lang } = useI18n();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ActivityFilter>("all");
  const recent = job.activity?.recent;
  const visible = useMemo(() => filterActivity(recent ?? [], query, filter), [recent, query, filter]);
  const seconds = (value: number) => `${number(value / 1000, lang)} s`;
  return (
    <section className="live-activity oc-panel" aria-labelledby="live-activity-title">
      <header className="live-section-heading">
        <div><Activity size={17} /><h2 id="live-activity-title">{t("liveActivity")}</h2></div>
        <span className="oc-badge info">{t("currentlyProcessing")} · {number(job.activity?.active.length ?? job.progress.in_flight, lang)}</span>
      </header>
      {!job.activity ? <p className="live-empty">{t("activityUnavailable")}</p> : <>
        <div className="live-active" aria-label={t("currentlyProcessing")}>
          {job.activity.active.length ? <ul>{job.activity.active.map((entry) => (
            <li key={entry.id}>
              <span className="live-active-dot" />
              <span className="live-operation-kind">{t(kinds[entry.kind])}</span>
              <span className="live-active-url" title={entry.url}>{entry.url}</span>
              <small>{seconds(entry.elapsed_ms)}</small>
            </li>
          ))}</ul> : <p className="live-empty">{t("noActiveRequests")}</p>}
        </div>
        <div className="live-section-heading live-recent-heading">
          <h3>{t("recentRequests")}</h3>
          <span>{number(visible.length, lang)} / {number(recent?.length ?? 0, lang)}</span>
        </div>
        <div className="live-toolbar">
          <label className="live-search"><Search size={14} />
            <input aria-label={t("searchActivity")} placeholder={t("searchActivity")} value={query} onChange={(event) => setQuery(event.target.value)} />
          </label>
          <select className="oc-select" aria-label={t("filterActivity")} value={filter} onChange={(event) => setFilter(event.target.value as ActivityFilter)}>
            <option value="all">{t("allOperations")}</option>
            <option value="page">{t("pageOperations")}</option>
            <option value="resource">{t("resourceOperations")}</option>
            <option value="external">{t("external")}</option>
            <option value="errors">{t("onlyErrors")}</option>
          </select>
        </div>
        <div className="live-table-scroll" tabIndex={0} role="region" aria-label={t("recentRequests")}>
          <table className="live-table">
            <thead><tr><th>{t("urlAndTitle")}</th><th>{t("httpStatus")}</th><th>{t("status")}</th><th>{t("processingTime")}</th></tr></thead>
            <tbody>{visible.map((entry) => (
              <tr key={entry.id} className={entry.state === "error" ? "live-row-error" : ""}>
                <td><div className="live-url" title={entry.url}>{entry.url}</div>
                  <div className={`live-url-detail ${entry.error || entry.render_error ? "live-error-text" : ""}`} title={entry.error ?? entry.render_error ?? entry.title ?? undefined}>
                    <span>{t(kinds[entry.kind])}</span><i>·</i>{entry.error ?? entry.render_error ?? entry.title ?? t(states[entry.state])}
                  </div>
                </td>
                <td><span className={`status-code s${Math.floor((entry.status ?? 0) / 100)}`}>{entry.status ?? "—"}</span></td>
                <td><span className={`live-result ${entry.state}`}>{t(states[entry.state])}</span>
                  {entry.rendered && <span className="live-js rendered" aria-label={t("rendered")} title={t("rendered")}>JS ✓</span>}
                  {entry.render_error && entry.state !== "cancelled" && <span className="live-js failed" title={entry.render_error}>{t("jsFallback")}</span>}
                </td>
                <td className="live-time" title={entry.response_ms === null ? undefined : `HTTP: ${entry.response_ms} ms`}>{seconds(entry.elapsed_ms)}</td>
              </tr>
            ))}</tbody>
          </table>
          {!visible.length && <p className="live-empty">{t(recent?.length ? "noActivityMatches" : "activityWaiting")}</p>}
        </div>
        <footer className="live-feed-note">{t("recentLimit")}</footer>
      </>}
    </section>
  );
}
