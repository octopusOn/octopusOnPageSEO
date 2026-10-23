import { useMemo, useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Check, Download, KeyRound, Plus, Search, X } from "lucide-react";
import { Button, DataTable, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { CompetitorRow, PageRow, ProjectRow, SubdomainRow, TrafficRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { download, fold, toCsv } from "../../lib/report";
import { Entity } from "../../lib/Entity";
import { useGlossary } from "../../lib/glossary";
import { shortUrl, tableLabels } from "../../lib/tables";
import { subfolders, type FolderRow } from "./organic";
import { ReportFrame } from "./ReportFrame";

function top10(row: { organic: { pos_1: number; pos_2_3: number; pos_4_10: number } }): number {
  return row.organic.pos_1 + row.organic.pos_2_3 + row.organic.pos_4_10;
}

export function PagesReport({ projectId, domain, shownDomain }: { projectId: number; domain: string | undefined; shownDomain: string }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { params, go } = useShell();
  const [limit, setLimit] = useState(100);
  const [query, setQuery] = useState("");
  const columns: Column<PageRow>[] = [
    { key: "url", label: t("overview.colPage"), render: (row) => (
        <Entity kind="url" value={row.url}>
          <span className="cell-url">{shortUrl(row.url)}</span>
        </Entity>
      ), sort: (row) => row.url },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.organic.traffic, locale), sort: (row) => row.organic.traffic, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), help: gloss("organic_keywords"), render: (row) => formatNumber(row.organic.keywords, locale), sort: (row) => row.organic.keywords, numeric: true },
    { key: "top3", label: t("overview.colTop3"), render: (row) => formatNumber(row.organic.pos_1 + row.organic.pos_2_3, locale), sort: (row) => row.organic.pos_1 + row.organic.pos_2_3, numeric: true },
    { key: "top10", label: t("overview.colTop10"), render: (row) => formatNumber(top10(row), locale), sort: top10, numeric: true },
    { key: "value", label: t("overview.colValue"), help: gloss("traffic_value"), render: (row) => formatUsd(row.organic.traffic_value_usd, locale), sort: (row) => row.organic.traffic_value_usd, numeric: true },
    {
      key: "open",
      label: "",
      render: (row) => (
        <Button small className="ghost" onClick={() => go({ params: { ...params, tab: "keywords", url: row.url } })} title={t("overview.pageKeywordsOpen")}>
          <KeyRound size={13} aria-hidden="true" />
          {t("overview.colKeywords")}
        </Button>
      ),
    },
  ];
  return (
    <ReportFrame report="pages" projectId={projectId} domain={domain} title={t("overview.pagesTitle")} hint={t("overview.pagesHint")} limit={limit} onLimit={setLimit}>
      {(pages) => {
        const needle = fold(query.trim());
        const rows = needle ? pages.filter((row) => fold(row.url).includes(needle)) : pages;
        return (
          <div className="page-stack">
            <div className="filters">
              <label className="filter-search">
                <Search size={15} aria-hidden="true" />
                <span className="sr-only">{t("overview.filterPages")}</span>
                <input className="oc-input" value={query} placeholder={t("overview.filterPages")} onChange={(event) => setQuery(event.target.value)} />
              </label>
              <Button
                small
                disabled={rows.length === 0}
                onClick={() =>
                  download(
                    `${shownDomain}-pages.csv`,
                    toCsv(
                      columns.map((column) => column.label),
                      rows.map((row) => [row.url, Math.round(row.organic.traffic), row.organic.keywords, row.organic.pos_1 + row.organic.pos_2_3, top10(row), Math.round(row.organic.traffic_value_usd)]),
                    ),
                  )
                }
              >
                <Download size={14} />
                {t("overview.export")}
              </Button>
            </div>
            <DataTable rows={rows} columns={columns} rowKey={(row) => row.url} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
          </div>
        );
      }}
    </ReportFrame>
  );
}

