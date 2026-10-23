import { useMemo, useState } from "react";
import { ArrowDownRight, ArrowUpRight, Globe } from "lucide-react";
import { Button, DataTable, Empty, Segmented, Stat, type Column } from "@octopusseo/ui";
import type { BacklinkSummary, CountryRow, DomainOverview, HistoryPoint, ProjectRow, RankedKeywords, RankMetrics } from "../../api/types";
import { useI18n } from "../../i18n";
import { useGlossary } from "../../lib/glossary";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { featureName, tableLabels } from "../../lib/tables";
import { Authority, followShare } from "../backlinks/Backlinks";
import { LinkHistory } from "../backlinks/LinkHistory";
import { brandSplit, brandTerms, featureShares, intentShares, type Share } from "./organic";
import { ReportFrame } from "./ReportFrame";

/** Months of history to ask for: the default two years, or everything since October 2020. */
const RANGES = { recent: 24, all: 120 } as const;
type Range = keyof typeof RANGES;

/** Keywords read for the mix: the first thousand by traffic. */
const MIX_ROWS = 1000;

function Positions({ metrics }: { metrics: RankMetrics }) {
  const { t, locale } = useI18n();
  const rows: [string, number][] = [
    ["overview.pos1", metrics.pos_1],
    ["overview.pos23", metrics.pos_2_3],
    ["overview.pos410", metrics.pos_4_10],
    ["overview.pos1120", metrics.pos_11_20],
    ["overview.pos21100", metrics.pos_21_100],
  ];
  const most = Math.max(1, ...rows.map(([, value]) => value));
  return (
    <section className="sub-panel">
      <h4>{t("overview.positions")}</h4>
      <ol className="bars" aria-label={t("overview.positions")}>
        {rows.map(([label, value]) => (
          <li key={label}>
            <span className="bar-label">{t(label)}</span>
            <span className="bar-track" aria-hidden="true">
              <span className="bar-fill" style={{ width: `${(value / most) * 100}%` }} />
            </span>
            <span className="bar-value">{formatNumber(value, locale)}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}

function Movement({ metrics }: { metrics: RankMetrics }) {
  const { t, locale } = useI18n();
  const items: [string, number, "good" | "bad"][] = [
    ["overview.new", metrics.new, "good"],
    ["overview.up", metrics.up, "good"],
    ["overview.down", metrics.down, "bad"],
    ["overview.lost", metrics.lost, "bad"],
  ];
  return (
    <section className="sub-panel">
      <h4>{t("overview.movement")}</h4>
      <div className="movement">
        {items.map(([label, value, tone]) => (
          <div key={label} className={`movement-item ${tone}`}>
            {tone === "good" ? <ArrowUpRight size={16} aria-hidden="true" /> : <ArrowDownRight size={16} aria-hidden="true" />}
            <span>{t(label)}</span>
            <strong>{formatNumber(value, locale)}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}

function SummaryView({ data }: { data: DomainOverview }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { organic, paid } = data;
  if (organic.keywords === 0 && paid.keywords === 0) {
    return (
      <Empty icon={<Globe size={28} />} title={t("overview.noDataTitle")}>
        <p>{t("overview.noDataText")}</p>
      </Empty>
    );
  }
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat label={t("overview.organicKeywords")} help={gloss("organic_keywords")} value={formatCompact(organic.keywords, locale)} hint={formatNumber(organic.keywords, locale)} />
        <Stat label={t("overview.organicTraffic")} help={gloss("traffic")} value={formatCompact(organic.traffic, locale)} hint={`${formatNumber(organic.traffic, locale)} ${t("overview.perMonth")}`} />
        <Stat label={t("overview.trafficValue")} help={gloss("traffic_value")} value={formatUsd(organic.traffic_value_usd, locale)} hint={t("overview.perMonth")} />
        <Stat label={t("overview.paidKeywords")} help={gloss("paid_traffic")} value={formatCompact(paid.keywords, locale)} hint={`${t("overview.paidTraffic")}: ${formatCompact(paid.traffic, locale)}`} />
      </div>
      <div className="two-columns">
        <Positions metrics={organic} />
        <Movement metrics={organic} />
      </div>
    </div>
  );
}

/** Every series of the history; the first two are shown until others are picked in the legend. */
function HistoryChart({ points }: { points: HistoryPoint[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const months = points.map((point) => new Date(Date.UTC(point.year, point.month - 1, 1)).toLocaleDateString(locale, { month: "short", year: "2-digit" }));
    const axis = axisStyle(tokens);
    const line = (name: string, data: number[], axisIndex = 0, area = false) => ({ name, type: "line" as const, smooth: true, symbol: "none", yAxisIndex: axisIndex, ...(area ? { areaStyle: { opacity: 0.12 } } : {}), data });
    const traffic = t("overview.trafficSeries");
    const keywords = t("overview.keywordsSeries");
    const value = t("overview.valueSeries");
    const paid = t("overview.colPaidTraffic");
    const top3 = t("overview.top3Series");
    const top10 = t("overview.top10Series");
    return {
      grid: { left: 56, right: 56, top: 44, bottom: 28 },
      legend: { top: 0, left: 0, selected: { [value]: false, [paid]: false, [top3]: false, [top10]: false } },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: months, ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", ...axis, axisLabel: { ...axis.axisLabel, formatter: (amount: number) => formatCompact(amount, locale) } },
        { type: "value", ...axis, splitLine: { show: false }, axisLabel: { ...axis.axisLabel, formatter: (amount: number) => formatCompact(amount, locale) } },
      ],
      series: [
        line(traffic, points.map((point) => Math.round(point.organic.traffic)), 0, true),
        line(keywords, points.map((point) => point.organic.keywords), 1),
        line(value, points.map((point) => Math.round(point.organic.traffic_value_usd))),
        line(paid, points.map((point) => Math.round(point.paid.traffic))),
        line(top3, points.map((point) => point.organic.pos_1 + point.organic.pos_2_3), 1),
        line(top10, points.map((point) => point.organic.pos_1 + point.organic.pos_2_3 + point.organic.pos_4_10), 1),
      ],
    };
  }, [points, tokens, t, locale]);
  if (points.length === 0) return <p className="faint">{t("overview.noDataText")}</p>;
  return <EChart option={option} height={320} />;
}

function HistoryReport({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t } = useI18n();
  const [range, setRange] = useState<Range>("recent");
  return (
    <ReportFrame
      report="history"
      projectId={projectId}
      domain={domain}
      more={range === "all" ? { months: RANGES.all } : undefined}
      title={t("overview.history")}
      hint={range === "all" ? t("overview.historyHintAll") : t("overview.historyHint")}
      extra={
        <Segmented<Range>
          label={t("overview.historyRange")}
          value={range}
          onChange={setRange}
          options={[
            { value: "recent", label: t("overview.range24") },
            { value: "all", label: t("overview.rangeAll") },
          ]}
        />
      }
    >
      {(points) => <HistoryChart points={points} />}
    </ReportFrame>
  );
}

/** The domain's link profile in short; the Backlinks page has the rest. */
function LinkProfile({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  return (
    <PaidFrame<BacklinkSummary> module="backlinks" ask={{ project_id: projectId, report: "summary", ...(domain ? { target: domain } : {}) }} title={t("backlinks.profileTitle")} hint={t("overview.linkProfileHint")}>
      {(summary) => (
        <div className="oc-stats">
          <Stat label={t("backlinks.authority")} help={gloss("oa")} value={<Authority rank={summary.rank} />} hint={t("backlinks.authorityHint")} />
          <Stat label={t("backlinks.referringDomains")} help={gloss("referring_domains")} value={formatCompact(summary.referring_domains, locale)} hint={t("backlinks.followShare", { share: followShare(summary) ?? "—" })} />
          <Stat label={t("backlinks.backlinks")} help={gloss("backlinks")} value={formatCompact(summary.backlinks, locale)} hint={formatNumber(summary.backlinks, locale)} />
          <Stat label={t("overview.spamScore")} help={gloss("spam_score")} value={summary.spam_score ?? "—"} hint={t("overview.spamScoreHint")} tone={(summary.spam_score ?? 0) >= 50 ? "bad" : undefined} />
        </div>
      )}
    </PaidFrame>
  );
}

function ShareList({ title, shares, label, total }: { title: string; shares: Share[]; label: (key: string) => string; total: number }) {
  const { locale } = useI18n();
  const most = Math.max(1, ...shares.map((share) => share.keywords));
  return (
    <section className="sub-panel">
      <h4>{title}</h4>
      <ol className="bars" aria-label={title}>
        {shares.map((share) => (
          <li key={share.key}>
            <span className="bar-label">{label(share.key)}</span>
            <span className="bar-track" aria-hidden="true">
              <span className="bar-fill" style={{ width: `${(share.keywords / most) * 100}%` }} />
            </span>
            <span className="bar-value">
              {formatNumber(share.keywords, locale)}
              {total > 0 && <small className="faint"> · %{Math.round((share.traffic / total) * 100)}</small>}
            </span>
          </li>
        ))}
      </ol>
    </section>
  );
}

/** Intent, result-page elements and brand searches of the first thousand keywords by traffic. */
function KeywordMix({ project, domain }: { project: ProjectRow; domain: string | undefined }) {
  const { t, locale } = useI18n();
  const terms = useMemo(() => brandTerms(domain ?? project.domain, domain && domain !== project.domain ? [] : project.brands), [domain, project.domain, project.brands]);
  return (
    <ReportFrame report="keywords" projectId={project.id} domain={domain} limit={MIX_ROWS} title={t("overview.mixTitle")} hint={t("overview.mixHint", { n: formatNumber(MIX_ROWS, locale) })}>
      {(data: RankedKeywords) => {
        const traffic = data.keywords.reduce((sum, row) => sum + row.traffic, 0);
        const split = brandSplit(data.keywords, terms);
        return (
          <div className="page-stack">
            <div className="share-grid">
              <ShareList title={t("overview.mixIntent")} shares={intentShares(data.keywords)} label={(key) => (key === "unknown" ? t("overview.intentUnknown") : t(`overview.intent_${key}`))} total={traffic} />
              <ShareList title={t("overview.mixFeatures")} shares={featureShares(data.keywords).slice(0, 8)} label={(key) => featureName(key, t)} total={traffic} />
              <ShareList title={t("overview.mixBrand")} shares={[split.branded, split.other]} label={(key) => (key === "branded" ? t("overview.branded") : t("overview.nonBranded"))} total={traffic} />
            </div>
            <p className="faint small">{terms.length > 0 ? t("overview.brandTerms", { terms: terms.join(", ") }) : t("overview.brandTermsNone")}</p>
          </div>
        );
      }}
    </ReportFrame>
  );
}

function CountriesTable({ rows }: { rows: CountryRow[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const [all, setAll] = useState(false);
  const regions = useMemo(() => new Intl.DisplayNames([locale], { type: "region" }), [locale]);
  const total = rows.reduce((sum, row) => sum + row.organic.traffic, 0);
  const name = (row: CountryRow) => {
    if (!row.country) return `#${row.location_code}`;
    try {
      return regions.of(row.country.toUpperCase()) ?? row.country;
    } catch {
      return row.country;
    }
  };
  const columns: Column<CountryRow>[] = [
    { key: "country", label: t("overview.colCountry"), render: (row) => <strong className="cell-main">{name(row)}</strong>, sort: name },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.organic.traffic, locale), sort: (row) => row.organic.traffic, numeric: true },
    { key: "share", label: t("overview.colShare"), render: (row) => (total > 0 ? `%${((row.organic.traffic / total) * 100).toFixed(1)}` : "—"), sort: (row) => row.organic.traffic, numeric: true },
    { key: "keywords", label: t("overview.colKeywords"), help: gloss("organic_keywords"), render: (row) => formatNumber(row.organic.keywords, locale), sort: (row) => row.organic.keywords, numeric: true },
    { key: "paid", label: t("overview.colPaidTraffic"), help: gloss("paid_traffic"), render: (row) => formatNumber(row.paid.traffic, locale), sort: (row) => row.paid.traffic, numeric: true },
  ];
  const shown = all ? rows : rows.slice(0, 10);
  return (
    <div className="page-stack">
      <DataTable rows={shown} columns={columns} rowKey={(row) => `${row.location_code}|${row.language ?? ""}`} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      {rows.length > 10 && (
        <Button small className="ghost" onClick={() => setAll(!all)}>
          {all ? t("overview.showTop10") : t("overview.showAllCountries", { n: rows.length })}
        </Button>
      )}
    </div>
  );
}

export function Summary({ project, domain }: { project: ProjectRow; domain: string | undefined }) {
  const { t } = useI18n();
  return (
    <div className="page-stack">
      <ReportFrame report="summary" projectId={project.id} domain={domain} title={t("overview.summaryTitle")} hint={t("overview.source")}>
        {(data) => <SummaryView data={data} />}
      </ReportFrame>
      <LinkProfile projectId={project.id} domain={domain} />
      <HistoryReport projectId={project.id} domain={domain} />
      <LinkHistory projectId={project.id} domain={domain} />
      <KeywordMix project={project} domain={domain} />
      <ReportFrame report="countries" projectId={project.id} domain={domain} title={t("overview.countriesTitle")} hint={t("overview.countriesHint")}>
        {(rows) => <CountriesTable rows={rows} />}
      </ReportFrame>
    </div>
  );
}
