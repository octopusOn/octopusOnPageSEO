// A domain's link profile month by month since 2019: authority, linking sites and backlinks,
// and the sites won and lost each month (from May 2021).

import { useMemo } from "react";
import type { LinkHistoryPoint } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";

function LinkHistoryChart({ points }: { points: LinkHistoryPoint[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    const months = points.map((point) => {
      const [year, month] = point.month.split("-").map(Number);
      return new Date(Date.UTC(year ?? 2020, (month ?? 1) - 1, 1)).toLocaleDateString(locale, { month: "short", year: "2-digit" });
    });
    const backlinks = t("backlinks.backlinks");
    const won = t("backlinks.newDomains");
    const lost = t("backlinks.lostDomains");
    return {
      grid: { left: 48, right: 56, top: 44, bottom: 28 },
      legend: { top: 0, left: 0, selected: { [backlinks]: false, [won]: false, [lost]: false } },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: months, ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", min: 0, max: 100, ...axis },
        { type: "value", ...axis, splitLine: { show: false }, axisLabel: { ...axis.axisLabel, formatter: (amount: number) => formatCompact(amount, locale) } },
      ],
      series: [
        { name: t("backlinks.authority"), type: "line", step: "end", symbol: "none", data: points.map((point) => point.rank) },
        { name: t("backlinks.referringDomains"), type: "line", smooth: true, symbol: "none", yAxisIndex: 1, areaStyle: { opacity: 0.1 }, data: points.map((point) => point.referring_domains) },
        { name: backlinks, type: "line", smooth: true, symbol: "none", yAxisIndex: 1, data: points.map((point) => point.backlinks) },
        { name: won, type: "bar", yAxisIndex: 1, barMaxWidth: 6, data: points.map((point) => point.new_referring_domains) },
        { name: lost, type: "bar", yAxisIndex: 1, barMaxWidth: 6, data: points.map((point) => -point.lost_referring_domains) },
      ],
    };
  }, [points, tokens, t, locale]);
  if (points.length === 0) return <p className="faint">{t("backlinks.noHistory")}</p>;
  return <EChart option={option} height={280} />;
}

/** The authority history of a domain (the project's unless `domain` is given). */
export function LinkHistory({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t } = useI18n();
  return (
    <PaidFrame<LinkHistoryPoint[]> module="backlinks" ask={{ project_id: projectId, report: "history", ...(domain ? { target: domain } : {}) }} title={t("overview.linkHistory")} hint={t("overview.linkHistoryHint")}>
      {(points) => <LinkHistoryChart points={points} />}
    </PaidFrame>
  );
}
