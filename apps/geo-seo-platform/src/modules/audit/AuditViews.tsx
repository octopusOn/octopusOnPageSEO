// The Site Audit's other views of a crawl: how scores moved across audits, internal links
// (click depth, pages only the sitemap knows, weakly linked pages, internal PageRank) and
// hreflang annotations.

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Languages } from "lucide-react";
import { Button, DataTable, Empty, Loading, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AuditRow, LinkedPage, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { Entity } from "../../lib/Entity";
import { explain } from "../../lib/errors";
import { formatNumber } from "../../lib/format";
import { tableLabels } from "../../lib/tables";
import { finishedAudits } from "./measures";

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

/** Health, GEO readiness and findings audit after audit. */
export function AuditTrend({ audits }: { audits: AuditRow[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const rows = useMemo(() => finishedAudits(audits), [audits]);
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 44, right: 56, top: 36, bottom: 28 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: rows.map((row) => new Date(row.started_at).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", min: 0, max: 100, ...axis },
        { type: "value", minInterval: 1, ...axis, splitLine: { show: false } },
      ],
      series: [
        { name: t("audit.health"), type: "line", smooth: true, symbolSize: 6, data: rows.map((row) => row.health) },
        { name: t("audit.geoScore"), type: "line", smooth: true, symbolSize: 6, data: rows.map((row) => row.geo) },
        { name: t("audit.findings"), type: "bar", yAxisIndex: 1, barMaxWidth: 18, itemStyle: { opacity: 0.4 }, data: rows.map((row) => row.issues) },
      ],
    };
  }, [rows, tokens, t, locale]);
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("audit.trendTitle")}</h3>
          <span className="faint">{t("audit.trendHint")}</span>
        </div>
      </header>
      {rows.length < 2 ? <p className="faint">{t("audit.trendOne")}</p> : <EChart option={option} height={260} />}
      {rows.length > 0 && (
        <DataTable
          rows={rows.slice().reverse()}
          columns={[
            { key: "date", label: t("audit.trendDate"), render: (row) => new Date(row.started_at).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" }), sort: (row) => row.started_at },
            { key: "health", label: t("audit.health"), render: (row) => (row.health === null ? "—" : one(row.health, locale)), sort: (row) => row.health, numeric: true },
            { key: "geo", label: t("audit.geoScore"), render: (row) => (row.geo === null ? "—" : one(row.geo, locale)), sort: (row) => row.geo, numeric: true },
            { key: "pages", label: t("audit.pages"), render: (row) => formatNumber(row.pages, locale), sort: (row) => row.pages, numeric: true },
            { key: "issues", label: t("audit.findings"), render: (row) => formatNumber(row.issues, locale), sort: (row) => row.issues, numeric: true },
          ]}
          rowKey={(row) => String(row.id)}
          initialSort={{ key: "date", direction: "desc" }}
          tableId="audit-trend"
          empty={t("overview.emptyRows")}
          labels={tableLabels(t, locale)}
        />
      )}
    </section>
  );
}

