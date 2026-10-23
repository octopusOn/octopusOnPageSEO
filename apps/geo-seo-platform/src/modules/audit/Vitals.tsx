// Core Web Vitals: what real Chrome users experience on the site (phones and desktops, with the
// last weeks), and PageSpeed runs of its key pages with what would save the most time. Free
// with the user's own Google API key.

import { Fragment, useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, CircleX, Gauge, Monitor, Play, PlugZap, Smartphone } from "lucide-react";
import { Button, Empty, Field, Loading, Stat } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { FieldData, FieldHistory, OriginVitals, PageVitals, ProjectRow, VitalMetric, VitalName } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { Entity } from "../../lib/Entity";
import { explain } from "../../lib/errors";
import { formatNumber, relativeTime } from "../../lib/format";
import { ratingOf, VITALS, vitalText } from "./measures";

function Distribution({ metric }: { metric: VitalMetric }) {
  const { t, locale } = useI18n();
  const share = (value: number | null) => Math.max(0, (value ?? 0) * 100);
  if (metric.good === null) return null;
  const parts: [string, number][] = [
    ["good", share(metric.good)],
    ["needs", share(metric.needs)],
    ["poor", share(metric.poor)],
  ];
  return (
    <span className="vital-bar" role="img" aria-label={parts.map(([name, value]) => `${t(`audit.rating_${name}`)} ${new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value)}%`).join(", ")}>
      {parts.map(([name, value]) => (
        <span key={name} className={`vital-part ${name}`} style={{ width: `${value}%` }} />
      ))}
    </span>
  );
}

function MetricTile({ name, metric }: { name: VitalName; metric: VitalMetric | null }) {
  const { t, locale } = useI18n();
  const value = metric?.p75 ?? null;
  const rating = value === null ? null : ratingOf(name, value);
  return (
    <div className={`vital-tile ${rating ?? "none"}`}>
      <span className="vital-name">
        {t(`audit.vital_${name}`)}
        {(name === "lcp" || name === "inp" || name === "cls") && <span className="oc-chip small">{t("audit.coreVital")}</span>}
      </span>
      <strong>{vitalText(name, value, locale)}</strong>
      <span className="faint small">{rating ? t(`audit.rating_${rating}`) : t("audit.noFieldData")}</span>
      {metric && <Distribution metric={metric} />}
    </div>
  );
}

function FieldTiles({ field }: { field: FieldData }) {
  return (
    <div className="vital-tiles">
      {VITALS.map((name) => (
        <MetricTile key={name} name={name} metric={field[name]} />
      ))}
    </div>
  );
}

function HistoryChart({ history }: { history: FieldHistory }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 52, right: 52, top: 36, bottom: 28 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: history.dates.map((date) => new Date(`${date}T12:00:00Z`).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", ...axis },
        { type: "value", ...axis, splitLine: { show: false } },
      ],
      series: [
        { name: t("audit.vital_lcp"), type: "line", smooth: true, connectNulls: true, data: history.lcp },
        { name: t("audit.vital_inp"), type: "line", smooth: true, connectNulls: true, data: history.inp },
        { name: t("audit.vital_cls"), type: "line", smooth: true, connectNulls: true, yAxisIndex: 1, lineStyle: { type: "dashed" }, data: history.cls },
      ],
    };
  }, [history, tokens, t, locale]);
  if (history.dates.length < 2) return null;
  return <EChart option={option} height={240} />;
}

function OriginCard({ origin }: { origin: OriginVitals }) {
  const { t } = useI18n();
  return (
    <section className="sub-panel vital-origin">
      <h4>
        {origin.device === "phone" ? <Smartphone size={15} aria-hidden="true" /> : <Monitor size={15} aria-hidden="true" />} {t(`audit.device_${origin.device}`)}
        {origin.passes !== null && (
          <span className={`oc-badge ${origin.passes ? "good" : "critical"}`}>
            {origin.passes ? <CheckCircle2 size={12} aria-hidden="true" /> : <CircleX size={12} aria-hidden="true" />} {origin.passes ? t("audit.vitalsPass") : t("audit.vitalsFail")}
          </span>
        )}
      </h4>
      <span className="faint small">
        {origin.origin}
        {origin.field?.first_date && origin.field.last_date && <> · {t("audit.fieldPeriod", { start: origin.field.first_date, end: origin.field.last_date })}</>}
      </span>
      {origin.field ? <FieldTiles field={origin.field} /> : <p className="faint small">{t("audit.noFieldData")}</p>}
      {origin.history && <HistoryChart history={origin.history} />}
    </section>
  );
}

