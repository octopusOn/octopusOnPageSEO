import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, FileUp, LineChart } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, QueryPageResult, SiteOverview } from "../api/types";
import { EChart, themeColors, type ChartOption } from "../components/EChart";
import { Button, Card, Empty, ErrorBanner, Kpi, Segmented, Spinner } from "../components/ui";
import { formatBytes, formatNumber, formatPercent } from "../lib/format";
import { joinQuery } from "../lib/queryBuilder";
import { useStored } from "../lib/storage";
import { useI18n, type Translate } from "../i18n";
import type { MessageKey } from "../i18n/en";
import { keys, useAppState } from "../state";

type Period = "all" | "last7d" | "last30d" | "last90d";

/** Days × series matrix from rows of (day, series, hits). */
function pivot(page: QueryPageResult | undefined) {
  const days: string[] = [];
  const series = new Map<string, Map<string, number>>();
  for (const row of page?.rows ?? []) {
    const day = String(row[0]);
    const name = row[1] === null ? "" : String(row[1]);
    const hits = Number(row[2] ?? 0);
    if (days[days.length - 1] !== day) days.push(day);
    const values = series.get(name) ?? new Map<string, number>();
    values.set(day, hits);
    series.set(name, values);
  }
  return { days, series };
}

function categoryLabel(t: Translate, name: string): string {
  const key = `category.${name || "browser"}` as MessageKey;
  return t(key) === key ? name : t(key);
}

