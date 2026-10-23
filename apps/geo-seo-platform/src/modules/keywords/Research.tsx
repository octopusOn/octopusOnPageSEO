// A keyword in depth: its searches since 2021, its parent topic and traffic potential, its
// searches in other countries, the weight of each result on its page and the page's past.

import { useMemo, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { ListOrdered } from "lucide-react";
import { Button, DataTable, Stat, type Column } from "@octopusseo/ui";
import { paid } from "../../api/client";
import type { Fetched, MarketVolume, RankedKeyword, ResearchData, ResearchReport, ResultWeight, SerpPage } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { Entity } from "../../lib/Entity";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { useGlossary } from "../../lib/glossary";
import { PaidFrame } from "../../lib/PaidFrame";
import { tableLabels } from "../../lib/tables";
import { Authority } from "../backlinks/Backlinks";
import { Difficulty } from "./KeywordTable";
import { serpTracks } from "./ideas";

/** The Keyword Explorer's result-page question, so its kept answer is shared. */
export function serpAsk(projectId: number, seed: string, mobile: boolean, market: string | undefined): Record<string, unknown> {
  return { project_id: projectId, report: "serp", seed, mobile, ...(market ? { market } : {}) };
}

/** Whether the result page of a keyword is kept: what the page weights and the parent topic read. */
function useKeptSerp(ask: Record<string, unknown>) {
  return useQuery({ queryKey: ["keywords", JSON.stringify(ask)], queryFn: () => paid.kept<SerpPage>("keywords", ask) });
}

function ResearchFrame<R extends ResearchReport>({
  report,
  projectId,
  seed,
  market,
  more,
  title,
  hint,
  children,
}: {
  report: R;
  projectId: number;
  seed: string;
  market: string | undefined;
  more?: Record<string, unknown>;
  title: string;
  hint?: string;
  children: (data: ResearchData[R], fetched: Fetched<ResearchData[R]>) => ReactNode;
}) {
  return (
    <PaidFrame<ResearchData[R]> module="research" ask={{ project_id: projectId, report, seed, ...(market ? { market } : {}), ...more }} title={title} hint={hint}>
      {children}
    </PaidFrame>
  );
}

function HistoryChart({ months }: { months: ResearchData["history"] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 56, right: 16, top: 16, bottom: 28 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: months.map((month) => new Date(Date.UTC(month.year, month.month - 1, 1)).toLocaleDateString(locale, { month: "short", year: "2-digit" })), ...axis, splitLine: { show: false } },
      yAxis: { type: "value", ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      series: [{ name: t("overview.colVolume"), type: "line", smooth: true, symbol: "none", areaStyle: { opacity: 0.14 }, data: months.map((month) => month.volume) }],
    };
  }, [months, tokens, t, locale]);
  if (months.length === 0) return <p className="faint">{t("keywords.noHistory")}</p>;
  return <EChart option={option} height={260} />;
}

/** Searches month by month since 2021: seasons and the long trend. */
export function VolumeHistory({ projectId, seed, market }: { projectId: number; seed: string; market: string | undefined }) {
  const { t } = useI18n();
  return (
    <ResearchFrame report="history" projectId={projectId} seed={seed} market={market} title={t("keywords.historyTitle")} hint={t("keywords.historyHint")}>
      {(months) => <HistoryChart months={months} />}
    </ResearchFrame>
  );
}