function PageRows({ pages }: { pages: PageVitals[] }) {
  const { t, locale } = useI18n();
  const [open, setOpen] = useState<string | null>(null);
  return (
    <div className="table-scroll">
      <table className="oc-table compact vitals-table">
        <thead>
          <tr>
            <th>{t("audit.colPage")}</th>
            <th>{t("audit.device")}</th>
            <th className="numeric">{t("audit.psiScore")}</th>
            <th className="numeric">{t("audit.vital_lcp")}</th>
            <th className="numeric">{t("audit.vital_cls")}</th>
            <th className="numeric">{t("audit.tbt")}</th>
            <th>{t("audit.fieldShort")}</th>
          </tr>
        </thead>
        <tbody>
          {pages.map((page) => {
            const key = `${page.url}|${page.device}`;
            const lab = page.lab;
            const score = lab?.score ?? null;
            return (
              <Fragment key={key}>
                <tr>
                  <td className="cell-url">
                    <button type="button" className="link-button" aria-expanded={open === key} onClick={() => setOpen(open === key ? null : key)}>
                      {page.url.replace(/^https?:\/\/(www\.)?/, "")}
                    </button>
                  </td>
                  <td>{page.device === "phone" ? <Smartphone size={14} aria-label={t("audit.device_phone")} /> : <Monitor size={14} aria-label={t("audit.device_desktop")} />}</td>
                  <td className="numeric">{score === null ? "—" : <span className={`score-chip ${score >= 90 ? "good" : score >= 50 ? "fair" : "poor"}`}>{score}</span>}</td>
                  <td className={`numeric rating-${lab?.lcp_ms == null ? "none" : ratingOf("lcp", lab.lcp_ms)}`}>{vitalText("lcp", lab?.lcp_ms ?? null, locale)}</td>
                  <td className={`numeric rating-${lab?.cls == null ? "none" : ratingOf("cls", lab.cls)}`}>{vitalText("cls", lab?.cls ?? null, locale)}</td>
                  <td className={`numeric rating-${lab?.tbt_ms == null ? "none" : ratingOf("tbt", lab.tbt_ms)}`}>{vitalText("tbt", lab?.tbt_ms ?? null, locale)}</td>
                  <td>{page.passes === null ? <span className="faint">—</span> : page.passes ? <CheckCircle2 size={14} className="good" aria-label={t("audit.vitalsPass")} /> : <CircleX size={14} className="warn" aria-label={t("audit.vitalsFail")} />}</td>
                </tr>
                {open === key && (
                  <tr className="vital-detail">
                    <td colSpan={7}>
                      <div className="page-stack">
                        <Entity kind="url" value={page.url}>
                          <span className="cell-url">{page.url}</span>
                        </Entity>
                        {page.field && <FieldTiles field={page.field} />}
                        <h5>{t("audit.opportunities")}</h5>
                        {lab && lab.opportunities.length > 0 ? (
                          <ul className="check-list">
                            {lab.opportunities.map((opportunity) => (
                              <li key={opportunity.id}>
                                <span>{opportunity.title}</span>
                                <strong>{t("audit.saves", { s: new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(opportunity.savings_ms / 1_000) })}</strong>
                              </li>
                            ))}
                          </ul>
                        ) : (
                          <p className="faint small">{t("audit.noOpportunities")}</p>
                        )}
                        <span className="faint small">{t("audit.checkedOn", { day: page.day })}</span>
                      </div>
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export function VitalsView({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { go, fail, notify, jobs } = useShell();
  const queryClient = useQueryClient();
  const setup = useQuery({ queryKey: ["audit", "vitals-setup", project.id], queryFn: () => api.vitalsSetup(project.id) });
  const report = useQuery({ queryKey: ["audit", "vitals", project.id], queryFn: () => api.vitalsReport(project.id) });
  const [pages, setPages] = useState("");
  const [desktop, setDesktop] = useState(false);
  useEffect(() => {
    if (setup.data && !pages) setPages(setup.data.pages.join("\n"));
  }, [setup.data, pages]);
  const running = jobs.find((job) => job.kind === "web-vitals" && job.project_id === project.id && job.status === "running");
  const start = useMutation({
    mutationFn: () =>
      api.startVitals(
        project.id,
        pages
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
        desktop,
      ),
    onSuccess: () => {
      notify(t("audit.vitalsStarted"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
  });
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) return <p className="warn">{explain(setup.error, t).message}</p>;
  const data = report.data;
  const checked = (data?.origins.length ?? 0) + (data?.pages.length ?? 0) > 0;
  const passing = data?.origins.find((origin) => origin.device === "phone")?.passes ?? null;
  return (
    <div className="page-stack">
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("audit.vitalsTitle")}</h3>
            <span className="faint">{t("audit.vitalsHint")}</span>
          </div>
        </header>
        {!setup.data.key_ready ? (
          <div className="oc-banner info" role="status">
            <PlugZap size={14} aria-hidden="true" />
            <span>{t("audit.vitalsNeedKey")}</span>
            <Button small onClick={() => go({ page: "sources" })}>
              {t("common.goToSources")}
            </Button>
          </div>
        ) : (
          <div className="vitals-setup">
            <Field label={t("audit.vitalsPages")} hint={t("audit.vitalsPagesHint", { n: 10 })}>
              <textarea className="oc-input" rows={4} value={pages} onChange={(event) => setPages(event.target.value)} />
            </Field>
            <div className="report-actions">
              <label className="oc-check">
                <input type="checkbox" checked={desktop} onChange={(event) => setDesktop(event.target.checked)} />
                <span>{t("audit.vitalsDesktop")}</span>
              </label>
              <span className="faint small">{setup.data.last_checked ? t("audit.lastChecked", { time: relativeTime(setup.data.last_checked, locale) }) : t("audit.neverChecked")}</span>
              <Button primary small busy={start.isPending || Boolean(running)} disabled={Boolean(running)} onClick={() => start.mutate()}>
                <Play size={14} />
                {t("audit.vitalsCheck")}
              </Button>
            </div>
            {running && (
              <div className="rank-progress" aria-live="polite">
                <div className="oc-progress" aria-hidden="true">
                  <div style={{ width: `${running.total ? (running.done / running.total) * 100 : 5}%` }} />
                </div>
                <span className="small">{t("audit.vitalsRunning", { done: formatNumber(running.done, locale), total: formatNumber(running.total, locale) })}</span>
              </div>
            )}
            <p className="faint small">{t("audit.vitalsFree")}</p>
          </div>
        )}
      </section>
      {!checked ? (
        <div className="oc-card">
          <Empty icon={<Gauge size={26} />} title={t("audit.vitalsEmptyTitle")}>
            <p>{t("audit.vitalsEmptyText")}</p>
          </Empty>
        </div>
      ) : (
        data && (
          <>
            <div className="oc-stats">
              <Stat label={t("audit.vitalsAssessment")} value={passing === null ? "—" : passing ? t("audit.vitalsPass") : t("audit.vitalsFail")} hint={t("audit.vitalsAssessmentHint")} tone={passing === false ? "bad" : passing ? "good" : undefined} />
              <Stat label={t("audit.pagesChecked")} value={formatNumber(new Set(data.pages.map((page) => page.url)).size, locale)} hint={t("audit.pagesCheckedHint")} />
              <Stat
                label={t("audit.averageScore")}
                value={(() => {
                  const scores = data.pages.map((page) => page.lab?.score).filter((score): score is number => typeof score === "number");
                  return scores.length ? formatNumber(Math.round(scores.reduce((sum, score) => sum + score, 0) / scores.length), locale) : "—";
                })()}
                hint={t("audit.averageScoreHint")}
              />
            </div>
            {data.origins.length > 0 ? (
              <div className="two-columns">
                {data.origins.map((origin) => (
                  <OriginCard key={`${origin.origin}|${origin.device}`} origin={origin} />
                ))}
              </div>
            ) : (
              <p className="faint">{t("audit.noOriginData")}</p>
            )}
            {data.pages.length > 0 && (
              <section className="oc-panel report">
                <header className="report-head">
                  <div className="report-title">
                    <h3 className="oc-panel-title">{t("audit.vitalsPagesTitle")}</h3>
                    <span className="faint">{t("audit.vitalsPagesTableHint")}</span>
                  </div>
                </header>
                <PageRows pages={data.pages} />
              </section>
            )}
          </>
        )
      )}
    </div>
  );
}
