import { useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Check, Download, Plus, Search } from "lucide-react";
import { Button, DataTable, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { CompetitorRow, PageRow, ProjectRow, SubdomainRow, TrafficRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { explain } from "../../lib/errors";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { download, fold, toCsv } from "../../lib/report";
import { shortUrl, tableLabels } from "../../lib/tables";
import { ReportFrame } from "./ReportFrame";

function top10(row: { organic: { pos_1: number; pos_2_3: number; pos_4_10: number } }): number {
  return row.organic.pos_1 + row.organic.pos_2_3 + row.organic.pos_4_10;
}

export function PagesReport({ projectId, domain, shownDomain }: { projectId: number; domain: string | undefined; shownDomain: string }) {
  const { t, locale } = useI18n();
  const [limit, setLimit] = useState(100);
  const [query, setQuery] = useState("");
  const columns: Column<PageRow>[] = [
    { key: "url", label: t("overview.colPage"), render: (row) => <span className="cell-url" title={row.url}>{shortUrl(row.url)}</span>, sort: (row) => row.url },
    { key: "traffic", label: t("overview.colTraffic"), render: (row) => formatNumber(row.organic.traffic, locale), sort: (row) => row.organic.traffic, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), render: (row) => formatNumber(row.organic.keywords, locale), sort: (row) => row.organic.keywords, numeric: true },
    { key: "top3", label: t("overview.colTop3"), render: (row) => formatNumber(row.organic.pos_1 + row.organic.pos_2_3, locale), sort: (row) => row.organic.pos_1 + row.organic.pos_2_3, numeric: true },
    { key: "top10", label: t("overview.colTop10"), render: (row) => formatNumber(top10(row), locale), sort: top10, numeric: true },
    { key: "value", label: t("overview.colValue"), render: (row) => formatUsd(row.organic.traffic_value_usd, locale), sort: (row) => row.organic.traffic_value_usd, numeric: true },
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

export function SubdomainsReport({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t, locale } = useI18n();
  const [limit, setLimit] = useState(100);
  const columns: Column<SubdomainRow>[] = [
    { key: "subdomain", label: t("overview.colSubdomain"), render: (row) => <strong className="cell-main">{row.subdomain}</strong>, sort: (row) => row.subdomain },
    { key: "traffic", label: t("overview.colTraffic"), render: (row) => formatNumber(row.organic.traffic, locale), sort: (row) => row.organic.traffic, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), render: (row) => formatNumber(row.organic.keywords, locale), sort: (row) => row.organic.keywords, numeric: true },
    { key: "top10", label: t("overview.colTop10"), render: (row) => formatNumber(top10(row), locale), sort: top10, numeric: true },
    { key: "value", label: t("overview.colValue"), render: (row) => formatUsd(row.organic.traffic_value_usd, locale), sort: (row) => row.organic.traffic_value_usd, numeric: true },
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

export function CompetitorsReport({ project, domain, onAnalyze }: { project: ProjectRow; domain: string | undefined; onAnalyze: (domain: string) => void }) {
  const { t, locale } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const [limit, setLimit] = useState(100);
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
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const columns: Column<CompetitorRow>[] = [
    { key: "domain", label: t("overview.colDomain"), render: (row) => <strong className="cell-main">{row.domain}</strong>, sort: (row) => row.domain },
    { key: "shared", label: t("overview.colShared"), render: (row) => formatNumber(row.shared_keywords, locale), sort: (row) => row.shared_keywords, numeric: true },
    { key: "position", label: t("overview.colAvgPosition"), render: (row) => (row.average_position === null ? "—" : row.average_position.toFixed(1)), sort: (row) => row.average_position, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), render: (row) => formatCompact(row.keywords, locale), sort: (row) => row.keywords, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), render: (row) => formatCompact(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
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
      {project.competitors.length > 0 ? (
        <ReportFrame report="compare" projectId={project.id} domain={domain} title={t("overview.compareTitle")} hint={t("overview.compareHint")}>
          {(rows) => <CompareChart rows={rows} />}
        </ReportFrame>
      ) : (
        <div className="oc-banner" role="status">
          {t("overview.compareNeedsCompetitors")}
        </div>
      )}
      <ReportFrame report="competitors" projectId={project.id} domain={domain} title={t("overview.competitorsTitle")} hint={t("overview.competitorsHint")} limit={limit} onLimit={setLimit}>
        {(rows) => <DataTable rows={rows} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "shared", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
      </ReportFrame>
    </div>
  );
}
