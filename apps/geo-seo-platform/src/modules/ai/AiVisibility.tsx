import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Check, CircleSlash, Download, Link2, Minus, Play, Plus, Search, Settings2, Sparkles, Trash2, Users } from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Segmented, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type {
  AiEngine,
  AiEstimate,
  AiPromptRow,
  AiReport,
  AiRun,
  AiRunPoint,
  AiSchedule,
  AiSetup,
  BrandShare,
  EngineInfo,
  EngineVisibility,
  Job,
  MentionMetrics,
  MentionRow,
  MentionsReport,
  ProjectRow,
  SourceCount,
} from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatCompact, formatCredits, formatNumber, formatPercent, formatUsd, relativeTime } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { download, toCsv } from "../../lib/report";
import { shortUrl, tableLabels } from "../../lib/tables";
import { cellState, csvRows, ENGINE_NAMES, ENGINES, filterPrompts, PERIODS, platformName, tagsOf, toneOf, type CellState } from "./ai";
import { AddPromptsDialog, AnswerDialog, BrandsDialog } from "./AiDialogs";

type T = (key: string, values?: Record<string, string | number>) => string;

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

/** What a run costs: dollars on own keys, credits on credits. */
function runCost(estimate: AiEstimate, month: boolean, t: T, locale: string): string {
  const factor = month ? estimate.runs_per_month : 1;
  const usd = estimate.engines.filter((engine) => engine.via !== "credit").reduce((sum, engine) => sum + engine.usd, 0) * factor;
  const credits = (month ? estimate.month_credits : estimate.credits) || 0;
  const parts = [];
  if (usd > 0) parts.push(formatUsd(usd, locale));
  if (credits > 0) parts.push(t("common.credits", { n: formatCredits(credits, locale) }));
  return parts.join(" + ") || formatUsd(0, locale);
}

/** An assistant as a switch: on or off, and how it would be asked. */
function EngineChip({ info, busy, onToggle }: { info: EngineInfo; busy: boolean; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      className={`oc-chip chip-button engine-chip ${info.enabled ? "accent" : ""} ${info.via ? "" : "unavailable"}`}
      aria-pressed={info.enabled}
      disabled={busy}
      onClick={onToggle}
      title={info.model ?? undefined}
    >
      {info.enabled ? <Check size={13} aria-hidden="true" /> : <Minus size={13} aria-hidden="true" />}
      <span>{ENGINE_NAMES[info.engine]}</span>
      <small>{info.via ? t(`ai.via_${info.via}`) : t("ai.via_none")}</small>
    </button>
  );
}