export function OverviewPage({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const { setPage } = useAppState();
  const [period, setPeriod] = useStored<Period>("overview-period", "last30d");
  const periodValue = period === "all" ? null : period;
  const date = periodValue ? `date:${periodValue}` : null;

  const info = useQuery<SiteOverview, CommandError>({ queryKey: keys.overview(site), queryFn: () => api.siteOverview(site) });
  const crawl = useQuery<Awaited<ReturnType<typeof api.crawlOverview>>, CommandError>({
    queryKey: keys.crawl(site, periodValue),
    queryFn: () => api.crawlOverview(site, periodValue),
    enabled: (info.data?.hits ?? 0) > 0,
  });
  const daily = useQuery({
    queryKey: keys.query(site, `overview-daily ${periodValue}`),
    queryFn: () =>
      api.runQuery(
        site,
        { kind: "text", text: joinQuery(date, "group:day,bot_category metric:hits sort:day:asc,bot_category:asc") },
        0,
        10000,
      ),
    enabled: (info.data?.hits ?? 0) > 0,
  });
  const devices = useQuery({
    queryKey: keys.query(site, `overview-devices ${periodValue}`),
    queryFn: () =>
      api.runQuery(
        site,
        {
          kind: "text",
          text: joinQuery(
            date,
            "bot:googlebot-smartphone,googlebot-desktop verification:verified group:day,bot metric:hits sort:day:asc,bot:asc",
          ),
        },
        0,
        10000,
      ),
    enabled: (info.data?.hits ?? 0) > 0,
  });

  const colors = themeColors();
  const axis = {
    axisLine: { lineStyle: { color: colors.border } },
    axisLabel: { color: colors.text },
    splitLine: { lineStyle: { color: colors.border } },
  };

  const dailyOption = useMemo<ChartOption>(() => {
    const { days, series } = pivot(daily.data);
    return {
      tooltip: { trigger: "axis" },
      legend: { top: 0, left: "center", textStyle: { color: colors.text }, type: "scroll" },
      grid: { left: 56, right: 16, top: 40, bottom: 28 },
      xAxis: { type: "category", data: days, ...axis },
      yAxis: { type: "value", ...axis },
      series: [...series.entries()].map(([name, values]) => ({
        name: categoryLabel(t, name),
        type: "bar",
        stack: "hits",
        data: days.map((day) => values.get(day) ?? 0),
      })),
    };
  }, [daily.data, t, colors.text, colors.border]);

  const devicesOption = useMemo<ChartOption>(() => {
    const { days, series } = pivot(devices.data);
    return {
      tooltip: { trigger: "axis" },
      legend: { top: 0, left: "center", textStyle: { color: colors.text } },
      grid: { left: 56, right: 16, top: 40, bottom: 28 },
      xAxis: { type: "category", data: days, ...axis },
      yAxis: { type: "value", ...axis },
      series: [...series.entries()].map(([name, values]) => ({
        name,
        type: "line",
        smooth: true,
        showSymbol: false,
        areaStyle: { opacity: 0.08 },
        data: days.map((day) => values.get(day) ?? 0),
      })),
    };
  }, [devices.data, colors.text, colors.border]);

  const statusOption = useMemo<ChartOption>(
    () => ({
      tooltip: { trigger: "item" },
      legend: { bottom: 0, textStyle: { color: colors.text } },
      series: [
        {
          type: "pie",
          radius: ["48%", "72%"],
          center: ["50%", "45%"],
          label: { color: colors.text },
          data: (crawl.data?.search.status_classes ?? []).map((share) => ({ name: share.name, value: share.value })),
          color: ["#22b893", "#4f5bd5", "#f2a33a", "#e5566d", "#9aa5b8"],
        },
      ],
    }),
    [crawl.data, colors.text],
  );

  if (info.isLoading) return <div className="page"><Spinner /></div>;
  if (info.error) return <div className="page"><ErrorBanner error={info.error} /></div>;
  if ((info.data?.hits ?? 0) === 0) {
    return (
      <div className="page">
        <Empty icon={<LineChart size={40} />} title={t("overview.noData")}>
          <Button variant="primary" icon={<FileUp size={16} />} onClick={() => setPage("import")}>
            {t("overview.goImport")}
          </Button>
        </Empty>
      </div>
    );
  }

  const overview = crawl.data;
  const n = (value: number) => formatNumber(value, locale);
  return (
    <div className="page">
      <div className="row between">
        <Segmented<Period>
          value={period}
          onChange={setPeriod}
          options={[
            { value: "last7d", label: t("overview.last7") },
            { value: "last30d", label: t("overview.last30") },
            { value: "last90d", label: t("overview.last90") },
            { value: "all", label: t("overview.allData") },
          ]}
        />
        {(crawl.isFetching || daily.isFetching) && <Spinner />}
      </div>
      <ErrorBanner error={crawl.error} />
      {overview && (
        <div className="grid-kpi">
          <Kpi label={t("overview.totalHits")} value={n(overview.total_hits)} />
          <Kpi label={t("overview.searchHits")} value={n(overview.search.hits)} tone="success" />
          <Kpi label={t("overview.urlsCrawled")} value={n(overview.search.unique_urls)} />
          <Kpi label={t("overview.hitsPerDay")} value={n(Math.round(overview.search.hits_per_day))} />
          <Kpi label={t("overview.urlsPerDay")} value={n(Math.round(overview.search.unique_urls_per_day))} />
          <Kpi
            label={t("overview.errors")}
            value={n(overview.search.errors)}
            tone={overview.search.errors > 0 ? "danger" : undefined}
          />
          {overview.search.avg_bytes !== null && (
            <Kpi label={t("overview.avgBytes")} value={formatBytes(overview.search.avg_bytes, locale)} />
          )}
          {overview.search.avg_ms !== null && (
            <Kpi label={t("overview.avgMs")} value={`${n(Math.round(overview.search.avg_ms))} ms`} />
          )}
          <Kpi label={t("overview.nonSuccess")} value={formatPercent(overview.search.non_success_ratio, locale)} />
          <Kpi label={t("overview.parameters")} value={formatPercent(overview.search.parameter_ratio, locale)} />
          <Kpi label={t("overview.assets")} value={formatPercent(overview.search.asset_ratio, locale)} />
          <Kpi
            label={t("overview.spoofed")}
            value={n(overview.spoofed_hits)}
            tone={overview.spoofed_hits > 0 ? "danger" : undefined}
          />
        </div>
      )}
      {overview && overview.alerts.length > 0 && (
        <Card title={t("alerts.title")}>
          <ul className="alerts">
            {overview.alerts.map((alert) => (
              <li key={`${alert.day}-${alert.series}-${alert.kind}`} className={`alert ${alert.kind}`}>
                <AlertTriangle size={15} />
                <span className="mono">{alert.day}</span>
                <strong>{t(`alerts.series.${alert.series}` as MessageKey)}</strong>
                <span>
                  {alert.kind === "server-errors"
                    ? t("alerts.errors", { value: formatPercent(alert.value, locale), baseline: formatPercent(alert.baseline, locale) })
                    : t(alert.kind === "drop" ? "alerts.drop" : "alerts.spike", {
                        value: n(Math.round(alert.value)),
                        baseline: n(Math.round(alert.baseline)),
                        change: formatPercent(Math.abs(alert.value - alert.baseline) / Math.max(alert.baseline, 1), locale),
                      })}
                </span>
              </li>
            ))}
          </ul>
        </Card>
      )}
      <Card title={t("overview.dailyTraffic")}>
        <EChart option={dailyOption} height={300} />
      </Card>
      <div className="grid-2">
        <Card title={t("overview.googlebotDevices")}>
          <EChart option={devicesOption} />
        </Card>
        <Card title={t("overview.statusClasses")}>
          <EChart option={statusOption} />
        </Card>
      </div>
      {overview && (
        <div className="grid-2">
          <Card title={t("overview.categories")}>
            <table className="table-simple">
              <tbody>
                {overview.categories.map((share) => (
                  <tr key={share.name}>
                    <td>{categoryLabel(t, share.name === "browser" ? "" : share.name)}</td>
                    <td className="num">{n(share.value)}</td>
                    <td className="num muted">{formatPercent(share.ratio, locale)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Card>
          <Card title={t("overview.bots")}>
            <div className="table-wrap">
              <table className="table-simple">
                <thead>
                  <tr>
                    <th>{t("bots.bot")}</th>
                    <th className="num">{t("overview.verified")}</th>
                    <th className="num">{t("overview.spoofedColumn")}</th>
                    <th className="num">{t("overview.unverified")}</th>
                  </tr>
                </thead>
                <tbody>
                  {overview.bots.slice(0, 15).map((bot) => (
                    <tr key={bot.bot}>
                      <td>{bot.bot}</td>
                      <td className="num">{n(bot.verified)}</td>
                      <td className="num" style={{ color: bot.spoofed ? "var(--danger)" : undefined }}>
                        {n(bot.spoofed)}
                      </td>
                      <td className="num muted">{n(bot.unverified)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
