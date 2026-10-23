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
  Flag,
  Globe,
  MapPin,
  Play,
  Plus,
  Search,
  Sparkles,
  Tag,
  Trash2,
  TrendingUp,
  X,
} from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Segmented, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { CompetitorShare, Estimate, FeatureCount, Job, KeywordPlaces, KeywordRank, LandingPage, PlaceRank, ProjectRow, RankReport, RankRun, RankSetup, Schedule, SharePoint, TrackerRow, VisibilityPoint } from "../../api/types";
import { useI18n } from "../../i18n";
import { Entity } from "../../lib/Entity";
import { useGlossary } from "../../lib/glossary";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { useShell } from "../../lib/context";
import { useScheduleNote } from "../../lib/sync";
import { explain } from "../../lib/errors";
import { formatCredits, formatNumber, formatPercent, formatUsd, relativeTime } from "../../lib/format";
import { blockedText, CostLabel, refreshSpending } from "../../lib/PaidFrame";
import { download, fold, toCsv } from "../../lib/report";
import { featureName, shortUrl, tableLabels } from "../../lib/tables";
import { Sparkline } from "../keywords/KeywordTable";
import { ALL_RANKS, BANDS, comparedChange, csvRows, DEPTHS, filterRanks, heights, PERIODS, placeLabel, placesCsvRows, tagsOf, type Band, type RankFilters } from "./rank";
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
  allPlaces,
  running,
  onTracker,
  onAll,
  onPlaces,
  onAdd,
}: {
  project: ProjectRow;
  setup: RankSetup;
  tracker: TrackerRow | null;
  allPlaces: boolean;
  running: Job | undefined;
  onTracker: (id: number) => void;
  onAll: () => void;
  onPlaces: () => void;
  onAdd: () => void;
}) {
  const { t, locale } = useI18n();
  const scheduleNote = useScheduleNote();
  const { notify, go, fail } = useShell();
  const queryClient = useQueryClient();
  const estimate = useQuery({ queryKey: ["rank", "estimate", project.id], queryFn: () => api.rankEstimate(project.id), enabled: setup.keywords > 0 });
  const save = useMutation({
    mutationFn: (next: { schedule: Schedule; depth: number; ai_overview: boolean }) => api.saveRankSettings(project.id, next),
    onSuccess: (next) => {
      queryClient.setQueryData(["rank", "setup", project.id], next);
      void queryClient.invalidateQueries({ queryKey: ["rank", "estimate", project.id] });
    },
    onError: (thrown) => fail(thrown),
  });
  const start = useMutation({
    mutationFn: () => api.startRankCheck(project.id),
    onSuccess: () => {
      notify(t("rank.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
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
        {setup.trackers.length > 1 && (
          <button type="button" className={`oc-chip chip-button ${allPlaces ? "accent" : ""}`} aria-pressed={allPlaces} onClick={onAll}>
            <Globe size={13} aria-hidden="true" />
            {t("rank.allPlaces")}
          </button>
        )}
        {setup.trackers.map((entry) => (
          <button
            key={entry.id}
            type="button"
            className={`oc-chip chip-button ${!allPlaces && entry.id === tracker?.id ? "accent" : ""}`}
            aria-pressed={!allPlaces && entry.id === tracker?.id}
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
        <Field label={t("rank.schedule")} hint={scheduleNote}>
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
              {estimate.data.runs_per_month > 0 && (
                <span title={estimate.data.queued ? t("rank.queueHint") : t("rank.queueOwnKeyHint")}>
                  {" · "}
                  {t(estimate.data.queued ? "rank.monthCostQueued" : "rank.monthCost", { cost: money(estimate.data.month_usd, estimate.data.month_credits, run, t, locale) })}
                </span>
              )}
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
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const estimate = useQuery({ queryKey: ["rank", "volumes", project.id, missing], queryFn: () => api.rankVolumesEstimate(project.id) });
  const fetch = useMutation({
    mutationFn: () => api.fetchRankVolumes(project.id),
    onSuccess: (done) => {
      notify(t("rank.volumesFetched", { n: formatNumber(done.updated, locale) }));
      void queryClient.invalidateQueries({ queryKey: ["rank"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
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

function ShareBars({ shares, traffic }: { shares: CompetitorShare[]; traffic: Record<string, number> }) {
  const { t, locale } = useI18n();
  const most = Math.max(1, ...shares.map((share) => share.visibility));
  return (
    <ol className="bars domains" aria-label={t("rank.sharesTitle")}>
      {shares.map((share) => (
        <li key={share.domain} className={share.own ? "own" : undefined}>
          <span className="bar-label" title={share.domain}>
            <span className="bar-domain">{share.domain}</span>
            <small className="faint">{t("rank.shareCounts", { top10: share.top10, top3: share.top3 })}</small>
            {traffic[share.domain] !== undefined && <small className="faint">{t("rank.visits", { n: formatNumber(traffic[share.domain]!, locale) })}</small>}
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

/** What else the result page shows for the site: AI Overview, snippet, other elements that
 * show it (local pack, images, videos …), competing pages. */
function Signals({ row }: { row: KeywordRank }) {
  const { t } = useI18n();
  const features = row.features
    .filter((feature) => feature !== "organic")
    .map((feature) => featureName(feature, t))
    .join(", ");
  const others = row.owned.filter((feature) => feature !== "ai_overview" && feature !== "featured_snippet");
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
      {others.length > 0 && (
        <span className="signal good" title={t("rank.ownedSignal", { features: others.map((feature) => featureName(feature, t)).join(", ") })}>
          <Flag size={13} aria-hidden="true" />
          {others.length}
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

/** The position on the compared day and the change since. */
function ComparedCell({ row }: { row: KeywordRank }) {
  const { t } = useI18n();
  if (!row.compared) return <span className="faint">—</span>;
  const then = row.compared.position;
  const change = comparedChange(row);
  return (
    <span className="rank-position">
      <span>{then ?? <span className="faint">{t("rank.notRanked")}</span>}</span>
      {change !== null && change !== 0 && (
        <span className={`rank-change ${change > 0 ? "up" : "down"}`}>
          {change > 0 ? <ArrowUp size={12} aria-hidden="true" /> : <ArrowDown size={12} aria-hidden="true" />}
          {Math.abs(change)}
        </span>
      )}
      {then === null && row.position !== null && <span className="oc-chip small good">{t("rank.comparedNew")}</span>}
      {then !== null && row.position === null && <span className="oc-chip small critical">{t("rank.comparedLost")}</span>}
    </span>
  );
}

/** Today's date, `YYYY-MM-DD`. */
const today = () => new Date().toISOString().slice(0, 10);

function KeywordRanks({
  project,
  rows,
  compare,
  onCompare,
  onOpen,
}: {
  project: ProjectRow;
  rows: KeywordRank[];
  compare: string | null;
  onCompare: (day: string | null) => void;
  onOpen: (row: KeywordRank) => void;
}) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { notify, fail } = useShell();
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
    onError: (thrown) => fail(thrown),
  });
  const tag = useMutation({
    mutationFn: (names: string[]) => api.setKeywordTags(project.id, chosen, names),
    onSuccess: refresh,
    onError: (thrown) => fail(thrown),
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
    ...(compare
      ? [
          {
            key: "compared",
            label: t("rank.colCompared", { date: new Date(`${compare}T12:00:00Z`).toLocaleDateString(locale, { day: "numeric", month: "short", year: "numeric" }) }),
            render: (row: KeywordRank) => <ComparedCell row={row} />,
            sort: (row: KeywordRank) => comparedChange(row),
            numeric: true,
          } satisfies Column<KeywordRank>,
        ]
      : []),
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    { key: "traffic", label: t("rank.colTraffic"), help: gloss("traffic"), optional: true, render: (row) => (row.traffic === null ? "—" : formatNumber(row.traffic, locale)), sort: (row) => row.traffic, numeric: true },
    ...project.competitors.map(
      (domain) =>
        ({
          key: `rival-${domain}`,
          label: domain,
          optional: true,
          render: (row: KeywordRank) => {
            const position = row.rivals[domain] ?? null;
            if (position === null) return <span className="faint">—</span>;
            const ahead = row.position === null || position < row.position;
            return <span className={ahead ? "rival-ahead" : undefined} title={ahead ? t("rank.rivalAhead") : undefined}>{position}</span>;
          },
          sort: (row: KeywordRank) => row.rivals[domain] ?? 1000,
          numeric: true,
        }) satisfies Column<KeywordRank>,
    ),
    {
      key: "url",
      label: t("rank.colUrl"),
      render: (row) =>
        row.url ? (
          <Entity kind="url" value={row.url}>
            <span className="faint small">{shortUrl(row.url)}</span>
          </Entity>
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
          <Field label={t("rank.compareWith")}>
            <span className="compare-field">
              <input className="oc-input" type="date" value={compare ?? ""} max={today()} aria-label={t("rank.compareWith")} onChange={(event) => onCompare(event.target.value || null)} />
              {compare && (
                <Button small className="icon ghost" aria-label={t("rank.compareClear")} onClick={() => onCompare(null)}>
                  <X size={14} />
                </Button>
              )}
            </span>
          </Field>
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
                    t("rank.colTraffic"),
                    t("rank.colUrl"),
                    "AI Overview",
                    t("rank.aiCitedColumn"),
                    t("rank.ownedColumn"),
                    t("rank.colChecked"),
                    ...project.competitors,
                    ...(compare ? [t("rank.colCompared", { date: compare })] : []),
                  ],
                  csvRows(shown, project.competitors, Boolean(compare)),
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
  // While checks wait in the queue, the list is read again now and then.
  const runs = useQuery({ queryKey: ["rank", "runs", project.id], queryFn: () => api.rankRuns(project.id), refetchInterval: (query) => (query.state.data?.some((run) => run.waiting > 0) ? 60_000 : false) });
  if (!runs.data?.length) return null;
  const tone = (run: RankRun) => (run.status === "done" ? "good" : run.status === "running" ? "accent" : run.status === "partial" ? "warning" : "critical");
  const queued = (run: RankRun) => run.status === "running" && run.waiting > 0;
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
            <span className={`oc-badge ${tone(run)}`} title={queued(run) ? t("rank.queueHint") : undefined}>
              {queued(run) ? t("rank.run_queued") : t(`rank.run_${run.status}`)}
            </span>
            <span>{relativeTime(run.started_at, locale)}</span>
            <span className="faint small">{run.scheduled ? t("rank.runScheduled") : t("rank.runManual")}</span>
            <span className="small">
              {queued(run)
                ? t("rank.runWaiting", { answered: formatNumber(Math.max(0, run.total - run.waiting - run.failed), locale), total: formatNumber(run.total, locale), cost: formatUsd(run.cost_usd, locale) })
                : t("rank.runLine", { checked: formatNumber(run.checked, locale), failed: formatNumber(run.failed, locale), cost: formatUsd(run.cost_usd, locale) })}
            </span>
            {run.error && <span className="faint small run-error">{run.error}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** The elements of the latest result pages and how often they show the site. */
function FeatureBars({ features }: { features: FeatureCount[] }) {
  const { t, locale } = useI18n();
  if (features.length === 0) return <p className="faint">{t("rank.noFeatures")}</p>;
  const most = Math.max(1, ...features.map((feature) => feature.shown));
  return (
    <ol className="bars feature-bars" aria-label={t("rank.featuresTitle")}>
      {features.map((feature) => (
        <li key={feature.feature}>
          <span className="bar-label">
            <span className="bar-domain">{featureName(feature.feature, t)}</span>
            <small className="faint">{t("rank.featureLine", { owned: formatNumber(feature.owned, locale), shown: formatNumber(feature.shown, locale) })}</small>
          </span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${(feature.shown / most) * 100}%` }} />
            {feature.owned > 0 && <span className="bar-fill owned" style={{ width: `${(feature.owned / most) * 100}%` }} />}
          </span>
          <span className="bar-value">{formatNumber(feature.shown, locale)}</span>
        </li>
      ))}
    </ol>
  );
}

function LandingPages({ pages }: { pages: LandingPage[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const columns: Column<LandingPage>[] = [
    {
      key: "url",
      label: t("rank.colUrl"),
      render: (row) => (
        <Entity kind="url" value={row.url}>
          <span className="cell-url">{shortUrl(row.url)}</span>
        </Entity>
      ),
      sort: (row) => row.url,
    },
    { key: "keywords", label: t("rank.colKeywords"), render: (row) => formatNumber(row.keywords, locale), sort: (row) => row.keywords, numeric: true },
    { key: "best", label: t("rank.colBest"), render: (row) => row.best_position, sort: (row) => row.best_position, numeric: true },
    { key: "traffic", label: t("rank.colVisits"), help: gloss("traffic"), render: (row) => formatNumber(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
  ];
  return (
    <div className="landing-pages">
      <DataTable rows={pages} columns={columns} rowKey={(row) => row.url} initialSort={{ key: "traffic", direction: "desc" }} empty={t("rank.noLandingPages")} labels={tableLabels(t, locale)} />
    </div>
  );
}

function ReportView({
  project,
  report,
  days,
  compare,
  onDays,
  onCompare,
  onOpen,
}: {
  project: ProjectRow;
  report: RankReport;
  days: number;
  compare: string | null;
  onDays: (days: number) => void;
  onCompare: (day: string | null) => void;
  onOpen: (row: KeywordRank) => void;
}) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
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
      <div className="oc-stats rank-stats">
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
        <Stat label={t("rank.topCounts")} help={gloss("top3_top10")} value={`${formatNumber(summary.top3, locale)} / ${formatNumber(summary.top10, locale)}`} hint={t("rank.topCountsHint")} />
        <Stat label={t("rank.aiTitle")} help={gloss("ai_overview")} value={formatNumber(summary.ai_overviews, locale)} hint={t("rank.aiCitedCount", { n: formatNumber(summary.ai_cited, locale) })} />
        <Stat label={t("rank.trafficStat")} help={gloss("traffic")} value={formatNumber(summary.traffic, locale)} hint={t("rank.trafficHint")} />
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
            <ShareBars shares={report.competitors} traffic={report.traffic} />
          </section>
          <section className="sub-panel">
            <h4>{t("rank.shareHistory")}</h4>
            {report.share_history.length > 1 ? <ShareHistory history={report.share_history} shares={report.competitors} /> : <p className="faint">{t("rank.oneDay")}</p>}
          </section>
        </div>
      </section>
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("rank.pagesAndFeatures")}</h3>
            <span className="faint">{t("rank.pagesAndFeaturesHint")}</span>
          </div>
        </header>
        <div className="two-columns">
          <section className="sub-panel">
            <h4>
              {t("rank.featuresTitle")} <span className="faint small">{t("rank.featuresLegend")}</span>
            </h4>
            <FeatureBars features={summary.features} />
          </section>
          <section className="sub-panel">
            <h4>{t("rank.landingTitle")}</h4>
            <LandingPages pages={report.landing_pages} />
          </section>
        </div>
      </section>
      <KeywordRanks project={project} rows={report.rows} compare={compare} onCompare={onCompare} onOpen={onOpen} />
    </div>
  );
}

/** A keyword's position on one place with its change. */
function PlaceCell({ place }: { place: PlaceRank | undefined }) {
  const { t } = useI18n();
  if (!place) return <span className="faint">—</span>;
  if (place.position === null && place.error) {
    return (
      <span className="rank-failed" title={t("rank.checkFailed", { error: place.error })}>
        <AlertTriangle size={14} aria-hidden="true" />
      </span>
    );
  }
  return (
    <span className="rank-position" title={place.url ?? undefined}>
      <strong>{place.position ?? <span className="faint">{t("rank.notRanked")}</span>}</strong>
      {place.change !== null && place.change !== 0 && (
        <span className={`rank-change ${place.change > 0 ? "up" : "down"}`}>
          {place.change > 0 ? <ArrowUp size={12} aria-hidden="true" /> : <ArrowDown size={12} aria-hidden="true" />}
          {Math.abs(place.change)}
        </span>
      )}
    </span>
  );
}

/** Every keyword on every place side by side. */
function PlacesView({ project, days, onDays }: { project: ProjectRow; days: number; onDays: (days: number) => void }) {
  const { t, locale } = useI18n();
  const [query, setQuery] = useState("");
  const places = useQuery({ queryKey: ["rank", "places", project.id, days], queryFn: () => api.rankPlaces(project.id, days) });
  const deviceName = (name: string) => t(`rank.${name}`);
  if (places.isLoading) return <Loading label={t("common.loading")} />;
  if (places.error || !places.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(places.error, t).message} retry={() => void places.refetch()} retryLabel={t("common.retry")} />;
  }
  const { trackers, rows, summaries } = places.data;
  const folded = fold(query.trim());
  const shown = folded ? rows.filter((row) => fold(row.keyword).includes(folded)) : rows;
  const columns: Column<KeywordPlaces>[] = [
    {
      key: "keyword",
      label: t("overview.colKeyword"),
      render: (row) => (
        <Entity kind="keyword" value={row.keyword}>
          <strong className="cell-main">{row.keyword}</strong>
        </Entity>
      ),
      sort: (row) => row.keyword,
    },
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    ...trackers.map(
      (tracker, index) =>
        ({
          key: `place-${tracker.id}`,
          label: placeLabel(tracker, deviceName),
          render: (row: KeywordPlaces) => <PlaceCell place={row.places[index]} />,
          sort: (row: KeywordPlaces) => row.places[index]?.position ?? 1000,
          numeric: true,
        }) satisfies Column<KeywordPlaces>,
    ),
  ];
  return (
    <div className="page-stack">
      <div className="oc-stats">
        {summaries.map((summary) => {
          const tracker = trackers.find((entry) => entry.id === summary.tracker_id);
          return (
            <Stat
              key={summary.tracker_id}
              label={tracker ? placeLabel(tracker, deviceName) : String(summary.tracker_id)}
              value={summary.visibility === null ? "—" : formatPercent(summary.visibility, locale)}
              hint={t("rank.placeSummary", { position: summary.average_position === null ? "—" : one(summary.average_position, locale), n: formatNumber(summary.top10, locale) })}
            />
          );
        })}
      </div>
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("rank.placesTitle")}</h3>
            <span className="faint">{t("rank.placesHint")}</span>
          </div>
          <div className="report-actions">
            <Segmented<string> label={t("rank.days")} value={String(days)} onChange={(value) => onDays(Number(value))} options={PERIODS.map((value) => ({ value: String(value), label: t(`rank.days${value}`) }))} />
          </div>
        </header>
        <div className="page-stack">
          <div className="table-toolbar">
            <label className="filter-search">
              <Search size={15} aria-hidden="true" />
              <span className="sr-only">{t("rank.filterSearch")}</span>
              <input className="oc-input" value={query} placeholder={t("rank.filterSearch")} onChange={(event) => setQuery(event.target.value)} />
            </label>
            <Button
              small
              disabled={shown.length === 0}
              onClick={() =>
                download(
                  `rank-places-${project.domain}.csv`,
                  toCsv(
                    [t("overview.colKeyword"), t("rank.tagsColumn"), t("overview.colVolume"), ...trackers.flatMap((tracker) => [placeLabel(tracker, deviceName), `${placeLabel(tracker, deviceName)} ${t("rank.colChange")}`])],
                    placesCsvRows(shown),
                  ),
                )
              }
            >
              <Download size={14} />
              {t("overview.export")}
            </Button>
          </div>
          <DataTable rows={shown} columns={columns} rowKey={(row) => String(row.keyword_id)} initialSort={{ key: `place-${trackers[0]?.id ?? 0}`, direction: "asc" }} tableId="rank-places" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
        </div>
      </section>
    </div>
  );
}

export function RankTracker({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { jobs } = useShell();
  const setup = useQuery({ queryKey: ["rank", "setup", project.id], queryFn: () => api.rankSetup(project.id) });
  const [trackerId, setTrackerId] = useState<number | null>(null);
  const [allPlaces, setAllPlaces] = useState(false);
  const [days, setDays] = useState(90);
  const [compare, setCompare] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [places, setPlaces] = useState(false);
  const [detail, setDetail] = useState<KeywordRank | null>(null);
  const trackers = setup.data?.trackers ?? [];
  const tracker = trackers.find((entry) => entry.id === trackerId) ?? trackers[0] ?? null;
  const report = useQuery({
    queryKey: ["rank", "report", project.id, tracker?.id ?? null, days, compare],
    queryFn: () => api.rankReport(project.id, tracker?.id ?? null, days, compare),
    enabled: Boolean(setup.data && setup.data.keywords > 0 && !(allPlaces && trackers.length > 1)),
    // Another period or compared day keeps the page in place while it loads.
    placeholderData: (previous) => (previous?.tracker?.id === tracker?.id ? previous : undefined),
  });
  const everyPlace = allPlaces && trackers.length > 1;
  const running = jobs.find((job) => job.kind === "rank-check" && job.project_id === project.id && job.status === "running");
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  return (
    <div className="page-stack">
      <SetupBar
        project={project}
        setup={data}
        tracker={tracker}
        allPlaces={everyPlace}
        running={running}
        onTracker={(id) => {
          setTrackerId(id);
          setAllPlaces(false);
        }}
        onAll={() => setAllPlaces(true)}
        onPlaces={() => setPlaces(true)}
        onAdd={() => setAdding(true)}
      />
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
          {everyPlace ? (
            <PlacesView project={project} days={days} onDays={setDays} />
          ) : (
            <>
              {report.isLoading && <Loading label={t("common.loading")} />}
              {report.error && <ErrorState title={t("common.loadFailed")} message={explain(report.error, t).message} retry={() => void report.refetch()} retryLabel={t("common.retry")} />}
              {report.data && <ReportView project={project} report={report.data} days={days} compare={compare} onDays={setDays} onCompare={setCompare} onOpen={setDetail} />}
            </>
          )}
          <Runs project={project} />
        </>
      )}
      {adding && <AddKeywordsDialog project={project} setup={data} onClose={() => setAdding(false)} />}
      {places && <PlacesDialog project={project} setup={data} onClose={() => setPlaces(false)} />}
      {detail && tracker && <KeywordDialog project={project} tracker={tracker} row={detail} onClose={() => setDetail(null)} />}
    </div>
  );
}
