// Local SEO: the project's business on Google Maps — where it shows from every point of a grid
// around it for each keyword, the businesses beating it there, its profile, the businesses of its
// kind around it and its reviews.

import { lazy, Suspense, useMemo, useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { MapPin, Play, Plus, Store, X } from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Segmented, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { GridCell, GridPoint, Job, LocalRival, LocalRun, LocalSchedule, LocalSetup, LocalTrendPoint, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { explain } from "../../lib/errors";
import { formatCredits, formatNumber, formatPercent, formatUsd, relativeTime } from "../../lib/format";
import { useGlossary } from "../../lib/glossary";
import { blockedText, CostLabel } from "../../lib/PaidFrame";
import { useScheduleNote } from "../../lib/sync";
import { tableLabels } from "../../lib/tables";
import { CompetitorsTab, FindBusiness, ProfileTab, Rating } from "./Business";
import { BANDS, bandOf, GRID_SIZES, gridRows, pointLabel, RADII_KM, trendOf } from "./local";
import { ReviewsTab } from "./Reviews";

const GridMap = lazy(() => import("./GridMap"));

const TABS = ["grid", "profile", "competitors", "reviews"] as const;
type Tab = (typeof TABS)[number];

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

const keyOf = (cell: GridPoint) => `${cell.row}-${cell.col}`;

/** The grid as squares, north at the top: drawn when the map cannot be, and beside it on phones. */
function GridSquares({ cells, size, selected, onSelect }: { cells: GridCell[]; size: number; selected: string | null; onSelect: (key: string) => void }) {
  const { t } = useI18n();
  return (
    <div className="grid-squares" style={{ gridTemplateColumns: `repeat(${size}, minmax(0, 1fr))` }} role="grid" aria-label={t("local.gridLabel")}>
      {gridRows(cells, size).map((row, index) =>
        row.map((cell, col) =>
          cell ? (
            <button
              key={`${index}-${col}`}
              type="button"
              className={`grid-square band-${bandOf(cell)}${selected === keyOf(cell) ? " selected" : ""}${index === (size - 1) / 2 && col === (size - 1) / 2 ? " centre" : ""}`}
              onClick={() => onSelect(keyOf(cell))}
              aria-label={cell.position === null ? t("local.pointMissing") : t("local.pointAt", { n: cell.position })}
            >
              {pointLabel(cell)}
            </button>
          ) : (
            <span key={`${index}-${col}`} className="grid-square empty" />
          ),
        ),
      )}
    </div>
  );
}

function Legend() {
  const { t } = useI18n();
  return (
    <div className="grid-legend" aria-label={t("local.legend")}>
      {BANDS.map((band) => (
        <span key={band} className="grid-legend-item">
          <span className={`grid-swatch band-${band}`} aria-hidden="true" />
          {t(`local.band_${band}`)}
        </span>
      ))}
    </div>
  );
}

/** The map with the grid; the squares when the map cannot load. */
function GridView({ cells, planned, size, centre, selected, onSelect }: { cells: GridCell[]; planned: GridPoint[]; size: number; centre: { lat: number; lng: number; title: string }; selected: string | null; onSelect: (key: string) => void }) {
  const { t } = useI18n();
  const [mapFailed, setMapFailed] = useState(false);
  const shown: (GridCell | GridPoint)[] = cells.length ? cells : planned;
  const label = (cell: GridCell | GridPoint) => ("position" in cell ? (cell.position === null ? t("local.pointMissing") : t("local.pointAt", { n: cell.position })) : t("local.pointPlanned"));
  return (
    <div className="grid-view">
      {mapFailed ? (
        cells.length ? (
          <GridSquares cells={cells} size={size} selected={selected} onSelect={onSelect} />
        ) : (
          <p className="faint">{t("local.mapFailed")}</p>
        )
      ) : (
        <Suspense fallback={<div className="grid-map loading" />}>
          <GridMap cells={shown} centre={centre} selected={selected} onSelect={onSelect} onFail={() => setMapFailed(true)} label={label} />
        </Suspense>
      )}
      <Legend />
    </div>
  );
}

/** What a selected point showed: the business's place and the first three there. */
function PointDetail({ cell, business }: { cell: GridCell | undefined; business: string }) {
  const { t } = useI18n();
  if (!cell) return <p className="faint small">{t("local.pickPoint")}</p>;
  return (
    <div className="point-detail">
      <h4>{cell.position === null ? t("local.pointMissing") : t("local.pointAt", { n: cell.position })}</h4>
      {cell.error && <p className="warn small">{cell.error}</p>}
      <ol className="point-top">
        {cell.top.map((place) => (
          <li key={`${place.position}-${place.title}`} className={place.title === business ? "own" : undefined}>
            <span className="point-rank">{place.position}</span>
            <span className="point-name">{place.title}</span>
            <Rating business={place} compact />
          </li>
        ))}
      </ol>
      <span className="faint small">
        {cell.lat.toFixed(4)}, {cell.lng.toFixed(4)}
      </span>
    </div>
  );
}

function TrendChart({ points }: { points: LocalTrendPoint[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 44, right: 52, top: 36, bottom: 28 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: points.map((point) => new Date(point.started_at).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", inverse: true, min: 1, ...axis },
        { type: "value", min: 0, max: 100, ...axis, splitLine: { show: false }, axisLabel: { ...axis.axisLabel, formatter: "{value}%" } },
      ],
      series: [
        { name: t("local.arp"), type: "line", smooth: true, symbolSize: 6, data: points.map((point) => point.arp) },
        { name: t("local.solv"), type: "bar", yAxisIndex: 1, barMaxWidth: 18, itemStyle: { opacity: 0.45 }, data: points.map((point) => point.solv) },
      ],
    };
  }, [points, tokens, t, locale]);
  return <EChart option={option} height={240} />;
}

function Rivals({ rivals }: { rivals: LocalRival[] }) {
  const { t, locale } = useI18n();
  const columns: Column<LocalRival>[] = [
    { key: "title", label: t("local.colBusiness"), render: (row) => <strong className="cell-main">{row.title}</strong>, sort: (row) => row.title },
    { key: "top3", label: t("local.colTop3Points"), render: (row) => formatNumber(row.top3, locale), sort: (row) => row.top3, numeric: true },
    { key: "average", label: t("local.colAverage"), render: (row) => one(row.average, locale), sort: (row) => row.average, numeric: true },
    { key: "rating", label: t("local.colRating"), render: (row) => <Rating business={row} compact />, sort: (row) => row.rating, numeric: true },
  ];
  return <DataTable rows={rivals} columns={columns} rowKey={(row) => row.cid ?? row.title} initialSort={{ key: "top3", direction: "desc" }} empty={t("local.noRivals")} labels={tableLabels(t, locale)} />;
}

function Runs({ runs, current, onPick }: { runs: LocalRun[]; current: number | null; onPick: (id: number) => void }) {
  const { t, locale } = useI18n();
  if (!runs.length) return null;
  const tone = (run: LocalRun) => (run.status === "done" ? "good" : run.status === "running" ? "accent" : run.status === "partial" ? "warning" : "critical");
  const queued = (run: LocalRun) => run.status === "running" && run.waiting > 0;
  return (
    <ul className="run-list">
      {runs.map((run) => (
        <li key={run.id} className={run.id === current ? "current" : undefined}>
          <span className={`oc-badge ${tone(run)}`}>{queued(run) ? t("rank.run_queued") : t(`rank.run_${run.status}`)}</span>
          <button type="button" className="link-button" onClick={() => onPick(run.id)}>
            {relativeTime(run.started_at, locale)}
          </button>
          <span className="faint small">
            {run.scheduled ? t("rank.runScheduled") : t("rank.runManual")} · {t("local.gridSize", { n: run.grid, km: one(run.radius_km, locale) })}
          </span>
          <span className="small">
            {queued(run)
              ? t("rank.runWaiting", { answered: formatNumber(Math.max(0, run.total - run.waiting - run.failed), locale), total: formatNumber(run.total, locale), cost: formatUsd(run.cost_usd, locale) })
              : t("local.runLine", { checked: formatNumber(run.checked, locale), failed: formatNumber(run.failed, locale), cost: formatUsd(run.cost_usd, locale) })}
          </span>
          {run.error && <span className="faint small run-error">{run.error}</span>}
        </li>
      ))}
    </ul>
  );
}

/** Keywords, grid size, radius, schedule, what a run costs and the run itself. */
function GridSetup({ project, setup, running }: { project: ProjectRow; setup: LocalSetup; running: Job | undefined }) {
  const { t, locale } = useI18n();
  const scheduleNote = useScheduleNote();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [keyword, setKeyword] = useState("");
  const estimate = useQuery({ queryKey: ["local", "estimate", project.id, setup.keywords.length, setup.grid, setup.radius_km, setup.schedule], queryFn: () => api.localGridEstimate(project.id) });
  const save = useMutation({
    mutationFn: (patch: Partial<{ keywords: string[]; grid: number; radius_km: number; schedule: LocalSchedule }>) =>
      api.saveLocalSettings(project.id, { keywords: setup.keywords, grid: setup.grid, radius_km: setup.radius_km, schedule: setup.schedule, ...patch }),
    onSuccess: (next) => {
      queryClient.setQueryData(["local", "setup", project.id], next);
      void queryClient.invalidateQueries({ queryKey: ["local", "estimate", project.id] });
    },
    onError: (thrown) => fail(thrown),
  });
  const start = useMutation({
    mutationFn: () => api.startLocalGrid(project.id),
    onSuccess: () => {
      notify(t("local.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const add = (event: FormEvent) => {
    event.preventDefault();
    const added = keyword.split(",").map((entry) => entry.trim()).filter(Boolean);
    if (added.length) save.mutate({ keywords: [...setup.keywords, ...added] });
    setKeyword("");
  };
  const run = estimate.data?.run;
  const blocked = blockedText(run, t);
  const money = (usd: number, credits: number | null) => (run?.via === "credit" && credits !== null ? t("common.credits", { n: formatCredits(credits, locale) }) : formatUsd(usd, locale));
  return (
    <section className="oc-panel local-setup">
      <div className="local-keywords">
        <span className="faint small">{t("local.keywordsLabel")}</span>
        <div className="feature-chips">
          {setup.keywords.map((entry) => (
            <span key={entry} className="oc-chip accent">
              {entry}
              <button type="button" className="chip-x" aria-label={t("local.removeKeyword", { keyword: entry })} onClick={() => save.mutate({ keywords: setup.keywords.filter((known) => known !== entry) })}>
                <X size={12} />
              </button>
            </span>
          ))}
          <form className="inline-add" onSubmit={add}>
            <input className="oc-input" value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder={t("local.keywordPlaceholder")} aria-label={t("local.addKeyword")} />
            <Button type="submit" small className="icon" disabled={!keyword.trim()} aria-label={t("local.addKeyword")}>
              <Plus size={14} />
            </Button>
          </form>
        </div>
      </div>
      <div className="rank-controls">
        <Field label={t("local.gridSizeLabel")}>
          <select className="oc-select" value={setup.grid} disabled={save.isPending} onChange={(event) => save.mutate({ grid: Number(event.target.value) })}>
            {GRID_SIZES.map((size) => (
              <option key={size} value={size}>
                {t("local.gridOption", { n: size, points: size * size })}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t("local.radiusLabel")}>
          <select className="oc-select" value={setup.radius_km} disabled={save.isPending} onChange={(event) => save.mutate({ radius_km: Number(event.target.value) })}>
            {[...new Set([...RADII_KM, setup.radius_km])].sort((a, b) => a - b).map((radius) => (
              <option key={radius} value={radius}>
                {t("local.km", { n: one(radius, locale) })}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t("rank.schedule")} hint={scheduleNote}>
          <Segmented<LocalSchedule>
            label={t("rank.schedule")}
            value={setup.schedule}
            disabled={save.isPending}
            onChange={(schedule) => save.mutate({ schedule })}
            options={[
              { value: "weekly", label: t("rank.scheduleWeekly") },
              { value: "monthly", label: t("local.scheduleMonthly") },
              { value: "manual", label: t("rank.scheduleManual") },
            ]}
          />
        </Field>
      </div>
      <div className="rank-run">
        <div className="rank-run-text">
          <span>{setup.schedule === "manual" || !setup.next_run_at ? t("rank.manualOnly") : t("rank.nextCheck", { time: relativeTime(setup.next_run_at, locale) })}</span>
          {estimate.data && run && !blocked && estimate.data.checks > 0 && (
            <span className="faint small">
              {t("local.runCost", { n: formatNumber(estimate.data.checks, locale) })} <CostLabel estimate={run} />
              {estimate.data.runs_per_month > 0 && (
                <span title={estimate.data.queued ? t("rank.queueHint") : t("rank.queueOwnKeyHint")}>
                  {" · "}
                  {t(estimate.data.queued ? "rank.monthCostQueued" : "rank.monthCost", { cost: money(estimate.data.month_usd, estimate.data.month_credits) })}
                </span>
              )}
            </span>
          )}
        </div>
        <Button primary small busy={start.isPending} disabled={Boolean(blocked) || Boolean(running) || setup.keywords.length === 0} title={blocked ?? undefined} onClick={() => start.mutate()}>
          <Play size={14} />
          {t("local.checkNow")}
        </Button>
      </div>
      {running && (
        <div className="rank-progress" aria-live="polite">
          <div className="oc-progress" aria-hidden="true">
            <div style={{ width: `${running.total ? (running.done / running.total) * 100 : 5}%` }} />
          </div>
          <span className="small">
            {t("local.checking", { done: formatNumber(running.done, locale), total: formatNumber(running.total, locale) })} <span className="faint">{running.step}</span>
          </span>
          <Button small busy={cancel.isPending} onClick={() => cancel.mutate(running.id)}>
            {t("rank.cancel")}
          </Button>
        </div>
      )}
      {blocked && (
        <div className="oc-banner warning" role="status">
          <span>{blocked}</span>
        </div>
      )}
    </section>
  );
}

function GridTab({ project, setup }: { project: ProjectRow; setup: LocalSetup }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { jobs } = useShell();
  const [runId, setRunId] = useState<number | null>(null);
  const [keyword, setKeyword] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const grid = useQuery({
    queryKey: ["local", "grid", project.id, runId],
    queryFn: () => api.localGrid(project.id, runId),
    // While points wait in the queue, the grid is read again now and then.
    refetchInterval: (state) => ((state.state.data?.runs ?? []).some((run) => run.waiting > 0) ? 60_000 : false),
  });
  const running = jobs.find((job) => job.kind === "local-grid" && job.project_id === project.id && job.status === "running");
  const business = setup.business!;
  const data = grid.data;
  const chosen = data?.keywords.find((entry) => entry.keyword === keyword) ?? data?.keywords[0];
  const cells = chosen?.cells ?? [];
  const size = data?.run?.grid ?? setup.grid;
  const summary = chosen?.summary;
  const cell = cells.find((entry) => keyOf(entry) === selected);
  return (
    <div className="page-stack">
      <GridSetup project={project} setup={setup} running={running} />
      {grid.isLoading && <Loading label={t("common.loading")} />}
      {grid.error && <ErrorState title={t("common.loadFailed")} message={explain(grid.error, t).message} retry={() => void grid.refetch()} retryLabel={t("common.retry")} />}
      {data && (
        <>
          {data.keywords.length > 1 && (
            <div className="feature-chips" role="group" aria-label={t("local.keywordsLabel")}>
              {data.keywords.map((entry) => (
                <button key={entry.keyword} type="button" className={`oc-chip chip-button ${entry.keyword === chosen?.keyword ? "accent" : ""}`} aria-pressed={entry.keyword === chosen?.keyword} onClick={() => setKeyword(entry.keyword)}>
                  {entry.keyword}
                  <span className="faint small">{entry.summary.arp === null ? "—" : one(entry.summary.arp, locale)}</span>
                </button>
              ))}
            </div>
          )}
          {summary && (
            <div className="oc-stats rank-stats">
              <Stat label={t("local.arp")} help={t("local.arpHelp")} value={summary.arp === null ? "—" : one(summary.arp, locale)} hint={t("local.foundOf", { n: formatNumber(summary.found, locale), total: formatNumber(summary.points, locale) })} />
              <Stat label={t("local.atrp")} help={t("local.atrpHelp")} value={summary.atrp === null ? "—" : one(summary.atrp, locale)} hint={t("local.atrpHint")} />
              <Stat label={t("local.solv")} help={gloss("share_of_voice")} value={formatPercent(summary.solv, locale)} hint={t("local.top3Of", { n: formatNumber(summary.top3, locale), total: formatNumber(summary.points, locale) })} />
            </div>
          )}
          <section className="oc-panel report">
            <header className="report-head">
              <div className="report-title">
                <h3 className="oc-panel-title">{chosen ? t("local.gridTitle", { keyword: chosen.keyword }) : t("local.gridPlanned")}</h3>
                <span className="faint">{data.run ? t("local.gridHint", { time: relativeTime(data.run.started_at, locale), n: data.run.grid, km: one(data.run.radius_km, locale) }) : t("local.gridPlannedHint")}</span>
              </div>
            </header>
            <div className="grid-layout">
              <GridView cells={cells} planned={setup.points} size={size} centre={{ lat: data.run?.lat ?? business.lat, lng: data.run?.lng ?? business.lng, title: business.title }} selected={selected} onSelect={setSelected} />
              <aside className="sub-panel">
                {cells.length ? <PointDetail cell={cell} business={business.title} /> : <p className="faint small">{t("local.noRunText")}</p>}
              </aside>
            </div>
          </section>
          {chosen && (
            <div className="two-columns">
              <section className="oc-panel report">
                <header className="report-head">
                  <div className="report-title">
                    <h3 className="oc-panel-title">{t("local.rivalsTitle")}</h3>
                    <span className="faint">{t("local.rivalsHint")}</span>
                  </div>
                </header>
                <Rivals rivals={data.rivals} />
              </section>
              <section className="oc-panel report">
                <header className="report-head">
                  <div className="report-title">
                    <h3 className="oc-panel-title">{t("local.trendTitle")}</h3>
                    <span className="faint">{chosen.keyword}</span>
                  </div>
                </header>
                {trendOf(data.trend, chosen.keyword).length > 1 ? <TrendChart points={trendOf(data.trend, chosen.keyword)} /> : <p className="faint">{t("local.trendOne")}</p>}
              </section>
            </div>
          )}
          {data.runs.length > 0 && (
            <section className="oc-panel report">
              <header className="report-head">
                <div className="report-title">
                  <h3 className="oc-panel-title">{t("local.runsTitle")}</h3>
                </div>
              </header>
              <Runs runs={data.runs} current={data.run?.id ?? null} onPick={(id) => setRunId(id)} />
            </section>
          )}
        </>
      )}
    </div>
  );
}

/** The chosen business in a line, with a way to choose another. */
function BusinessBar({ setup, onChange }: { setup: LocalSetup; onChange: () => void }) {
  const { t } = useI18n();
  const business = setup.business!;
  return (
    <div className="oc-card business-bar">
      <span className="oc-tile orange small" aria-hidden="true">
        <Store size={16} />
      </span>
      <div className="business-bar-text">
        <strong>{business.title}</strong>
        <span className="faint small">
          <MapPin size={12} aria-hidden="true" /> {[business.category, business.address].filter(Boolean).join(" · ")}
        </span>
      </div>
      <Rating business={business} />
      <Button small className="ghost" onClick={onChange}>
        {t("local.changeBusiness")}
      </Button>
    </div>
  );
}

export function LocalSeo({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go, params } = useShell();
  const [changing, setChanging] = useState(false);
  const setup = useQuery({ queryKey: ["local", "setup", project.id], queryFn: () => api.localSetup(project.id) });
  const tab: Tab = (TABS as readonly string[]).includes(params.tab ?? "") ? (params.tab as Tab) : "grid";
  const setTab = (next: Tab) => go({ params: { ...params, tab: next } });
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  if (!data.business || changing) {
    return (
      <div className="page-stack">
        {!data.business && (
          <div className="oc-card">
            <Empty icon={<Store size={28} />} title={t("local.emptyTitle")}>
              <p>{t("local.emptyText")}</p>
            </Empty>
          </div>
        )}
        {changing && (
          <Button small className="ghost" onClick={() => setChanging(false)}>
            {t("common.cancel")}
          </Button>
        )}
        <FindBusiness project={project} onDone={() => setChanging(false)} />
      </div>
    );
  }
  const labels: Record<Tab, string> = { grid: t("local.tabGrid"), profile: t("local.tabProfile"), competitors: t("local.tabCompetitors"), reviews: t("local.tabReviews") };
  return (
    <div className="page-stack">
      <BusinessBar setup={data} onChange={() => setChanging(true)} />
      <div className="oc-tabs" role="tablist" aria-label={t("common.navLocal")}>
        {TABS.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => setTab(entry)}>
            {labels[entry]}
          </button>
        ))}
      </div>
      {tab === "grid" && <GridTab project={project} setup={data} />}
      {tab === "profile" && <ProfileTab project={project} />}
      {tab === "competitors" && <CompetitorsTab project={project} radius={data.radius_km} />}
      {tab === "reviews" && <ReviewsTab project={project} setup={data} />}
    </div>
  );
}