/** Pages summed by their first folder: what each section of the site brings. */
export function SubfoldersReport({ projectId, domain, shownDomain }: { projectId: number; domain: string | undefined; shownDomain: string }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const [limit, setLimit] = useState(1000);
  const columns: Column<FolderRow>[] = [
    { key: "folder", label: t("overview.colFolder"), render: (row) => <strong className="cell-main">{row.folder}</strong>, sort: (row) => row.folder },
    { key: "pages", label: t("overview.colPages"), render: (row) => formatNumber(row.pages, locale), sort: (row) => row.pages, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.traffic, locale), sort: (row) => row.traffic, numeric: true, total: (rows) => formatNumber(rows.reduce((sum, row) => sum + row.traffic, 0), locale) },
    { key: "keywords", label: t("overview.colKeywords"), help: gloss("organic_keywords"), render: (row) => formatNumber(row.keywords, locale), sort: (row) => row.keywords, numeric: true },
    { key: "top10", label: t("overview.colTop10"), render: (row) => formatNumber(row.top10, locale), sort: (row) => row.top10, numeric: true },
    { key: "value", label: t("overview.colValue"), help: gloss("traffic_value"), render: (row) => formatUsd(row.value, locale), sort: (row) => row.value, numeric: true },
  ];
  return (
    <ReportFrame report="pages" projectId={projectId} domain={domain} title={t("overview.foldersTitle")} hint={t("overview.foldersHint")} limit={limit} onLimit={setLimit}>
      {(pages) => {
        const rows = subfolders(pages);
        return (
          <div className="page-stack">
            <p className="faint small">{t("overview.foldersFrom", { pages: formatNumber(pages.length, locale), folders: formatNumber(rows.length, locale) })}</p>
            <div className="filters">
              <Button
                small
                disabled={rows.length === 0}
                onClick={() =>
                  download(
                    `${shownDomain}-folders.csv`,
                    toCsv(
                      columns.map((column) => column.label),
                      rows.map((row) => [row.folder, row.pages, Math.round(row.traffic), row.keywords, row.top10, Math.round(row.value)]),
                    ),
                  )
                }
              >
                <Download size={14} />
                {t("overview.export")}
              </Button>
            </div>
            <DataTable rows={rows} columns={columns} rowKey={(row) => row.folder} initialSort={{ key: "traffic", direction: "desc" }} tableId="overview-folders" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
          </div>
        );
      }}
    </ReportFrame>
  );
}

export function SubdomainsReport({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const [limit, setLimit] = useState(100);
  const columns: Column<SubdomainRow>[] = [
    { key: "subdomain", label: t("overview.colSubdomain"), render: (row) => (
        <Entity kind="domain" value={row.subdomain}>
          <strong className="cell-main">{row.subdomain}</strong>
        </Entity>
      ), sort: (row) => row.subdomain },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.organic.traffic, locale), sort: (row) => row.organic.traffic, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), help: gloss("organic_keywords"), render: (row) => formatNumber(row.organic.keywords, locale), sort: (row) => row.organic.keywords, numeric: true },
    { key: "top10", label: t("overview.colTop10"), render: (row) => formatNumber(top10(row), locale), sort: top10, numeric: true },
    { key: "value", label: t("overview.colValue"), help: gloss("traffic_value"), render: (row) => formatUsd(row.organic.traffic_value_usd, locale), sort: (row) => row.organic.traffic_value_usd, numeric: true },
  ];
  return (
    <ReportFrame report="subdomains" projectId={projectId} domain={domain} title={t("overview.subdomainsTitle")} hint={t("overview.subdomainsHint")} limit={limit} onLimit={setLimit}>
      {(rows) => <DataTable rows={rows} columns={columns} rowKey={(row) => row.subdomain} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
    </ReportFrame>
  );
}

function CompareChart({ rows }: { rows: TrafficRow[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 140, right: 24, top: 30, bottom: 24 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis", axisPointer: { type: "shadow" } },
      xAxis: { type: "value", ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      yAxis: { type: "category", inverse: true, data: rows.map((row) => row.target), ...axis, splitLine: { show: false } },
      series: [
        { name: t("overview.trafficSeries"), type: "bar", stack: "traffic", barMaxWidth: 22, data: rows.map((row) => Math.round(row.organic_traffic)) },
        { name: t("overview.colPaidTraffic"), type: "bar", stack: "traffic", barMaxWidth: 22, data: rows.map((row) => Math.round(row.paid_traffic)) },
      ],
    };
  }, [rows, tokens, t, locale]);
  return <EChart option={option} height={Math.max(160, 48 + rows.length * 36)} />;
}

/** Competitors on two axes, keywords and traffic; a bubble's size is the keywords shared. */
function CompetitorMap({ rows, own }: { rows: CompetitorRow[]; own: string }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const shown = rows.slice(0, 20);
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    const most = Math.max(1, ...shown.map((row) => row.shared_keywords));
    return {
      grid: { left: 64, right: 24, top: 24, bottom: 48 },
      tooltip: {
        trigger: "item",
        formatter: (item: unknown) => {
          const data = (item as { data: [number, number, number, string] }).data;
          return `<strong>${data[3]}</strong><br/>${t("overview.colKeywords")}: ${formatNumber(data[0], locale)}<br/>${t("overview.colTraffic")}: ${formatNumber(data[1], locale)}<br/>${t("overview.colShared")}: ${formatNumber(data[2], locale)}`;
        },
      },
      xAxis: { type: "log", name: t("overview.colKeywords"), nameLocation: "middle", nameGap: 28, ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      yAxis: { type: "log", name: t("overview.colTraffic"), ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      series: [
        {
          type: "scatter",
          data: shown.map((row) => [Math.max(1, row.keywords), Math.max(1, Math.round(row.traffic)), row.shared_keywords, row.domain]),
          symbolSize: (value: [number, number, number]) => 10 + 34 * Math.sqrt(value[2] / most),
          label: { show: true, position: "top", formatter: (item: { data: [number, number, number, string] }) => item.data[3], fontSize: 11, color: tokens.ink },
          itemStyle: { opacity: 0.75 },
        },
      ],
    };
  }, [shown, tokens, t, locale]);
  if (shown.length === 0) return null;
  return (
    <section className="sub-panel">
      <h4>{t("overview.mapTitle", { domain: own })}</h4>
      <EChart option={option} height={340} />
    </section>
  );
}