/** Assistants, schedule, brands, what a run costs and the run itself. */
function SetupBar({ project, setup, running, onAdd, onBrands }: { project: ProjectRow; setup: AiSetup; running: Job | undefined; onAdd: () => void; onBrands: () => void }) {
  const { t, locale } = useI18n();
  const { notify, go, fail } = useShell();
  const queryClient = useQueryClient();
  const { settings } = setup;
  const estimate = useQuery({ queryKey: ["ai", "estimate", project.id, settings.engines.join(), settings.schedule, setup.prompts.length], queryFn: () => api.aiEstimate(project.id), enabled: setup.prompts.length > 0 });
  const save = useMutation({
    mutationFn: (patch: { schedule?: AiSchedule; engines?: AiEngine[] }) =>
      api.saveAiSettings(project.id, { schedule: settings.schedule, engines: settings.engines, models: settings.models, rivals: settings.rivals, ...patch }),
    onSuccess: (next) => {
      queryClient.setQueryData(["ai", "setup", project.id], next);
      void queryClient.invalidateQueries({ queryKey: ["ai", "estimate", project.id] });
    },
    onError: (thrown) => fail(thrown),
  });
  const start = useMutation({
    mutationFn: () => api.startAiRun(project.id),
    onSuccess: () => {
      notify(t("ai.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const toggle = (engine: AiEngine) => {
    const on = settings.engines.includes(engine);
    const engines = on ? settings.engines.filter((known) => known !== engine) : ENGINES.filter((known) => known === engine || settings.engines.includes(known));
    save.mutate({ engines });
  };
  const data = estimate.data;
  const unavailable = data?.unavailable ?? [];
  const askable = data ? data.engines.length > 0 : setup.engines.some((engine) => engine.enabled && engine.via);
  const next = settings.schedule === "manual" || !setup.next_run_at ? t("ai.manualOnly") : t("ai.nextRun", { time: relativeTime(setup.next_run_at, locale) });
  return (
    <section className="oc-panel rank-setup">
      <div className="rank-places" role="group" aria-label={t("ai.engines")}>
        {setup.engines.map((info) => (
          <EngineChip key={info.engine} info={info} busy={save.isPending} onToggle={() => toggle(info.engine)} />
        ))}
      </div>
      <div className="rank-controls">
        <Field label={t("ai.schedule")}>
          <Segmented<AiSchedule>
            label={t("ai.schedule")}
            value={settings.schedule}
            disabled={save.isPending}
            onChange={(schedule) => save.mutate({ schedule })}
            options={[
              { value: "weekly", label: t("ai.scheduleWeekly") },
              { value: "monthly", label: t("ai.scheduleMonthly") },
              { value: "manual", label: t("ai.scheduleManual") },
            ]}
          />
        </Field>
        <Button small onClick={onBrands}>
          <Users size={14} />
          {t("ai.brandsButton", { n: setup.brands.length - 1 })}
        </Button>
      </div>
      <div className="rank-run">
        <div className="rank-run-text">
          <span>{next}</span>
          {data && data.engines.length > 0 && (
            <span className="faint small">
              {t("ai.runCost", { n: formatNumber(data.answers, locale), cost: runCost(data, false, t, locale) })}
              {data.runs_per_month > 0 && <> · {t("ai.monthCost", { cost: runCost(data, true, t, locale) })}</>}
            </span>
          )}
        </div>
        <Button small onClick={onAdd}>
          <Plus size={14} />
          {t("ai.addPrompts")}
        </Button>
        <Button primary small busy={start.isPending} disabled={Boolean(running) || setup.prompts.length === 0 || !askable} onClick={() => start.mutate()}>
          <Play size={14} />
          {t("ai.askNow")}
        </Button>
      </div>
      {running && (
        <div className="rank-progress" aria-live="polite">
          <div className="oc-progress" aria-hidden="true">
            <div style={{ width: `${running.total ? (running.done / running.total) * 100 : 5}%` }} />
          </div>
          <span className="small">
            {t("ai.asking", { done: formatNumber(running.done, locale), total: formatNumber(running.total, locale) })}{" "}
            <span className="faint">{ENGINE_NAMES[running.step as AiEngine] ?? ""}</span>
          </span>
          <Button small busy={cancel.isPending} onClick={() => cancel.mutate(running.id)}>
            {t("ai.cancel")}
          </Button>
        </div>
      )}
      {setup.prompts.length > 0 && unavailable.length > 0 && (
        <div className="oc-banner warning" role="status">
          <span>{t("ai.unavailable", { names: unavailable.map((engine) => ENGINE_NAMES[engine]).join(", ") })}</span>
          <Button small onClick={() => go({ page: "sources" })}>
            {t("common.goToSources")}
          </Button>
        </div>
      )}
    </section>
  );
}

function EngineBars({ engines }: { engines: EngineVisibility[] }) {
  const { t, locale } = useI18n();
  return (
    <ol className="bars engines" aria-label={t("ai.byEngine")}>
      {engines.map((engine) => (
        <li key={engine.engine}>
          <span className="bar-label">{ENGINE_NAMES[engine.engine]}</span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${engine.mention_rate}%` }} />
          </span>
          <span className="bar-value" title={t("ai.engineLine", { answers: engine.answers, cited: formatPercent(engine.citation_rate, locale) })}>
            {engine.answers ? formatPercent(engine.mention_rate, locale) : "—"}
          </span>
        </li>
      ))}
    </ol>
  );
}

function HistoryChart({ history, engines }: { history: AiRunPoint[]; engines: AiEngine[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 44, right: 16, top: 36, bottom: 28 },
      legend: { top: 0, left: 0, type: "scroll" },
      tooltip: { trigger: "axis", valueFormatter: (value: unknown) => (typeof value === "number" ? formatPercent(value, locale) : "—") },
      xAxis: { type: "category", data: history.map((point) => new Date(point.date).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: { type: "value", min: 0, max: 100, ...axis, axisLabel: { ...axis.axisLabel, formatter: "{value}%" } },
      series: [
        { name: t("ai.allEngines"), type: "line", symbol: "circle", symbolSize: 6, lineStyle: { width: 3 }, areaStyle: { opacity: 0.1 }, data: history.map((point) => point.mention_rate) },
        ...engines.map((engine) => ({
          name: ENGINE_NAMES[engine],
          type: "line",
          symbol: "none",
          connectNulls: true,
          lineStyle: { width: 1.25, type: "dashed", opacity: 0.8 },
          data: history.map((point) => point.engines[engine] ?? null),
        })),
      ],
    };
  }, [history, engines, tokens, t, locale]);
  return <EChart option={option} height={260} />;
}

function BrandBars({ brands }: { brands: BrandShare[] }) {
  const { t, locale } = useI18n();
  const most = Math.max(1, ...brands.map((brand) => brand.share));
  return (
    <ol className="bars domains" aria-label={t("ai.shareTitle")}>
      {brands.map((brand) => (
        <li key={brand.key} className={brand.own ? "own" : undefined}>
          <span className="bar-label" title={brand.key}>
            {brand.key}
          </span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${(brand.share / most) * 100}%` }} />
          </span>
          <span
            className="bar-value"
            title={t("ai.brandLine", { mentions: brand.mentions, citations: brand.citations, position: brand.average_position === null ? "—" : one(brand.average_position, locale) })}
          >
            {formatPercent(brand.share, locale)}
          </span>
        </li>
      ))}
    </ol>
  );
}

function SourceBars({ sources, label }: { sources: SourceCount[]; label: string }) {
  const { locale } = useI18n();
  const most = Math.max(1, ...sources.map((source) => source.answers));
  return (
    <ol className="bars domains" aria-label={label}>
      {sources.map((source) => (
        <li key={source.key} className={source.own ? "own" : undefined}>
          <span className="bar-label" title={source.title ?? source.key}>
            {source.key.startsWith("http") ? (
              <a href={source.key} target="_blank" rel="noreferrer noopener">
                {source.title || shortUrl(source.key)}
              </a>
            ) : (
              source.key
            )}
          </span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${(source.answers / most) * 100}%` }} />
          </span>
          <span className="bar-value">{formatNumber(source.answers, locale)}</span>
        </li>
      ))}
    </ol>
  );
}

const CELL_ICON: Record<CellState, typeof Check> = { cited: Link2, mentioned: Check, rival: Users, absent: CircleSlash, error: AlertTriangle, missing: Minus };

/** A question on one assistant: cited, named (with its place), a rival named instead (with its place), absent, failed or not asked. */
function CellButton({ row, engine, onOpen }: { row: AiPromptRow; engine: AiEngine; onOpen: () => void }) {
  const { t } = useI18n();
  const cell = row.results[engine];
  const state = cellState(cell);
  const Icon = CELL_ICON[state];
  const label =
    state === "error"
      ? t("ai.answerFailed", { error: cell?.error ?? "" })
      : state === "rival" && cell?.rival
        ? t("ai.cellRivalTitle", { rival: cell.rival.key, n: cell.rival.position })
        : cell?.position
          ? t("ai.cellPlaceTitle", { state: t(`ai.cell_${state}`), n: cell.position })
          : t(`ai.cell_${state}`);
  const place = state === "rival" ? cell?.rival?.position : cell?.position;
  return (
    <button type="button" className={`ai-cell ${state}`} onClick={onOpen} disabled={state === "missing"} title={label}>
      <Icon size={14} aria-hidden="true" />
      {place ? <span>{place}.</span> : null}
      <span className="sr-only">{label}</span>
    </button>
  );
}

function PromptMatrix({
  project,
  rows,
  engines,
  onOpen,
  onAdd,
}: {
  project: ProjectRow;
  rows: AiPromptRow[];
  engines: AiEngine[];
  onOpen: (promptId: number, engine: AiEngine) => void;
  onAdd: () => void;
}) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const shown = useMemo(() => filterPrompts(rows, query, tag), [rows, query, tag]);
  const tags = useMemo(() => tagsOf(rows), [rows]);
  const chosen = rows.filter((row) => selected.has(row.prompt_id)).map((row) => row.prompt_id);
  const remove = useMutation({
    mutationFn: () => api.removeAiPrompts(project.id, chosen),
    onSuccess: () => {
      notify(t("ai.removed", { n: chosen.length }));
      setSelected(new Set());
      void queryClient.invalidateQueries({ queryKey: ["ai"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const toggle = (id: number) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const allShown = shown.length > 0 && shown.every((row) => selected.has(row.prompt_id));
  const score = (row: AiPromptRow, engine: AiEngine) => ({ cited: 3, mentioned: 2, absent: 1, rival: 0.5, error: 0, missing: 0 })[cellState(row.results[engine])];
  const columns: Column<AiPromptRow>[] = [
    {
      key: "select",
      label: "",
      width: "36px",
      render: (row) => <input type="checkbox" checked={selected.has(row.prompt_id)} onChange={() => toggle(row.prompt_id)} aria-label={t("ai.select", { prompt: row.prompt })} />,
    },
    {
      key: "prompt",
      label: t("ai.colPrompt"),
      render: (row) => (
        <span className="ai-prompt">
          <span className="cell-main">{row.prompt}</span>
          {row.tags.map((name) => (
            <span key={name} className="oc-chip small">
              {name}
            </span>
          ))}
        </span>
      ),
      sort: (row) => row.prompt,
    },
    ...engines.map(
      (engine): Column<AiPromptRow> => ({
        key: engine,
        label: ENGINE_NAMES[engine],
        lang: "en",
        render: (row) => <CellButton row={row} engine={engine} onOpen={() => onOpen(row.prompt_id, engine)} />,
        sort: (row) => score(row, engine) * 100 - (row.results[engine]?.position ?? 0),
      }),
    ),
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("ai.promptsTitle")}</h3>
          <span className="faint">{t("ai.promptsHint")}</span>
        </div>
        <div className="report-actions ai-legend" aria-hidden="true">
          {(["cited", "mentioned", "rival", "absent", "error"] as const).map((state) => {
            const Icon = CELL_ICON[state];
            return (
              <span key={state} className={`ai-cell ${state} legend`}>
                <Icon size={13} />
                {t(`ai.cell_${state}`)}
              </span>
            );
          })}
        </div>
      </header>
      <div className="page-stack">
        <div className="filters">
          <label className="filter-search">
            <Search size={15} aria-hidden="true" />
            <span className="sr-only">{t("ai.filterSearch")}</span>
            <input className="oc-input" value={query} placeholder={t("ai.filterSearch")} onChange={(event) => setQuery(event.target.value)} />
          </label>
          {tags.length > 0 && (
            <Field label={t("ai.filterTag")}>
              <select className="oc-select" value={tag} onChange={(event) => setTag(event.target.value)}>
                <option value="">{t("ai.allTags")}</option>
                {tags.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </Field>
          )}
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
                    if (allShown) next.delete(row.prompt_id);
                    else next.add(row.prompt_id);
                  }
                  return next;
                })
              }
            />
            <span>{t("ai.selectShown", { n: formatNumber(shown.length, locale) })}</span>
          </label>
          <Button
            small
            disabled={chosen.length === 0}
            busy={remove.isPending}
            onClick={() => {
              if (window.confirm(t("ai.removeConfirm", { n: chosen.length }))) remove.mutate();
            }}
          >
            <Trash2 size={14} />
            {t("ai.removeSelected", { n: chosen.length })}
          </Button>
          <Button small onClick={onAdd}>
            <Plus size={14} />
            {t("ai.addPrompts")}
          </Button>
          <Button
            small
            disabled={shown.length === 0}
            onClick={() =>
              download(
                `ai-visibility-${project.domain}.csv`,
                toCsv([t("ai.colPrompt"), t("ai.tagsColumn"), ...engines.map((engine) => ENGINE_NAMES[engine])], csvRows(shown, engines, (state) => t(`ai.cell_${state}`))),
              )
            }
          >
            <Download size={14} />
            {t("overview.export")}
          </Button>
        </div>
        <DataTable rows={shown} columns={columns} rowKey={(row) => String(row.prompt_id)} initialSort={{ key: "prompt", direction: "asc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      </div>
    </section>
  );
}

function Runs({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const runs = useQuery({ queryKey: ["ai", "runs", project.id], queryFn: () => api.aiRuns(project.id) });
  if (!runs.data?.length) return null;
  const tone = (run: AiRun) => (run.status === "done" ? "good" : run.status === "running" ? "accent" : run.status === "partial" || run.status === "cancelled" ? "warning" : "critical");
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("ai.runsTitle")}</h3>
        </div>
      </header>
      <ul className="run-list">
        {runs.data.map((run) => (
          <li key={run.id}>
            <span className={`oc-badge ${tone(run)}`}>{t(`ai.run_${run.status}`)}</span>
            <span>{relativeTime(run.started_at, locale)}</span>
            <span className="faint small">{run.scheduled ? t("ai.runScheduled") : t("ai.runManual")}</span>
            <span className="small">{t("ai.runLine", { checked: formatNumber(run.checked, locale), failed: formatNumber(run.failed, locale), cost: formatUsd(run.cost_usd, locale) })}</span>
            {run.error && <span className="faint small run-error">{run.error}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

function ReportView({
  project,
  setup,
  report,
  days,
  onDays,
  onOpen,
  onAdd,
}: {
  project: ProjectRow;
  setup: AiSetup;
  report: AiReport;
  days: number;
  onDays: (days: number) => void;
  onOpen: (promptId: number, engine: AiEngine) => void;
  onAdd: () => void;
}) {
  const { t, locale } = useI18n();
  const { summary } = report;
  const shownEngines = ENGINES.filter((engine) => setup.settings.engines.includes(engine) || report.engines.some((entry) => entry.engine === engine && entry.answers > 0));
  const tone = toneOf(summary.sentiment);
  const period = (
    <Segmented<string>
      label={t("ai.days")}
      value={String(days)}
      onChange={(value) => onDays(Number(value))}
      options={PERIODS.map((value) => ({ value: String(value), label: t(`ai.days${value}`) }))}
    />
  );
  return (
    <div className="page-stack">
      {summary.answers === 0 ? (
        <div className="oc-card">
          <Empty icon={<Sparkles size={28} />} title={t("ai.noAnswersTitle")}>
            <p>{t("ai.noAnswersText")}</p>
          </Empty>
        </div>
      ) : (
        <>
          <div className="oc-stats">
            <Stat label={t("ai.mentionRate")} value={formatPercent(summary.mention_rate, locale)} hint={t("ai.mentionRateHint", { mentioned: formatNumber(summary.mentioned, locale), answers: formatNumber(summary.answers, locale) })} />
            <Stat label={t("ai.citationRate")} value={formatPercent(summary.citation_rate, locale)} hint={t("ai.citationRateHint", { n: formatNumber(summary.cited, locale) })} />
            <Stat label={t("ai.shareOfVoice")} value={formatPercent(summary.share_of_voice, locale)} hint={t("ai.shareOfVoiceHint")} />
            <Stat label={t("ai.averagePosition")} value={summary.average_position === null ? "—" : one(summary.average_position, locale)} hint={t("ai.averagePositionHint")} />
            <Stat
              label={t("ai.tone")}
              value={tone ? t(`ai.tone_${tone}`) : "—"}
              hint={t("ai.toneHint")}
              tone={tone === "positive" ? "good" : tone === "negative" ? "bad" : undefined}
            />
          </div>
          <section className="oc-panel report">
            <header className="report-head">
              <div className="report-title">
                <h3 className="oc-panel-title">{t("ai.byEngine")}</h3>
                <span className="faint">{t("ai.byEngineHint")}</span>
              </div>
              <div className="report-actions">{period}</div>
            </header>
            <div className="two-columns">
              <section className="sub-panel">
                <h4>{t("ai.mentionRate")}</h4>
                <EngineBars engines={report.engines} />
              </section>
              <section className="sub-panel">
                <h4>{t("ai.history")}</h4>
                {report.history.length > 1 ? <HistoryChart history={report.history} engines={shownEngines} /> : <p className="faint">{t("ai.oneRun")}</p>}
              </section>
            </div>
          </section>
          <section className="oc-panel report">
            <header className="report-head">
              <div className="report-title">
                <h3 className="oc-panel-title">{t("ai.shareTitle")}</h3>
                <span className="faint">{t("ai.shareHint")}</span>
              </div>
            </header>
            <div className="two-columns">
              <section className="sub-panel">
                <h4>{t("ai.shareOfVoice")}</h4>
                <BrandBars brands={report.brands} />
              </section>
              <section className="sub-panel">
                <h4>{t("ai.topDomains")}</h4>
                {report.top_domains.length ? <SourceBars sources={report.top_domains} label={t("ai.topDomains")} /> : <p className="faint">{t("ai.noSources")}</p>}
                <h4>{t("ai.ownPages")}</h4>
                {report.own_pages.length ? <SourceBars sources={report.own_pages} label={t("ai.ownPages")} /> : <p className="faint small">{t("ai.noOwnPages", { domain: project.domain })}</p>}
              </section>
            </div>
          </section>
        </>
      )}
      <PromptMatrix project={project} rows={report.prompts} engines={shownEngines} onOpen={onOpen} onAdd={onAdd} />
    </div>
  );
}

/** The LLM Mentions database: the site's mentions across AI answers, the questions, its rivals. */
function MentionsPanel({ project, canCompare }: { project: ProjectRow; canCompare: boolean }) {
  const { t, locale } = useI18n();
  const [report, setReport] = useState<MentionsReport>("summary");
  const [limit, setLimit] = useState(100);
  const shown = report === "compare" && !canCompare ? "summary" : report;
  return (
    <PaidFrame<unknown>
      module="ai_mentions"
      ask={{ project_id: project.id, report: shown }}
      title={t("ai.mentionsTitle")}
      hint={t("ai.mentionsHint")}
      limit={shown === "queries" ? limit : undefined}
      onLimit={shown === "queries" ? setLimit : undefined}
      extra={
        <Segmented<MentionsReport>
          label={t("ai.mentionsReport")}
          value={shown}
          onChange={setReport}
          options={[
            { value: "summary", label: t("ai.mentionsSummary") },
            { value: "queries", label: t("ai.mentionsQueries") },
            { value: "compare", label: t("ai.mentionsCompare"), disabled: !canCompare },
          ]}
        />
      }
    >
      {(data) => {
        if (shown === "queries") return <MentionQuestions rows={data as MentionRow[]} />;
        if (shown === "compare") return <MentionCompare rows={data as MentionMetrics[]} own={project.domain} />;
        const metrics = data as MentionMetrics;
        return (
          <div className="page-stack">
            <div className="oc-stats">
              <Stat label={t("ai.mentions")} value={formatNumber(metrics.mentions, locale)} hint={t("ai.mentionsCount")} />
              <Stat label={t("ai.aiVolume")} value={formatCompact(metrics.ai_search_volume, locale)} hint={t("ai.aiVolumeHint")} />
            </div>
            <div className="two-columns">
              <section className="sub-panel">
                <h4>{t("ai.platforms")}</h4>
                <GroupBars groups={metrics.platforms} label={t("ai.platforms")} name={platformName} />
              </section>
              <section className="sub-panel">
                <h4>{t("ai.mentionSources")}</h4>
                {metrics.sources.length ? <GroupBars groups={metrics.sources.slice(0, 12)} label={t("ai.mentionSources")} /> : <p className="faint">{t("ai.noSources")}</p>}
              </section>
            </div>
          </div>
        );
      }}
    </PaidFrame>
  );
}

function GroupBars({ groups, label, name = (key) => key }: { groups: MentionMetrics["platforms"]; label: string; name?: (key: string) => string }) {
  const { locale } = useI18n();
  const most = Math.max(1, ...groups.map((group) => group.mentions));
  return (
    <ol className="bars domains" aria-label={label}>
      {groups.map((group) => (
        <li key={group.key}>
          <span className="bar-label" title={group.key}>
            {name(group.key)}
          </span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${(group.mentions / most) * 100}%` }} />
          </span>
          <span className="bar-value">{formatNumber(group.mentions, locale)}</span>
        </li>
      ))}
    </ol>
  );
}

