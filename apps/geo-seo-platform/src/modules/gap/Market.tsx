// The market around the site: traffic and authority side by side (each one's share), where
// each stands (keywords against traffic), the shares month by month, the categories the site
// ranks in and the domains leading a category.

import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download } from "lucide-react";
import { Button, DataTable, Segmented, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { BulkRow, CategoryName, CategoryPlayer, DomainCategory, ProjectRow, TrafficHistory } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { Entity } from "../../lib/Entity";
import { formatCompact, formatNumber } from "../../lib/format";
import { useGlossary } from "../../lib/glossary";
import { PaidFrame } from "../../lib/PaidFrame";
import { download, toCsv } from "../../lib/report";
import { tableLabels } from "../../lib/tables";
import { Authority } from "../backlinks/Backlinks";
import { shareSeries, trafficShares } from "./groups";

const RANGES = { year: 12, two: 24, all: 120 } as const;
type Range = keyof typeof RANGES;

function BulkView({ rows }: { rows: BulkRow[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const tokens = useChartTokens();
  const shares = useMemo(() => trafficShares(rows), [rows]);
  const option = useMemo<ChartOption>(
    () => ({
      tooltip: { trigger: "item", valueFormatter: (value: unknown) => `${Number(value).toFixed(1)}%` },
      legend: { bottom: 0, left: 0, type: "scroll" },
      series: [
        {
          type: "pie",
          radius: ["45%", "70%"],
          top: -20,
          itemStyle: { borderColor: tokens.raised, borderWidth: 2 },
          label: { color: tokens.text },
          data: rows.map((row) => ({ name: row.target, value: Number((shares.get(row.target) ?? 0).toFixed(1)) })),
        },
      ],
    }),
    [rows, shares, tokens],
  );
  const columns: Column<BulkRow>[] = [
    { key: "target", label: t("overview.colDomain"), render: (row) => (
        <Entity kind="domain" value={row.target}>
          <strong className="cell-main">{row.target}</strong>
        </Entity>
      ), sort: (row) => row.target },
    { key: "traffic", label: t("overview.organicTraffic"), help: gloss("traffic"), render: (row) => formatCompact(row.organic_traffic, locale), sort: (row) => row.organic_traffic, numeric: true },
    { key: "share", label: t("gap.colShare"), render: (row) => `${(shares.get(row.target) ?? 0).toFixed(1)}%`, sort: (row) => shares.get(row.target) ?? 0, numeric: true },
    { key: "keywords", label: t("overview.organicKeywords"), help: gloss("organic_keywords"), render: (row) => formatCompact(row.organic_keywords, locale), sort: (row) => row.organic_keywords, numeric: true },
    { key: "paid", label: t("overview.colPaidTraffic"), help: gloss("paid_traffic"), render: (row) => formatCompact(row.paid_traffic, locale), sort: (row) => row.paid_traffic, numeric: true },
    { key: "rank", label: "OA", help: gloss("oa"), render: (row) => <Authority rank={row.rank} />, sort: (row) => row.rank, numeric: true },
  ];
  return (
    <div className="page-stack">
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("gap.shareTitle")}</h4>
          <EChart option={option} height={260} />
        </section>
        <section className="sub-panel">
          <h4>{t("gap.shareHint")}</h4>
          <p className="faint small">{t("gap.shareText")}</p>
        </section>
      </div>
      <div className="table-toolbar">
        <Button
          small
          disabled={rows.length === 0}
          onClick={() =>
            download(
              "bulk-compare.csv",
              toCsv(
                [t("overview.colDomain"), t("overview.organicTraffic"), t("gap.colShare"), t("overview.organicKeywords"), t("overview.colPaidTraffic"), "OA"],
                rows.map((row) => [row.target, row.organic_traffic, (shares.get(row.target) ?? 0).toFixed(1), row.organic_keywords, row.paid_traffic, row.rank]),
              ),
            )
          }
        >
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable rows={rows} columns={columns} rowKey={(row) => row.target} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}

/** Where each site stands: keywords against traffic, a bubble's size its authority. */
const decadeBelow = (value: number) => 10 ** Math.floor(Math.log10(Math.max(1, value)));
const decadeAbove = (value: number) => 10 ** Math.ceil(Math.log10(Math.max(10, value)));

function PositionMap({ rows }: { rows: BulkRow[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 64, right: 24, top: 24, bottom: 48 },
      tooltip: {
        trigger: "item",
        formatter: (item: unknown) => {
          const data = (item as { data: [number, number, number, string] }).data;
          return `<strong>${data[3]}</strong><br/>${t("overview.organicKeywords")}: ${formatNumber(data[0], locale)}<br/>${t("overview.organicTraffic")}: ${formatNumber(data[1], locale)}<br/>OA: ${data[2]}`;
        },
      },
      // Whole powers of ten with room around the bubbles, so the largest and their names stay inside.
      xAxis: { type: "log", name: t("overview.organicKeywords"), nameLocation: "middle", nameGap: 28, min: (extent: { min: number }) => decadeBelow(extent.min), max: (extent: { max: number }) => decadeAbove(extent.max * 2), ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      yAxis: { type: "log", name: t("overview.organicTraffic"), min: (extent: { min: number }) => decadeBelow(extent.min), max: (extent: { max: number }) => decadeAbove(extent.max * 3), ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      series: [
        {
          type: "scatter",
          data: rows.map((row) => [Math.max(1, row.organic_keywords), Math.max(1, Math.round(row.organic_traffic)), row.rank ?? 0, row.target]),
          symbolSize: (value: [number, number, number]) => 12 + (value[2] / 100) * 36,
          label: { show: true, position: "top", formatter: (item: { data: [number, number, number, string] }) => item.data[3], fontSize: 11, color: tokens.ink },
          itemStyle: { opacity: 0.75 },
        },
      ],
    };
  }, [rows, tokens, t, locale]);
  if (rows.length === 0) return null;
  return (
    <section className="sub-panel">
      <h4>{t("gap.mapTitle")}</h4>
      <EChart option={option} height={320} />
    </section>
  );
}

function ShareChart({ histories }: { histories: TrafficHistory[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const series = useMemo(() => shareSeries(histories), [histories]);
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 48, right: 16, top: 44, bottom: 28 },
      legend: { top: 0, left: 0, type: "scroll" },
      tooltip: { trigger: "axis", valueFormatter: (value: unknown) => `%${Number(value).toFixed(1)}` },
      xAxis: {
        type: "category",
        data: series.months.map((key) => {
          const [year, month] = key.split("-").map(Number);
          return new Date(Date.UTC(year ?? 2020, (month ?? 1) - 1, 1)).toLocaleDateString(locale, { month: "short", year: "2-digit" });
        }),
        ...axis,
        splitLine: { show: false },
      },
      yAxis: { type: "value", max: 100, ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => `%${value}` } },
      series: series.series.map((entry) => ({ name: entry.target, type: "line", stack: "share", areaStyle: { opacity: 0.35 }, symbol: "none", smooth: true, data: entry.shares })),
    };
  }, [series, tokens, locale]);
  if (series.months.length === 0) return <p className="faint">{t("gap.noShareHistory")}</p>;
  return <EChart option={option} height={300} />;
}