export function CompetitorsReport({ project, domain, onAnalyze }: { project: ProjectRow; domain: string | undefined; onAnalyze: (domain: string) => void }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [limit, setLimit] = useState(100);
  // Domains typed in to compare beside the project's competitors, for this look only.
  const [typed, setTyped] = useState<string[]>([]);
  const [input, setInput] = useState("");
  const addTyped = (event: FormEvent) => {
    event.preventDefault();
    const next = input
      .split(/[\s,;]+/)
      .map((entry) => entry.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/^www\./, "").replace(/\/.*$/, ""))
      .filter((entry) => entry.includes(".") && !typed.includes(entry) && !project.competitors.includes(entry));
    setTyped([...typed, ...next].slice(0, 10));
    setInput("");
  };
  const add = useMutation({
    mutationFn: (competitor: string) =>
      api.updateProject(project.id, {
        name: project.name,
        domain: project.domain,
        country: project.country,
        language: project.language,
        competitors: [...project.competitors, competitor],
        brands: project.brands,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      void queryClient.invalidateQueries({ queryKey: ["overview"] });
      void queryClient.invalidateQueries({ queryKey: ["overview-estimate"] });
      notify(t("overview.added"));
    },
    onError: (thrown) => fail(thrown),
  });
  const columns: Column<CompetitorRow>[] = [
    { key: "domain", label: t("overview.colDomain"), render: (row) => (
        <Entity kind="domain" value={row.domain}>
          <strong className="cell-main">{row.domain}</strong>
        </Entity>
      ), sort: (row) => row.domain },
    { key: "shared", label: t("overview.colShared"), render: (row) => formatNumber(row.shared_keywords, locale), sort: (row) => row.shared_keywords, numeric: true },
    { key: "position", label: t("overview.colAvgPosition"), render: (row) => (row.average_position === null ? "—" : row.average_position.toFixed(1)), sort: (row) => row.average_position, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), help: gloss("organic_keywords"), render: (row) => formatCompact(row.keywords, locale), sort: (row) => row.keywords, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatCompact(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
    {
      key: "actions",
      label: "",
      render: (row) => (
        <span className="row-actions">
          <Button small className="ghost" onClick={() => onAnalyze(row.domain)}>
            {t("overview.analyze")}
          </Button>
          {project.competitors.includes(row.domain) ? (
            <span className="faint small row-done">
              <Check size={13} aria-hidden="true" /> {t("overview.inProject")}
            </span>
          ) : (
            <Button small disabled={project.competitors.length >= 10} busy={add.isPending && add.variables === row.domain} onClick={() => add.mutate(row.domain)}>
              <Plus size={13} />
              {t("overview.addCompetitor")}
            </Button>
          )}
        </span>
      ),
    },
  ];
  return (
    <div className="page-stack">
      <form className="oc-card list-bar" onSubmit={addTyped}>
        <input className="oc-input" value={input} onChange={(event) => setInput(event.target.value)} placeholder={t("overview.compareTypePlaceholder")} aria-label={t("overview.compareType")} spellCheck={false} autoComplete="off" />
        <Button type="submit" small disabled={!input.trim()}>
          <Plus size={14} aria-hidden="true" />
          {t("overview.compareAdd")}
        </Button>
        {typed.map((entry) => (
          <span key={entry} className="oc-chip">
            <span>{entry}</span>
            <button type="button" aria-label={t("overview.compareRemove", { domain: entry })} onClick={() => setTyped(typed.filter((other) => other !== entry))}>
              <X size={12} aria-hidden="true" />
            </button>
          </span>
        ))}
      </form>
      {project.competitors.length > 0 || typed.length > 0 ? (
        <ReportFrame report="compare" projectId={project.id} domain={domain} more={typed.length > 0 ? { targets: typed } : undefined} title={t("overview.compareTitle")} hint={t("overview.compareHint")}>
          {(rows) => <CompareChart rows={rows} />}
        </ReportFrame>
      ) : (
        <div className="oc-banner" role="status">
          {t("overview.compareNeedsCompetitors")}
        </div>
      )}
      <ReportFrame report="competitors" projectId={project.id} domain={domain} title={t("overview.competitorsTitle")} hint={t("overview.competitorsHint")} limit={limit} onLimit={setLimit}>
        {(rows) => (
          <div className="page-stack">
            <CompetitorMap rows={rows} own={domain ?? project.domain} />
            <DataTable rows={rows} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "shared", direction: "desc" }} tableId="overview-competitors" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
          </div>
        )}
      </ReportFrame>
    </div>
  );
}
