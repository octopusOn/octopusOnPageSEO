import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  ArrowDown,
  ArrowDownRight,
  ArrowUp,
  ArrowUpRight,
  Award,
  Download,
  MapPin,
  Play,
  Plus,
  Search,
  Sparkles,
  Tag,
  Trash2,
  TrendingUp,
} from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Segmented, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { CompetitorShare, Estimate, Job, KeywordRank, ProjectRow, RankReport, RankRun, RankSetup, Schedule, SharePoint, TrackerRow, VisibilityPoint } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatCredits, formatNumber, formatPercent, formatUsd, relativeTime } from "../../lib/format";
import { blockedText, CostLabel, refreshSpending } from "../../lib/PaidFrame";
import { download, toCsv } from "../../lib/report";
import { featureName, shortUrl, tableLabels } from "../../lib/tables";
import { Sparkline } from "../keywords/KeywordTable";
import { ALL_RANKS, BANDS, csvRows, DEPTHS, filterRanks, heights, PERIODS, placeLabel, tagsOf, type Band, type RankFilters } from "./rank";
import { AddKeywordsDialog, KeywordDialog, PlacesDialog } from "./RankDialogs";

type T = (key: string, values?: Record<string, string | number>) => string;

/** An amount in the way it is paid: credits on credits, dollars on the user's own key. */
function money(usd: number, credits: number | null, estimate: Estimate | undefined, t: T, locale: string): string {
  return estimate?.via === "credit" && credits !== null ? t("common.credits", { n: formatCredits(credits, locale) }) : formatUsd(usd, locale);
}

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