/** Each site's share of their organic traffic, month by month. */
function ShareHistory({ projectId, targets, market }: { projectId: number; targets: string[]; market: Record<string, unknown> }) {
  const { t } = useI18n();
  const [range, setRange] = useState<Range>("year");
  return (
    <PaidFrame<TrafficHistory[]>
      module="gap"
      ask={{ project_id: projectId, report: "share", targets, ...(range === "year" ? {} : { months: RANGES[range] }), ...market }}
      title={t("gap.shareHistoryTitle")}
      hint={t("gap.shareHistoryHint")}
      extra={
        <Segmented<Range>
          label={t("overview.historyRange")}
          value={range}
          onChange={setRange}
          options={[
            { value: "year", label: t("gap.range12") },
            { value: "two", label: t("overview.range24") },
            { value: "all", label: t("overview.rangeAll") },
          ]}
        />
      }
    >
      {(histories) => <ShareChart histories={histories} />}
    </PaidFrame>
  );
}

/** Category names, fetched once (free on DataForSEO); codes stand in when they cannot be. */
function useCategoryNames(projectId: number): (code: number) => string {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const names = useQuery({ queryKey: ["category-names", projectId], queryFn: () => api.categoryNames(projectId) });
  const fetch = useMutation({
    mutationFn: () => api.fetchCategoryNames(projectId),
    onSuccess: (list) => queryClient.setQueryData(["category-names", projectId], list),
  });
  const { mutate } = fetch;
  const missing = names.isSuccess && names.data === null && fetch.isIdle;
  useEffect(() => {
    if (missing) mutate();
  }, [missing, mutate]);
  const byCode = useMemo(() => new Map((names.data ?? []).map((entry: CategoryName) => [entry.code, entry.name])), [names.data]);
  return (code) => byCode.get(code) ?? t("gap.categoryCode", { code });
}

