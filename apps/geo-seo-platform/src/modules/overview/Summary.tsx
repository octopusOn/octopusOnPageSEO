import { useMemo } from "react";
import { ArrowDownRight, ArrowUpRight, Globe } from "lucide-react";
import { Empty, Stat } from "@octopusseo/ui";
import type { DomainOverview, HistoryPoint, RankMetrics } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { ReportFrame } from "./ReportFrame";

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
        <Stat label={t("overview.organicKeywords")} value={formatCompact(organic.keywords, locale)} hint={formatNumber(organic.keywords, locale)} />
        <Stat label={t("overview.organicTraffic")} value={formatCompact(organic.traffic, locale)} hint={`${formatNumber(organic.traffic, locale)} ${t("overview.perMonth")}`} />
        <Stat label={t("overview.trafficValue")} value={formatUsd(organic.traffic_value_usd, locale)} hint={t("overview.perMonth")} />
        <Stat label={t("overview.paidKeywords")} value={formatCompact(paid.keywords, locale)} hint={`${t("overview.paidTraffic")}: ${formatCompact(paid.traffic, locale)}`} />
      </div>
      <div className="two-columns">
        <Positions metrics={organic} />
        <Movement metrics={organic} />
      </div>
    </div>
  );
}

function HistoryChart({ points }: { points: HistoryPoint[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const months = points.map((point) => new Date(Date.UTC(point.year, point.month - 1, 1)).toLocaleDateString(locale, { month: "short", year: "2-digit" }));
    const axis = axisStyle(tokens);
    return {
      grid: { left: 56, right: 56, top: 36, bottom: 28 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: months, ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
        { type: "value", ...axis, splitLine: { show: false }, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      ],
      series: [
        {
          name: t("overview.trafficSeries"),
          type: "line",
          smooth: true,
          symbol: "none",
          areaStyle: { opacity: 0.12 },
          data: points.map((point) => Math.round(point.organic.traffic)),
        },
        {
          name: t("overview.keywordsSeries"),
          type: "line",
          smooth: true,
          symbol: "none",
          yAxisIndex: 1,
          data: points.map((point) => point.organic.keywords),
        },
      ],
    };
  }, [points, tokens, t, locale]);
  if (points.length === 0) return <p className="faint">{t("overview.noDataText")}</p>;
  return <EChart option={option} height={300} />;
}

export function Summary({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t } = useI18n();
  return (
    <div className="page-stack">
      <ReportFrame report="summary" projectId={projectId} domain={domain} title={t("overview.summaryTitle")} hint={t("overview.source")}>
        {(data) => <SummaryView data={data} />}
      </ReportFrame>
      <ReportFrame report="history" projectId={projectId} domain={domain} title={t("overview.history")} hint={t("overview.historyHint")}>
        {(points) => <HistoryChart points={points} />}
      </ReportFrame>
    </div>
  );
}