/** Places, schedule, depth, what a run costs and the run itself. */
function SetupBar({
  project,
  setup,
  tracker,
  running,
  onTracker,
  onPlaces,
  onAdd,
}: {
  project: ProjectRow;
  setup: RankSetup;
  tracker: TrackerRow | null;
  running: Job | undefined;
  onTracker: (id: number) => void;
  onPlaces: () => void;
  onAdd: () => void;
}) {
  const { t, locale } = useI18n();
  const { notify, go } = useShell();
  const queryClient = useQueryClient();
  const estimate = useQuery({ queryKey: ["rank", "estimate", project.id], queryFn: () => api.rankEstimate(project.id), enabled: setup.keywords > 0 });
  const save = useMutation({
    mutationFn: (next: { schedule: Schedule; depth: number; ai_overview: boolean }) => api.saveRankSettings(project.id, next),
    onSuccess: (next) => {
      queryClient.setQueryData(["rank", "setup", project.id], next);
      void queryClient.invalidateQueries({ queryKey: ["rank", "estimate", project.id] });
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const start = useMutation({
    mutationFn: () => api.startRankCheck(project.id),
    onSuccess: () => {
      notify(t("rank.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const { settings } = setup;
  const change = (patch: Partial<{ schedule: Schedule; depth: number; ai_overview: boolean }>) =>
    save.mutate({ schedule: settings.schedule, depth: settings.depth, ai_overview: settings.ai_overview, ...patch });
  const run = estimate.data?.run;
  const blocked = blockedText(run, t);
  const deviceName = (name: string) => t(`rank.${name}`);
  const next =
    settings.schedule === "manual" || !setup.next_check_at ? t("rank.manualOnly") : t("rank.nextCheck", { time: relativeTime(setup.next_check_at, locale) });
  return (
    <section className="oc-panel rank-setup">
      <div className="rank-places" role="group" aria-label={t("rank.places")}>
        {setup.trackers.map((entry) => (
          <button
            key={entry.id}
            type="button"
            className={`oc-chip chip-button ${entry.id === tracker?.id ? "accent" : ""}`}
            aria-pressed={entry.id === tracker?.id}
            onClick={() => onTracker(entry.id)}
          >
            <MapPin size={13} aria-hidden="true" />
            {placeLabel(entry, deviceName)}
          </button>
        ))}
        <Button small onClick={onPlaces}>
          <Plus size={14} />
          {setup.trackers.length ? t("rank.managePlaces") : t("rank.addPlace")}
        </Button>
      </div>
      <div className="rank-controls">
        <Field label={t("rank.schedule")}>
          <Segmented<Schedule>
            label={t("rank.schedule")}
            value={settings.schedule}
            disabled={save.isPending}
            onChange={(schedule) => change({ schedule })}
            options={[
              { value: "daily", label: t("rank.scheduleDaily") },
              { value: "weekly", label: t("rank.scheduleWeekly") },
              { value: "manual", label: t("rank.scheduleManual") },
            ]}
          />
        </Field>
        <Field label={t("rank.depth")}>
          <select className="oc-select" value={settings.depth} disabled={save.isPending} onChange={(event) => change({ depth: Number(event.target.value) })}>
            {DEPTHS.map((depth) => (
              <option key={depth} value={depth}>
                {t("rank.depthOption", { n: depth })}
              </option>
            ))}
          </select>
        </Field>
        <label className="oc-check" title={t("rank.aiOverviewHint")}>
          <input type="checkbox" checked={settings.ai_overview} disabled={save.isPending} onChange={(event) => change({ ai_overview: event.target.checked })} />
          <span>{t("rank.aiOverview")}</span>
        </label>
      </div>
      <div className="rank-run">
        <div className="rank-run-text">
          <span>{next}</span>
          {estimate.data && run && !blocked && setup.keywords > 0 && (
            <span className="faint small">
              {t("rank.runCost", { n: formatNumber(estimate.data.checks, locale) })} <CostLabel estimate={run} />
              {estimate.data.runs_per_month > 0 && <> · {t("rank.monthCost", { cost: money(estimate.data.month_usd, estimate.data.month_credits, run, t, locale) })}</>}
            </span>
          )}
        </div>
        <Button small onClick={onAdd}>
          <Plus size={14} />
          {t("rank.addKeywords")}
        </Button>
        <Button primary small busy={start.isPending} disabled={Boolean(blocked) || Boolean(running) || setup.keywords === 0} title={blocked ?? undefined} onClick={() => start.mutate()}>
          <Play size={14} />
          {t("rank.checkNow")}
        </Button>
      </div>
      {running && (
        <div className="rank-progress" aria-live="polite">
          <div className="oc-progress" aria-hidden="true">
            <div style={{ width: `${running.total ? (running.done / running.total) * 100 : 5}%` }} />
          </div>
          <span className="small">
            {t("rank.checking", { done: formatNumber(running.done, locale), total: formatNumber(running.total, locale) })} <span className="faint">{running.step}</span>
          </span>
          <Button small busy={cancel.isPending} onClick={() => cancel.mutate(running.id)}>
            {t("rank.cancel")}
          </Button>
        </div>
      )}
      {blocked && setup.keywords > 0 && (
        <div className="oc-banner warning" role="status">
          <span>{blocked}</span>
          <Button small onClick={() => go({ page: "sources" })}>
            {t("common.goToSources")}
          </Button>
        </div>
      )}
    </section>
  );
}

/** Keywords without a volume weigh as one search; fetching their volumes fixes visibility. */
function VolumesBanner({ project, missing }: { project: ProjectRow; missing: number }) {
  const { t, locale } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const estimate = useQuery({ queryKey: ["rank", "volumes", project.id, missing], queryFn: () => api.rankVolumesEstimate(project.id) });
  const fetch = useMutation({
    mutationFn: () => api.fetchRankVolumes(project.id),
    onSuccess: (done) => {
      notify(t("rank.volumesFetched", { n: formatNumber(done.updated, locale) }));
      void queryClient.invalidateQueries({ queryKey: ["rank"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const blocked = blockedText(estimate.data, t);
  return (
    <div className="oc-banner info" role="status">
      <span>{t("rank.volumesMissing", { n: formatNumber(missing, locale) })}</span>
      {estimate.data && !blocked && <CostLabel estimate={estimate.data} />}
      <Button small busy={fetch.isPending} disabled={Boolean(blocked) || !estimate.data} title={blocked ?? undefined} onClick={() => fetch.mutate()}>
        {t("rank.fetchVolumes")}
      </Button>
    </div>
  );
}

function VisibilityChart({ points }: { points: VisibilityPoint[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 44, right: 44, top: 36, bottom: 28 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: points.map((point) => new Date(point.date).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", min: 0, ...axis, axisLabel: { ...axis.axisLabel, formatter: "{value}%" } },
        { type: "value", min: 0, minInterval: 1, ...axis, splitLine: { show: false } },
      ],
      series: [
        { name: t("rank.visibility"), type: "line", smooth: true, symbol: "none", areaStyle: { opacity: 0.12 }, data: points.map((point) => point.visibility) },
        { name: t("rank.top10"), type: "line", step: "end", symbol: "none", yAxisIndex: 1, lineStyle: { width: 1.5, type: "dashed" }, data: points.map((point) => point.top10) },
        { name: t("rank.top3"), type: "line", step: "end", symbol: "none", yAxisIndex: 1, lineStyle: { width: 1.5, type: "dashed" }, data: points.map((point) => point.top3) },
      ],
    };
  }, [points, tokens, t, locale]);
  return <EChart option={option} height={260} />;
}

function ShareBars({ shares }: { shares: CompetitorShare[] }) {
  const { t, locale } = useI18n();
  const most = Math.max(1, ...shares.map((share) => share.visibility));
  return (
    <ol className="bars domains" aria-label={t("rank.sharesTitle")}>
      {shares.map((share) => (
        <li key={share.domain} className={share.own ? "own" : undefined}>
          <span className="bar-label" title={share.domain}>
            {share.domain}
          </span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${(share.visibility / most) * 100}%` }} />
          </span>
          <span className="bar-value" title={t("rank.shareLine", { n: share.keywords, position: share.average_position === null ? "—" : one(share.average_position, locale) })}>
            {formatPercent(share.visibility, locale)}
          </span>
        </li>
      ))}
    </ol>
  );
}

function ShareHistory({ history, shares }: { history: SharePoint[]; shares: CompetitorShare[] }) {
  const { locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 44, right: 16, top: 36, bottom: 28 },
      legend: { top: 0, left: 0, type: "scroll" },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: history.map((point) => new Date(point.date).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: { type: "value", min: 0, ...axis, axisLabel: { ...axis.axisLabel, formatter: "{value}%" } },
      series: shares.map((share) => ({
        name: share.domain,
        type: "line",
        smooth: true,
        symbol: "none",
        lineStyle: { width: share.own ? 3 : 1.5 },
        data: history.map((point) => point.shares[share.domain] ?? 0),
      })),
    };
  }, [history, shares, tokens, locale]);
  return <EChart option={option} height={260} />;
}

/** A position with its change since the check before. */
function Position({ row }: { row: KeywordRank }) {
  const { t } = useI18n();
  if (row.position === null && row.error && row.previous === null) {
    return (
      <span className="rank-failed" title={t("rank.checkFailed", { error: row.error })}>
        <AlertTriangle size={14} aria-hidden="true" />
        <span className="sr-only">{t("rank.checkFailed", { error: row.error })}</span>
      </span>
    );
  }
  return (
    <span className="rank-position">
      <strong>{row.position ?? <span className="faint">{t("rank.notRanked")}</span>}</strong>
      {row.change !== null && row.change !== 0 && (
        <span className={`rank-change ${row.change > 0 ? "up" : "down"}`} title={t("rank.changeTitle", { previous: row.previous ?? "—" })}>
          {row.change > 0 ? <ArrowUp size={12} aria-hidden="true" /> : <ArrowDown size={12} aria-hidden="true" />}
          {Math.abs(row.change)}
        </span>
      )}
    </span>
  );
}

/** What else the result page shows for the site: AI Overview, snippet, competing pages. */
function Signals({ row }: { row: KeywordRank }) {
  const { t } = useI18n();
  const features = row.features
    .filter((feature) => feature !== "organic")
    .map((feature) => featureName(feature, t))
    .join(", ");
  return (
    <span className="rank-signals" title={features}>
      {row.ai_overview && (
        <span className={row.ai_cited ? "signal cited" : "signal"} title={row.ai_cited ? t("rank.aiCited") : t("rank.aiShownOnly")}>
          <Sparkles size={13} aria-hidden="true" />
          AI
        </span>
      )}
      {row.snippet && (
        <span className="signal good" title={t("rank.snippetOwned")}>
          <Award size={13} aria-hidden="true" />
        </span>
      )}
      {row.cannibalised && (
        <span className="signal warning" title={`${t("rank.cannibalised")}: ${row.pages.map(([position, url]) => `${position}. ${shortUrl(url)}`).join(", ")}`}>
          <AlertTriangle size={13} aria-hidden="true" />
          {row.pages.length}
        </span>
      )}
    </span>
  );
}

function KeywordRanks({ project, rows, onOpen }: { project: ProjectRow; rows: KeywordRank[]; onOpen: (row: KeywordRank) => void }) {
  const { t, locale } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const [filters, setFilters] = useState<RankFilters>(ALL_RANKS);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const shown = useMemo(() => filterRanks(rows, filters), [rows, filters]);
  const tags = useMemo(() => tagsOf(rows), [rows]);
  const chosen = rows.filter((row) => selected.has(row.keyword_id)).map((row) => row.keyword_id);
  const refresh = () => {
    setSelected(new Set());
    void queryClient.invalidateQueries({ queryKey: ["rank"] });
  };
  const untrack = useMutation({
    mutationFn: () => api.untrackKeywords(project.id, chosen),
    onSuccess: () => {
      notify(t("rank.removed", { n: chosen.length }));
      refresh();
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const tag = useMutation({
    mutationFn: (names: string[]) => api.setKeywordTags(project.id, chosen, names),
    onSuccess: refresh,
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const toggle = (id: number) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const allShown = shown.length > 0 && shown.every((row) => selected.has(row.keyword_id));
  const columns: Column<KeywordRank>[] = [
    {
      key: "select",
      label: "",
      width: "36px",
      render: (row) => <input type="checkbox" checked={selected.has(row.keyword_id)} onChange={() => toggle(row.keyword_id)} aria-label={t("keywords.select", { keyword: row.keyword })} />,
    },
    {
      key: "keyword",
      label: t("overview.colKeyword"),
      render: (row) => (
        <span className="rank-keyword">
          <button type="button" className="link-button cell-main" onClick={() => onOpen(row)} title={t("rank.openDetail")}>
            {row.keyword}
          </button>
          {row.tags.map((name) => (
            <span key={name} className="oc-chip small">
              {name}
            </span>
          ))}
        </span>
      ),
      sort: (row) => row.keyword,
    },
    { key: "position", label: t("rank.colPosition"), render: (row) => <Position row={row} />, sort: (row) => row.position ?? 1000, numeric: true },
    { key: "best", label: t("rank.colBest"), render: (row) => row.best ?? "—", sort: (row) => row.best ?? 1000, numeric: true },
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    {
      key: "url",
      label: t("rank.colUrl"),
      render: (row) =>
        row.url ? (
          <a className="faint small" href={row.url} target="_blank" rel="noreferrer noopener" title={row.url}>
            {shortUrl(row.url)}
          </a>
        ) : (
          ""
        ),
      sort: (row) => row.url,
    },
    { key: "signals", label: t("rank.colSerp"), render: (row) => <Signals row={row} />, sort: (row) => Number(row.ai_cited) * 4 + Number(row.ai_overview) * 2 + Number(row.snippet) },
    { key: "history", label: t("rank.colHistory"), render: (row) => <Sparkline values={heights(row.history)} /> },
    { key: "checked", label: t("rank.colChecked"), render: (row) => (row.checked_at ? <span className="faint small">{relativeTime(row.checked_at, locale)}</span> : "—"), sort: (row) => row.checked_at },
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("rank.keywordsTitle")}</h3>
          <span className="faint">{t("rank.keywordsHint")}</span>
        </div>
      </header>
      <div className="page-stack">
        <div className="filters">
          <label className="filter-search">
            <Search size={15} aria-hidden="true" />
            <span className="sr-only">{t("rank.filterSearch")}</span>
            <input className="oc-input" value={filters.query} placeholder={t("rank.filterSearch")} onChange={(event) => setFilters({ ...filters, query: event.target.value })} />
          </label>
          {tags.length > 0 && (
            <Field label={t("rank.filterTag")}>
              <select className="oc-select" value={filters.tag} onChange={(event) => setFilters({ ...filters, tag: event.target.value })}>
                <option value="">{t("rank.allTags")}</option>
                {tags.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </Field>
          )}
          <Field label={t("rank.filterBand")}>
            <select className="oc-select" value={filters.band} onChange={(event) => setFilters({ ...filters, band: event.target.value as Band })}>
              {BANDS.map((band) => (
                <option key={band} value={band}>
                  {t(`rank.band_${band}`)}
                </option>
              ))}
            </select>
          </Field>
          <label className="oc-check">
            <input type="checkbox" checked={filters.aiOnly} onChange={(event) => setFilters({ ...filters, aiOnly: event.target.checked })} />
            <span>{t("rank.aiOnly")}</span>
          </label>
        </div>
        <div className="table-toolbar">
          <label className="oc-check">
            <input
              type="checkbox"
              checked={allShown}
              onChange={() =>
                setSelected((previous) => {
                  const next = new Set(previous);
                  for (const row of shown) {
                    if (allShown) next.delete(row.keyword_id);
                    else next.add(row.keyword_id);
                  }
                  return next;
                })
              }
            />
            <span>{t("keywords.selectShown", { n: formatNumber(shown.length, locale) })}</span>
          </label>
          <Button
            small
            disabled={chosen.length === 0}
            busy={tag.isPending}
            onClick={() => {
              const names = window.prompt(t("rank.tagPrompt"), "");
              if (names !== null) tag.mutate(names.split(",").map((name) => name.trim()).filter(Boolean));
            }}
          >
            <Tag size={14} />
            {t("rank.tagSelected")}
          </Button>
          <Button
            small
            disabled={chosen.length === 0}
            busy={untrack.isPending}
            onClick={() => {
              if (window.confirm(t("rank.removeConfirm", { n: chosen.length }))) untrack.mutate();
            }}
          >
            <Trash2 size={14} />
            {t("rank.removeSelected", { n: chosen.length })}
          </Button>
          <Button
            small
            disabled={shown.length === 0}
            onClick={() =>
              download(
                `rank-${project.domain}.csv`,
                toCsv(
                  [
                    t("overview.colKeyword"),
                    t("rank.tagsColumn"),
                    t("rank.colPosition"),
                    t("rank.colChange"),
                    t("rank.colBest"),
                    t("overview.colVolume"),
                    t("rank.colUrl"),
                    "AI Overview",
                    t("rank.aiCitedColumn"),
                    t("rank.colChecked"),
                  ],
                  csvRows(shown),
                ),
              )
            }
          >
            <Download size={14} />
            {t("overview.export")}
          </Button>
        </div>
        <DataTable rows={shown} columns={columns} rowKey={(row) => String(row.keyword_id)} initialSort={{ key: "position", direction: "asc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      </div>
    </section>
  );
}

function Runs({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const runs = useQuery({ queryKey: ["rank", "runs", project.id], queryFn: () => api.rankRuns(project.id) });
  if (!runs.data?.length) return null;
  const tone = (run: RankRun) => (run.status === "done" ? "good" : run.status === "running" ? "accent" : run.status === "partial" ? "warning" : "critical");
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("rank.runsTitle")}</h3>
        </div>
      </header>
      <ul className="run-list">
        {runs.data.map((run) => (
          <li key={run.id}>
            <span className={`oc-badge ${tone(run)}`}>{t(`rank.run_${run.status}`)}</span>
            <span>{relativeTime(run.started_at, locale)}</span>
            <span className="faint small">{run.scheduled ? t("rank.runScheduled") : t("rank.runManual")}</span>
            <span className="small">{t("rank.runLine", { checked: formatNumber(run.checked, locale), failed: formatNumber(run.failed, locale), cost: formatUsd(run.cost_usd, locale) })}</span>
            {run.error && <span className="faint small run-error">{run.error}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

function ReportView({ project, report, days, onDays, onOpen }: { project: ProjectRow; report: RankReport; days: number; onDays: (days: number) => void; onOpen: (row: KeywordRank) => void }) {
  const { t, locale } = useI18n();
  const { summary } = report;
  const period = (
    <Segmented<string>
      label={t("rank.days")}
      value={String(days)}
      onChange={(value) => onDays(Number(value))}
      options={PERIODS.map((value) => ({ value: String(value), label: t(`rank.days${value}`) }))}
    />
  );
  if (summary.checked === 0) {
    return (
      <div className="oc-card">
        <Empty icon={<TrendingUp size={28} />} title={t("rank.noChecksTitle")}>
          <p>{t("rank.noChecksText")}</p>
        </Empty>
      </div>
    );
  }
  const change = summary.visibility_change;
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat
          label={t("rank.visibility")}
          value={summary.visibility === null ? "—" : formatPercent(summary.visibility, locale)}
          hint={change === null ? t("rank.visibilityHint") : t("rank.visibilityChange", { change: `${change > 0 ? "+" : ""}${one(change, locale)}` })}
          tone={change === null || change === 0 ? undefined : change > 0 ? "good" : "bad"}
        />
        <Stat
          label={t("rank.averagePosition")}
          value={summary.average_position === null ? "—" : one(summary.average_position, locale)}
          hint={t("rank.rankedOf", { ranked: formatNumber(summary.ranked, locale), total: formatNumber(summary.checked, locale) })}
        />
        <Stat label={t("rank.topCounts")} value={`${formatNumber(summary.top3, locale)} / ${formatNumber(summary.top10, locale)}`} hint={t("rank.topCountsHint")} />
        <Stat label={t("rank.aiTitle")} value={formatNumber(summary.ai_overviews, locale)} hint={t("rank.aiCitedCount", { n: formatNumber(summary.ai_cited, locale) })} />
      </div>
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("rank.visibilityChart")}</h3>
            <span className="faint">{t("rank.source")}</span>
          </div>
          <div className="report-actions">{period}</div>
        </header>
        <div className="two-columns">
          <section className="sub-panel">
            <h4>{t("rank.visibility")}</h4>
            {report.visibility.length > 1 ? <VisibilityChart points={report.visibility} /> : <p className="faint">{t("rank.oneDay")}</p>}
          </section>
          <section className="sub-panel">
            <h4>{t("rank.movement")}</h4>
            <div className="movement">
              {(
                [
                  ["rank.improved", summary.improved, "good"],
                  ["rank.declined", summary.declined, "bad"],
                  ["rank.lost", summary.lost, "bad"],
                  ["rank.snippets", summary.snippets, "good"],
                  ["rank.cannibalisedCount", summary.cannibalised, "bad"],
                ] as const
              ).map(([label, value, tone]) => (
                <div key={label} className={`movement-item ${tone}`}>
                  {tone === "good" ? <ArrowUpRight size={16} aria-hidden="true" /> : <ArrowDownRight size={16} aria-hidden="true" />}
                  <span>{t(label)}</span>
                  <strong>{formatNumber(value, locale)}</strong>
                </div>
              ))}
            </div>
          </section>
        </div>
      </section>
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("rank.sharesTitle")}</h3>
            <span className="faint">{t("rank.sharesHint")}</span>
          </div>
        </header>
        <div className="two-columns">
          <section className="sub-panel">
            <h4>{t("rank.sharesNow")}</h4>
            <ShareBars shares={report.competitors} />
          </section>
          <section className="sub-panel">
            <h4>{t("rank.shareHistory")}</h4>
            {report.share_history.length > 1 ? <ShareHistory history={report.share_history} shares={report.competitors} /> : <p className="faint">{t("rank.oneDay")}</p>}
          </section>
        </div>
      </section>
      <KeywordRanks project={project} rows={report.rows} onOpen={onOpen} />
    </div>
  );
}

export function RankTracker({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { jobs } = useShell();
  const setup = useQuery({ queryKey: ["rank", "setup", project.id], queryFn: () => api.rankSetup(project.id) });
  const [trackerId, setTrackerId] = useState<number | null>(null);
  const [days, setDays] = useState(90);
  const [adding, setAdding] = useState(false);
  const [places, setPlaces] = useState(false);
  const [detail, setDetail] = useState<KeywordRank | null>(null);
  const trackers = setup.data?.trackers ?? [];
  const tracker = trackers.find((entry) => entry.id === trackerId) ?? trackers[0] ?? null;
  const report = useQuery({
    queryKey: ["rank", "report", project.id, tracker?.id ?? null, days],
    queryFn: () => api.rankReport(project.id, tracker?.id ?? null, days),
    enabled: Boolean(setup.data && setup.data.keywords > 0),
  });
  const running = jobs.find((job) => job.kind === "rank-check" && job.project_id === project.id && job.status === "running");
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  return (
    <div className="page-stack">
      <SetupBar project={project} setup={data} tracker={tracker} running={running} onTracker={setTrackerId} onPlaces={() => setPlaces(true)} onAdd={() => setAdding(true)} />
      {data.keywords === 0 ? (
        <div className="oc-card">
          <Empty icon={<TrendingUp size={28} />} title={t("rank.emptyTitle")}>
            <p>{t("rank.emptyText")}</p>
            <Button primary onClick={() => setAdding(true)}>
              <Plus size={15} />
              {t("rank.addKeywords")}
            </Button>
          </Empty>
        </div>
      ) : (
        <>
          {data.missing_volumes > 0 && <VolumesBanner project={project} missing={data.missing_volumes} />}
          {report.isLoading && <Loading label={t("common.loading")} />}
          {report.error && <ErrorState title={t("common.loadFailed")} message={explain(report.error, t).message} retry={() => void report.refetch()} retryLabel={t("common.retry")} />}
          {report.data && <ReportView project={project} report={report.data} days={days} onDays={setDays} onOpen={setDetail} />}
          <Runs project={project} />
        </>
      )}
      {adding && <AddKeywordsDialog project={project} setup={data} onClose={() => setAdding(false)} />}
      {places && <PlacesDialog project={project} setup={data} onClose={() => setPlaces(false)} />}
      {detail && tracker && <KeywordDialog project={project} tracker={tracker} row={detail} onClose={() => setDetail(null)} />}
    </div>
  );
}