function MentionQuestions({ rows }: { rows: MentionRow[] }) {
  const { t, locale } = useI18n();
  const columns: Column<MentionRow>[] = [
    {
      key: "question",
      label: t("ai.colQuestion"),
      render: (row) => (
        <span className="mention-question" title={row.answer}>
          <span className="cell-main">{row.question}</span>
        </span>
      ),
      sort: (row) => row.question,
    },
    { key: "platform", label: t("ai.colPlatform"), render: (row) => platformName(row.platform), sort: (row) => row.platform },
    { key: "volume", label: t("ai.aiVolume"), render: (row) => (row.ai_search_volume === null ? "—" : formatNumber(row.ai_search_volume, locale)), sort: (row) => row.ai_search_volume, numeric: true },
    {
      key: "sources",
      label: t("ai.colSources"),
      render: (row) => (
        <span className="faint small" title={row.sources.map((source) => source.url ?? source.domain).join("\n")}>
          {row.sources
            .slice(0, 3)
            .map((source) => source.domain)
            .join(", ")}
          {row.sources.length > 3 ? ` +${row.sources.length - 3}` : ""}
        </span>
      ),
      sort: (row) => row.sources.length,
    },
    { key: "seen", label: t("ai.colSeen"), render: (row) => (row.last_response_at ? <span className="faint small">{relativeTime(row.last_response_at.replace(" ", "T").replace(" +00:00", "Z"), locale)}</span> : "—"), sort: (row) => row.last_response_at },
  ];
  return <DataTable rows={rows} columns={columns} rowKey={(row) => `${row.platform}-${row.question}`} initialSort={{ key: "volume", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />;
}

function MentionCompare({ rows, own }: { rows: MentionMetrics[]; own: string }) {
  const { t, locale } = useI18n();
  const bars = (value: (row: MentionMetrics) => number, format: (value: number) => string, label: string) => {
    const most = Math.max(1, ...rows.map(value));
    return (
      <ol className="bars domains" aria-label={label}>
        {rows.map((row) => (
          <li key={row.target} className={row.target === own ? "own" : undefined}>
            <span className="bar-label" title={row.target}>
              {row.target}
            </span>
            <span className="bar-track" aria-hidden="true">
              <span className="bar-fill" style={{ width: `${(value(row) / most) * 100}%` }} />
            </span>
            <span className="bar-value">{format(value(row))}</span>
          </li>
        ))}
      </ol>
    );
  };
  return (
    <div className="two-columns">
      <section className="sub-panel">
        <h4>{t("ai.mentions")}</h4>
        {bars((row) => row.mentions, (value) => formatNumber(value, locale), t("ai.mentions"))}
      </section>
      <section className="sub-panel">
        <h4>{t("ai.aiVolume")}</h4>
        {bars((row) => row.ai_search_volume, (value) => formatCompact(value, locale), t("ai.aiVolume"))}
      </section>
    </div>
  );
}

export function AiVisibility({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { jobs } = useShell();
  const setup = useQuery({ queryKey: ["ai", "setup", project.id], queryFn: () => api.aiSetup(project.id) });
  const [days, setDays] = useState(90);
  const [adding, setAdding] = useState(false);
  const [brands, setBrands] = useState(false);
  const [answer, setAnswer] = useState<{ promptId: number; engine: AiEngine } | null>(null);
  const prompts = setup.data?.prompts.length ?? 0;
  const report = useQuery({ queryKey: ["ai", "report", project.id, days], queryFn: () => api.aiReport(project.id, days), enabled: prompts > 0 });
  const running = jobs.find((job) => job.kind === "ai-visibility" && job.project_id === project.id && job.status === "running");
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  return (
    <div className="page-stack">
      <SetupBar project={project} setup={data} running={running} onAdd={() => setAdding(true)} onBrands={() => setBrands(true)} />
      {prompts === 0 ? (
        <div className="oc-card">
          <Empty icon={<Sparkles size={28} />} title={t("ai.emptyTitle")}>
            <p>{t("ai.emptyText", { domain: project.domain })}</p>
            <div className="row">
              <Button primary onClick={() => setAdding(true)}>
                <Plus size={15} />
                {t("ai.addPrompts")}
              </Button>
              <Button onClick={() => setBrands(true)}>
                <Settings2 size={15} />
                {t("ai.brandsTitle")}
              </Button>
            </div>
          </Empty>
        </div>
      ) : (
        <>
          {report.isLoading && <Loading label={t("common.loading")} />}
          {report.error && <ErrorState title={t("common.loadFailed")} message={explain(report.error, t).message} retry={() => void report.refetch()} retryLabel={t("common.retry")} />}
          {report.data && (
            <ReportView project={project} setup={data} report={report.data} days={days} onDays={setDays} onOpen={(promptId, engine) => setAnswer({ promptId, engine })} onAdd={() => setAdding(true)} />
          )}
          <Runs project={project} />
        </>
      )}
      <MentionsPanel project={project} canCompare={data.brands.length > 1} />
      {adding && <AddPromptsDialog project={project} setup={data} onClose={() => setAdding(false)} />}
      {brands && <BrandsDialog project={project} setup={data} onClose={() => setBrands(false)} />}
      {answer && <AnswerDialog project={project} promptId={answer.promptId} engine={answer.engine} onClose={() => setAnswer(null)} />}
    </div>
  );
}