function ParentView({ data }: { data: ResearchData["parent"] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const columns: Column<RankedKeyword>[] = [
    { key: "keyword", label: t("overview.colKeyword"), render: (row) => (
        <Entity kind="keyword" value={row.keyword}>
          <strong className="cell-main">{row.keyword}</strong>
        </Entity>
      ), sort: (row) => row.keyword },
    { key: "position", label: t("overview.colPosition"), help: gloss("position"), render: (row) => row.position ?? "—", sort: (row) => row.position, numeric: true },
    { key: "volume", label: t("overview.colVolume"), help: gloss("volume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
  ];
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat
          label={t("keywords.parentTopic")}
          help={t("keywords.parentTopicHelp")}
          value={
            data.parent ? (
              <Entity kind="keyword" value={data.parent.keyword}>
                <span className="stat-text">{data.parent.keyword}</span>
              </Entity>
            ) : (
              "—"
            )
          }
          hint={data.parent?.volume ? t("keywords.parentVolume", { n: formatNumber(data.parent.volume, locale) }) : undefined}
        />
        <Stat label={t("keywords.trafficPotential")} help={t("keywords.trafficPotentialHelp")} value={formatCompact(data.reach.traffic, locale)} hint={t("keywords.pageRanksFor", { n: formatNumber(data.reach.keywords, locale) })} />
        <Stat
          label={t("keywords.firstResult")}
          value={
            <Entity kind="url" value={data.url}>
              <span className="stat-text">{data.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
            </Entity>
          }
        />
      </div>
      <h4>{t("keywords.alsoRanksFor")}</h4>
      <DataTable rows={data.keywords.slice(0, 100)} columns={columns} rowKey={(row) => `${row.keyword}|${row.url ?? ""}`} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}

/** Asks for the result page first when it is not kept. */
function NeedsSerp({ onSerp }: { onSerp: () => void }) {
  const { t } = useI18n();
  return (
    <div className="oc-banner info" role="status">
      <span>{t("keywords.needsSerp")}</span>
      <Button small onClick={onSerp}>
        <ListOrdered size={14} aria-hidden="true" />
        {t("keywords.openSerpTab")}
      </Button>
    </div>
  );
}

/** What the first result ranks for: the parent topic and the traffic potential (Ahrefs' way). */
export function ParentTopicReport({ projectId, seed, market }: { projectId: number; seed: string; market: string | undefined }) {
  const { t } = useI18n();
  const { params, go } = useShell();
  const kept = useKeptSerp(serpAsk(projectId, seed, false, market));
  if (!kept.data) {
    return (
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("keywords.parentTitle")}</h3>
            <span className="faint">{t("keywords.parentHint")}</span>
          </div>
        </header>
        {kept.isSuccess && <NeedsSerp onSerp={() => go({ params: { ...params, tab: "serp" } })} />}
      </section>
    );
  }
  return (
    <ResearchFrame report="parent" projectId={projectId} seed={seed} market={market} title={t("keywords.parentTitle")} hint={t("keywords.parentHint")}>
      {(data) => <ParentView data={data} />}
    </ResearchFrame>
  );
}

function CountriesView({ rows }: { rows: MarketVolume[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const regions = useMemo(() => new Intl.DisplayNames([locale], { type: "region" }), [locale]);
  const languages = useMemo(() => new Intl.DisplayNames([locale], { type: "language" }), [locale]);
  const name = (market: string) => {
    const [country = "", language = ""] = market.split("-");
    try {
      return `${regions.of(country.toUpperCase()) ?? country} · ${languages.of(language) ?? language}`;
    } catch {
      return market;
    }
  };
  const columns: Column<MarketVolume>[] = [
    { key: "market", label: t("query.market"), render: (row) => <strong className="cell-main">{name(row.market)}</strong>, sort: (row) => name(row.market) },
    { key: "volume", label: t("overview.colVolume"), help: gloss("volume"), render: (row) => (row.data?.volume == null ? "—" : formatNumber(row.data.volume, locale)), sort: (row) => row.data?.volume ?? null, numeric: true },
    { key: "difficulty", label: t("overview.colKd"), help: gloss("kd"), render: (row) => <Difficulty value={row.data?.difficulty ?? null} />, sort: (row) => row.data?.difficulty ?? null, numeric: true },
    { key: "cpc", label: t("overview.colCpc"), help: gloss("cpc"), render: (row) => (row.data?.cpc == null ? "—" : formatUsd(row.data.cpc, locale)), sort: (row) => row.data?.cpc ?? null, numeric: true },
  ];
  return <DataTable rows={rows} columns={columns} rowKey={(row) => row.market} initialSort={{ key: "volume", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />;
}

/** The keyword's searches in other big markets. */
export function CountryVolumes({ projectId, seed }: { projectId: number; seed: string }) {
  const { t } = useI18n();
  return (
    <ResearchFrame report="countries" projectId={projectId} seed={seed} market={undefined} title={t("keywords.countriesTitle")} hint={t("keywords.countriesHint")}>
      {(rows) => <CountriesView rows={rows} />}
    </ResearchFrame>
  );
}

function WeightsView({ rows }: { rows: ResultWeight[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const columns: Column<ResultWeight>[] = [
    { key: "rank", label: "#", render: (row) => row.rank, sort: (row) => row.rank, numeric: true, width: "48px" },
    { key: "url", label: t("overview.colPage"), render: (row) => (
        <Entity kind="url" value={row.url}>
          <span className="cell-url">{row.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
        </Entity>
      ), sort: (row) => row.url },
    { key: "page_rank", label: t("backlinks.colPageRank"), help: gloss("oa"), render: (row) => <Authority rank={row.page_rank} />, sort: (row) => row.page_rank, numeric: true },
    { key: "referring", label: t("backlinks.referringDomains"), help: gloss("referring_domains"), render: (row) => (row.referring_domains === null ? "—" : formatNumber(row.referring_domains, locale)), sort: (row) => row.referring_domains, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => (row.traffic === null ? "—" : formatNumber(row.traffic, locale)), sort: (row) => row.traffic, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), help: gloss("organic_keywords"), render: (row) => (row.keywords === null ? "—" : formatNumber(row.keywords, locale)), sort: (row) => row.keywords, numeric: true },
  ];
  return <DataTable rows={rows} columns={columns} rowKey={(row) => `${row.rank}|${row.url}`} initialSort={{ key: "rank", direction: "asc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />;
}

/** How strong each result of the kept result page is: what it takes to rank there. */
export function SerpWeights({ projectId, seed, mobile, market }: { projectId: number; seed: string; mobile: boolean; market: string | undefined }) {
  const { t } = useI18n();
  const kept = useKeptSerp(serpAsk(projectId, seed, mobile, market));
  if (!kept.data) return null;
  return (
    <ResearchFrame report="serp_metrics" projectId={projectId} seed={seed} market={market} more={mobile ? { mobile } : undefined} title={t("keywords.weightsTitle")} hint={t("keywords.weightsHint")}>
      {(rows) => <WeightsView rows={rows} />}
    </ResearchFrame>
  );
}

function HistoryGrid({ pages }: { pages: ResearchData["serp_history"] }) {
  const { t, locale } = useI18n();
  const grid = useMemo(() => serpTracks(pages), [pages]);
  if (grid.dates.length === 0) return <p className="faint">{t("keywords.noSerpHistory")}</p>;
  const tone = (rank: number | null) => (rank === null ? "none" : rank <= 3 ? "top3" : rank <= 10 ? "top10" : "rest");
  return (
    <div className="oc-table-wrap">
      <table className="oc-table serp-grid">
        <thead>
          <tr>
            <th>{t("overview.colPage")}</th>
            {grid.dates.map((date) => (
              <th key={date} className="number">
                {new Date(`${date}T00:00:00Z`).toLocaleDateString(locale, { month: "short", year: "2-digit" })}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {grid.tracks.map((track) => (
            <tr key={track.url}>
              <td>
                <Entity kind="url" value={track.url}>
                  <span className="cell-url">{track.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
                </Entity>
              </td>
              {track.ranks.map((rank, index) => (
                <td key={grid.dates[index]} className={`number rank-${tone(rank)}`}>
                  {rank ?? "—"}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** The result page month by month for a year: who held which place. */
export function SerpHistory({ projectId, seed, market }: { projectId: number; seed: string; market: string | undefined }) {
  const { t } = useI18n();
  return (
    <ResearchFrame report="serp_history" projectId={projectId} seed={seed} market={market} title={t("keywords.serpHistoryTitle")} hint={t("keywords.serpHistoryHint")}>
      {(pages) => <HistoryGrid pages={pages} />}
    </ResearchFrame>
  );
}