function PlayersView({ rows, own }: { rows: CategoryPlayer[]; own: string }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const columns: Column<CategoryPlayer>[] = [
    { key: "domain", label: t("overview.colDomain"), render: (row) => (
        <Entity kind="domain" value={row.domain}>
          <strong className={`cell-main${row.domain === own ? " own-row" : ""}`}>{row.domain}</strong>
        </Entity>
      ), sort: (row) => row.domain },
    { key: "traffic", label: t("overview.organicTraffic"), help: gloss("traffic"), render: (row) => formatCompact(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
    { key: "change", label: t("gap.colChange"), render: (row) => (row.traffic_change === null ? "—" : <span className={row.traffic_change >= 0 ? "good" : "warn"}>{row.traffic_change >= 0 ? "+" : ""}{formatCompact(row.traffic_change, locale)}</span>), sort: (row) => row.traffic_change, numeric: true },
    { key: "keywords", label: t("overview.organicKeywords"), help: gloss("organic_keywords"), render: (row) => formatCompact(row.keywords, locale), sort: (row) => row.keywords, numeric: true },
  ];
  return <DataTable rows={rows} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />;
}

/** The categories the site ranks in, and the domains leading the chosen one. */
function Categories({ project, market }: { project: ProjectRow; market: Record<string, unknown> }) {
  const { t, locale } = useI18n();
  const nameOf = useCategoryNames(project.id);
  const [code, setCode] = useState<number | null>(null);
  const columns: Column<DomainCategory>[] = [
    {
      key: "name",
      label: t("gap.colCategory"),
      render: (row) => (
        <label className="oc-check">
          <input type="radio" name="market-category" checked={code === row.code} onChange={() => setCode(row.code)} />
          <span>
            <strong>{nameOf(row.code)}</strong>
            {row.path.length > 1 && <span className="faint small"> · {row.path.slice(0, -1).map(nameOf).join(" › ")}</span>}
          </span>
        </label>
      ),
      sort: (row) => nameOf(row.code),
    },
    { key: "traffic", label: t("overview.organicTraffic"), render: (row) => formatCompact(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
    { key: "keywords", label: t("overview.organicKeywords"), render: (row) => formatNumber(row.keywords, locale), sort: (row) => row.keywords, numeric: true },
  ];
  return (
    <div className="page-stack">
      <PaidFrame<DomainCategory[]> module="gap" ask={{ project_id: project.id, report: "categories", ...market }} title={t("gap.categoriesTitle")} hint={t("gap.categoriesHint")}>
        {(rows) => <DataTable rows={rows} columns={columns} rowKey={(row) => row.path.join(">")} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
      </PaidFrame>
      {code !== null && (
        <PaidFrame<CategoryPlayer[]> module="gap" ask={{ project_id: project.id, report: "players", categories: [code], ...market }} title={t("gap.playersTitle", { category: nameOf(code) })} hint={t("gap.playersHint")}>
          {(rows) => <PlayersView rows={rows} own={project.domain} />}
        </PaidFrame>
      )}
    </div>
  );
}

export function MarketTab({ project, competitors, market }: { project: ProjectRow; competitors: string[]; market: Record<string, unknown> }) {
  const { t } = useI18n();
  const [input, setInput] = useState("");
  const [targets, setTargets] = useState<string[]>([]);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setTargets(
      input
        .split(/[\n,;\s]+/)
        .map((line) => line.trim())
        .filter(Boolean),
    );
  };
  return (
    <div className="page-stack">
      <form className="oc-card list-bar" onSubmit={submit}>
        <textarea className="oc-input bulk-input" rows={3} value={input} onChange={(event) => setInput(event.target.value)} placeholder={t("gap.bulkPlaceholder")} spellCheck={false} />
        <Button type="submit">{t("backlinks.bulkCompare")}</Button>
        <span className="faint small">{t("gap.bulkHint")}</span>
      </form>
      <PaidFrame<BulkRow[]> module="gap" ask={{ project_id: project.id, report: "bulk", targets, ...market }} title={t("gap.bulkTitle")} hint={targets.length ? t("backlinks.bulkCount", { n: targets.length }) : t("backlinks.bulkDefault")}>
        {(rows) => (
          <div className="page-stack">
            <BulkView rows={rows} />
            <PositionMap rows={rows} />
          </div>
        )}
      </PaidFrame>
      <ShareHistory projectId={project.id} targets={targets.length > 0 ? targets.filter((target) => target !== project.domain) : competitors} market={market} />
      <Categories project={project} market={market} />
    </div>
  );
}