function PageCell({ page }: { page: { url: string; title?: string } }) {
  return (
    <span className="link-cell">
      {page.title && <strong className="cell-main">{page.title}</strong>}
      <Entity kind="url" value={page.url}>
        <span className="cell-url faint small">{page.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
      </Entity>
    </span>
  );
}

/** Internal links of an audit's crawl. */
export function LinksView({ project, auditId }: { project: ProjectRow; auditId: number | null }) {
  const { t, locale } = useI18n();
  const links = useQuery({ queryKey: ["audit", "links", project.id, auditId], queryFn: () => api.auditLinks(project.id, auditId) });
  const columns = (weak: boolean): Column<LinkedPage>[] => [
    { key: "page", label: t("audit.colPage"), render: (row) => <PageCell page={row} />, sort: (row) => row.url },
    { key: "rank", label: t("audit.colRank"), help: t("audit.rankHelp"), render: (row) => <span className="rank-score">{one(row.rank, locale)}</span>, sort: (row) => row.rank, numeric: true },
    { key: "inlinks", label: t("audit.colInlinks"), render: (row) => formatNumber(row.inlinks, locale), sort: (row) => row.inlinks, numeric: true },
    { key: "depth", label: t("audit.colDepth"), render: (row) => (row.depth === null ? "—" : row.depth), sort: (row) => row.depth, numeric: true },
    ...(weak ? [] : [{ key: "indexability", label: t("audit.colIndexability"), optional: true, render: (row: LinkedPage) => row.indexability, sort: (row: LinkedPage) => row.indexability }]),
  ];
  if (links.isLoading) return <Loading label={t("audit.linksComputing")} />;
  if (links.error) return <p className="warn">{explain(links.error, t).message}</p>;
  const data = links.data;
  if (!data) return <p className="faint">{t("audit.noCrawl")}</p>;
  const most = Math.max(1, ...data.depth.map(([, n]) => n));
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat label={t("audit.linkedPages")} value={formatNumber(data.pages, locale)} hint={t("audit.linkedPagesHint")} />
        <Stat label={t("audit.averageDepth")} value={data.average_depth === null ? "—" : one(data.average_depth, locale)} hint={t("audit.deepHint", { n: formatNumber(data.deep, locale) })} tone={data.deep > 0 ? "bad" : undefined} />
        <Stat label={t("audit.orphans")} value={data.sitemaps ? formatNumber(data.orphan_total, locale) : "—"} hint={data.sitemaps ? t("audit.orphansHint") : t("audit.noSitemaps")} tone={data.orphan_total > 0 ? "bad" : undefined} />
        <Stat label={t("audit.weak")} value={formatNumber(data.weak_total, locale)} hint={t("audit.weakHint")} tone={data.weak_total > 0 ? "bad" : undefined} />
      </div>
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("audit.depthTitle")}</h4>
          <ol className="bars">
            {data.depth.map(([depth, n]) => (
              <li key={depth}>
                <span className="bar-label">{depth >= 6 ? t("audit.depthMore", { n: 6 }) : t("audit.depthClicks", { n: depth })}</span>
                <span className="bar-track" aria-hidden="true">
                  <span className={`bar-fill${depth >= 4 ? " poor" : ""}`} style={{ width: `${(n / most) * 100}%` }} />
                </span>
                <span className="bar-value">{formatNumber(n, locale)}</span>
              </li>
            ))}
          </ol>
          <p className="faint small">{t("audit.depthHint")}</p>
        </section>
        <section className="sub-panel">
          <h4>{t("audit.orphanTitle")}</h4>
          {!data.sitemaps ? (
            <p className="faint small">{t("audit.noSitemaps")}</p>
          ) : data.orphans.length === 0 ? (
            <p className="faint small">{t("audit.noOrphans")}</p>
          ) : (
            <ul className="plain-list small orphan-list">
              {data.orphans.map((orphan) => (
                <li key={orphan.url}>
                  <Entity kind="url" value={orphan.url}>
                    <span className="cell-url">{orphan.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
                  </Entity>{" "}
                  <span className="faint">{orphan.status === null ? t("audit.notCrawled") : `HTTP ${orphan.status}`}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
      <section className="sub-panel">
        <h4>{t("audit.weakTitle")}</h4>
        <p className="faint small">{t("audit.weakText")}</p>
        <DataTable rows={data.weak} columns={columns(true)} rowKey={(row) => row.url} initialSort={{ key: "rank", direction: "asc" }} tableId="audit-weak" empty={t("audit.noWeak")} labels={tableLabels(t, locale)} />
      </section>
      <section className="sub-panel">
        <h4>{t("audit.topTitle")}</h4>
        <p className="faint small">{t("audit.topText")}</p>
        <DataTable rows={data.top} columns={columns(false)} rowKey={(row) => row.url} initialSort={{ key: "rank", direction: "desc" }} tableId="audit-top" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      </section>
    </div>
  );
}

/** Hreflang annotations of an audit's crawl. */
export function HreflangView({ project, auditId }: { project: ProjectRow; auditId: number | null }) {
  const { t, lang, locale } = useI18n();
  const [offset, setOffset] = useState(0);
  const report = useQuery({ queryKey: ["audit", "hreflang", project.id, auditId, lang, offset], queryFn: () => api.auditHreflang(project.id, auditId, lang, offset) });
  if (report.isLoading) return <Loading label={t("common.loading")} />;
  if (report.error) return <p className="warn">{explain(report.error, t).message}</p>;
  const data = report.data;
  if (!data) return <p className="faint">{t("audit.noCrawl")}</p>;
  if (data.pages === 0) {
    return (
      <div className="oc-card">
        <Empty icon={<Languages size={26} />} title={t("audit.noHreflangTitle")}>
          <p>{t("audit.noHreflangText")}</p>
        </Empty>
      </div>
    );
  }
  const shown = ["page", "lang", "target", "target_status", "return_link"].filter((name) => data.columns.includes(name));
  const at = shown.map((name) => data.columns.indexOf(name));
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat label={t("audit.hreflangPages")} value={formatNumber(data.pages, locale)} hint={t("audit.hreflangPagesHint")} />
        <Stat label={t("audit.hreflangLanguages")} value={formatNumber(data.languages.length, locale)} hint={data.languages.slice(0, 6).map(([code]) => code).join(", ")} />
        <Stat label={t("audit.hreflangProblems")} value={formatNumber(data.issues.reduce((sum, issue) => sum + issue.count, 0), locale)} hint={t("audit.hreflangProblemsHint")} />
      </div>
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("audit.hreflangChecks")}</h4>
          <ul className="check-list">
            {data.issues.map((issue) => (
              <li key={issue.id} className={issue.count > 0 ? "bad" : "good"}>
                <span>{issue.title}</span>
                <strong>{formatNumber(issue.count, locale)}</strong>
              </li>
            ))}
          </ul>
        </section>
        <section className="sub-panel">
          <h4>{t("audit.hreflangLanguages")}</h4>
          <div className="term-chips">
            {data.languages.map(([code, pages]) => (
              <span key={code} className="oc-chip small">
                {code} <span className="faint">{formatNumber(pages, locale)}</span>
              </span>
            ))}
          </div>
        </section>
      </div>
      <section className="sub-panel">
        <h4>{t("audit.hreflangRows", { n: formatNumber(data.total, locale) })}</h4>
        <div className="table-scroll">
          <table className="oc-table compact">
            <thead>
              <tr>
                {shown.map((name) => (
                  <th key={name}>{t(`audit.hreflangCol_${name}`)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {data.rows.map((row, index) => (
                <tr key={index}>
                  {at.map((column, place) => (
                    <td key={place} className={shown[place] === "page" || shown[place] === "target" ? "cell-url" : undefined}>
                      {row[column] ?? ""}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {data.total > data.rows.length && (
          <div className="pager">
            <Button small disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - 200))} aria-label={t("audit.previousPage")}>
              <ChevronLeft size={14} />
            </Button>
            <span className="faint small">
              {formatNumber(offset + 1, locale)}–{formatNumber(offset + data.rows.length, locale)} / {formatNumber(data.total, locale)}
            </span>
            <Button small disabled={offset + data.rows.length >= data.total} onClick={() => setOffset(offset + 200)} aria-label={t("audit.nextPage")}>
              <ChevronRight size={14} />
            </Button>
          </div>
        )}
      </section>
    </div>
  );
}

