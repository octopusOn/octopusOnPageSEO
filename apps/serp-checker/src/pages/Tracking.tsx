import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Download, LineChart, Minus, Play, Plus, Trash2, X } from "lucide-react";
import { api, errorText, type ExportFormat } from "../api/client";
import {
  engines,
  type Device,
  type Engine,
  type Job,
  type MatchMode,
  type Project,
  type ProjectSpec,
  type RankReport,
  type RankRow,
  type Schedule,
  type Settings,
  type TrackerSpec,
} from "../api/types";
import { EChart, axisStyle, useChartTokens } from "../components/EChart";
import { JobCard } from "../components/JobCard";
import { Button, Empty, ErrorState, Loading, Modal } from "../components/ui";
import { date, number, percent, persist, stored, usd, useI18n, type Key } from "../i18n";

export function Tracking({
  jobs,
  settings,
  notify,
}: {
  jobs: Job[];
  settings?: Settings;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const [selected, setSelected] = useState<number | null>(() => stored("serp.project", null));
  const [creating, setCreating] = useState(false);
  const [confirm, setConfirm] = useState<{ usd: number; searches: number } | null>(null);
  const [deleting, setDeleting] = useState(false);
  const list = projects.data ?? [];
  const project = list.find((item) => item.id === selected) ?? list[0];
  const select = (id: number) => {
    setSelected(id);
    persist("serp.project", id);
  };
  const report = useQuery({
    queryKey: ["rank-report", project?.id],
    queryFn: () => api.rankReport(project!.id),
    enabled: Boolean(project),
  });
  const projectJobs = jobs.filter((job) => job.kind === "track" && job.project_id === project?.id);
  const running = projectJobs.find((job) => job.status === "running" || job.status === "challenge");
  const failed = projectJobs.find((job) => job.status === "failed");

  // Reload the report when a check of this project ends.
  const seen = useRef(new Set<string>());
  useEffect(() => {
    for (const job of jobs.filter((item) => item.kind === "track")) {
      if (job.status === "running" || job.status === "challenge") seen.current.add(job.id);
      else if (seen.current.delete(job.id)) {
        void queryClient.invalidateQueries({ queryKey: ["rank-report", job.project_id] });
        void queryClient.invalidateQueries({ queryKey: ["projects"] });
      }
    }
  }, [jobs, queryClient]);

  const start = useMutation({
    mutationFn: (id: number) => api.track(id),
    onSuccess: () => {
      setConfirm(null);
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const check = async () => {
    if (!project) return;
    try {
      const estimate = await api.estimateTrack(project.id);
      if (settings && estimate.usd > settings.confirm_above_usd) setConfirm({ usd: estimate.usd, searches: estimate.searches });
      else start.mutate(project.id);
    } catch (error) {
      notify(errorText(error), true);
    }
  };
  const remove = useMutation({
    mutationFn: (id: number) => api.deleteProject(id),
    onSuccess: () => {
      setDeleting(false);
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const exportReport = async (format: ExportFormat) => {
    if (!project) return;
    try {
      const rows = await api.exportRankReport(project.id, `${project.name}-ranks`, format);
      if (rows !== null) notify(t("exported", { rows: number(rows, lang, 0) }));
    } catch (error) {
      notify(errorText(error), true);
    }
  };

  if (projects.isLoading) return <Loading />;
  if (projects.isError) return <ErrorState message={errorText(projects.error)} retry={() => void projects.refetch()} />;

  return (
    <div className="tracking">
      {list.length === 0 ? (
        <section className="oc-card empty-project">
          <Empty icon={<LineChart size={28} />}>
            <p>{t("noProjects")}</p>
            <Button primary onClick={() => setCreating(true)}>
              <Plus size={16} />
              {t("newProject")}
            </Button>
          </Empty>
        </section>
      ) : (
        project && (
          <>
            <div className="project-bar">
              <label className="oc-field project-picker">
                <span>{t("project")}</span>
                <select className="oc-select" value={project.id} onChange={(event) => select(Number(event.target.value))}>
                  {list.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name} · {item.target.value}
                    </option>
                  ))}
                </select>
              </label>
              <div className="project-meta">
                <span className="oc-chip">{t(`schedule_${project.schedule}` as Key)}</span>
                {project.trackers.map((tracker) => (
                  <span className="oc-chip" key={tracker.id}>
                    {t(tracker.engine)} · {tracker.country.toUpperCase()}-{tracker.language} · {t(tracker.device ?? "desktop")} · {tracker.depth}
                  </span>
                ))}
                <span className="faint">
                  {project.last_run_at ? t("lastChecked", { date: date(project.last_run_at, lang) }) : t("neverChecked")}
                </span>
              </div>
              <div className="project-actions">
                <Button small onClick={() => setCreating(true)}>
                  <Plus size={14} />
                  {t("newProject")}
                </Button>
                <ExportMenu onExport={(format) => void exportReport(format)} />
                <Button small className="icon ghost" aria-label={t("deleteProject")} onClick={() => setDeleting(true)}>
                  <Trash2 size={15} />
                </Button>
                <Button primary small busy={start.isPending} disabled={Boolean(running) || project.keywords === 0} onClick={() => void check()}>
                  <Play size={14} />
                  {t("checkNow")}
                </Button>
              </div>
            </div>
            {(running ?? failed) && (
              <div className="jobs">
                <JobCard
                  job={(running ?? failed)!}
                  onStop={(id) => void api.cancel(id)}
                  onDismiss={() => void api.clearJobs().then(() => queryClient.invalidateQueries({ queryKey: ["jobs"] }))}
                />
              </div>
            )}
            {report.isLoading && <Loading />}
            {report.isError && <ErrorState message={errorText(report.error)} retry={() => void report.refetch()} />}
            {report.data && <Report report={report.data} notify={notify} />}
          </>
        )
      )}

      {creating && (
        <ProjectDialog
          settings={settings}
          onClose={() => setCreating(false)}
          onCreated={(created) => {
            setCreating(false);
            select(created.id);
            void queryClient.invalidateQueries({ queryKey: ["projects"] });
          }}
          notify={notify}
        />
      )}
      {confirm && project && (
        <Modal
          title={t("checkNow")}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <Button onClick={() => setConfirm(null)}>{t("cancel")}</Button>
              <Button primary busy={start.isPending} onClick={() => start.mutate(project.id)}>
                {t("checkNow")}
              </Button>
            </>
          }
        >
          <p>{t("trackConfirm", { searches: number(confirm.searches, lang, 0), usd: usd(confirm.usd, lang) })}</p>
        </Modal>
      )}
      {deleting && project && (
        <Modal
          title={t("deleteProject")}
          onClose={() => setDeleting(false)}
          footer={
            <>
              <Button onClick={() => setDeleting(false)}>{t("cancel")}</Button>
              <Button className="danger solid" busy={remove.isPending} onClick={() => remove.mutate(project.id)}>
                {t("deleteProject")}
              </Button>
            </>
          }
        >
          <p>{t("deleteProjectConfirm", { name: project.name })}</p>
        </Modal>
      )}
    </div>
  );
}

export function ExportMenu({ onExport }: { onExport: (format: ExportFormat) => void }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <div className="export-menu-wrap">
      <Button small onClick={() => setOpen(!open)} aria-expanded={open}>
        <Download size={14} />
        {t("export")}
      </Button>
      {open && (
        <>
          <button className="menu-dismiss" aria-label={t("close")} onClick={() => setOpen(false)} />
          <div className="oc-menu export-menu" role="menu">
            {(["xlsx", "csv", "json"] as const).map((format) => (
              <button
                key={format}
                role="menuitem"
                className="oc-menu-item"
                onClick={() => {
                  setOpen(false);
                  onExport(format);
                }}
              >
                {format === "xlsx" ? "Excel (.xlsx)" : format.toUpperCase()}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function Report({ report, notify }: { report: RankReport; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const latest = report.visibility.at(-1);
  const previous = report.visibility.at(-2);
  return (
    <>
      <div className="oc-stats rank-stats">
        <div className="oc-stat">
          <span className="oc-stat-label">{t("visibility")}</span>
          <span className="oc-stat-value">{latest ? percent(latest.visibility / 100, lang, 1) : "—"}</span>
          {latest && previous && (
            <span className="oc-stat-hint">
              <Delta value={latest.visibility - previous.visibility} digits={1} />
            </span>
          )}
        </div>
        <div className="oc-stat">
          <span className="oc-stat-label">{t("top3")}</span>
          <span className="oc-stat-value">{latest ? number(latest.top3, lang, 0) : "—"}</span>
        </div>
        <div className="oc-stat">
          <span className="oc-stat-label">{t("top10")}</span>
          <span className="oc-stat-value">{latest ? number(latest.top10, lang, 0) : "—"}</span>
        </div>
        <div className="oc-stat">
          <span className="oc-stat-label">{t("rankedKeywords")}</span>
          <span className="oc-stat-value">
            {latest ? number(latest.ranked, lang, 0) : "—"}
            <small> / {number(report.project.keywords, lang, 0)}</small>
          </span>
        </div>
      </div>
      <div className="two-columns wide-left">
        <VisibilityChart report={report} />
        <Competitors report={report} notify={notify} />
      </div>
      <Keywords report={report} notify={notify} />
    </>
  );
}

function Delta({ value, digits = 0 }: { value: number | null | undefined; digits?: number }) {
  const { lang } = useI18n();
  if (value === null || value === undefined) return <span className="delta">—</span>;
  if (Math.abs(value) < 10 ** -digits / 2)
    return (
      <span className="delta same">
        <Minus size={12} />
      </span>
    );
  return (
    <span className={`delta ${value > 0 ? "up" : "down"}`}>
      {value > 0 ? <ArrowUp size={12} /> : <ArrowDown size={12} />}
      {number(Math.abs(value), lang, digits)}
    </span>
  );
}

function VisibilityChart({ report }: { report: RankReport }) {
  const { t, lang } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo(
    () => ({
      grid: { left: 44, right: 44, top: 30, bottom: 28 },
      legend: { top: 0, right: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: report.visibility.map((point) => point.date.slice(5)), ...axisStyle(tokens), splitLine: { show: false } },
      yAxis: [
        { type: "value", min: 0, ...axisStyle(tokens), axisLabel: { ...axisStyle(tokens).axisLabel, formatter: (value: number) => percent(value / 100, lang) } },
        { type: "value", min: 0, minInterval: 1, ...axisStyle(tokens), splitLine: { show: false } },
      ],
      series: [
        {
          name: t("visibility"),
          type: "line",
          smooth: true,
          showSymbol: false,
          areaStyle: { opacity: 0.12 },
          data: report.visibility.map((point) => point.visibility),
        },
        { name: t("top10"), type: "bar", yAxisIndex: 1, barMaxWidth: 10, itemStyle: { opacity: 0.55 }, data: report.visibility.map((point) => point.top10) },
      ],
    }),
    [report.visibility, tokens, t, lang],
  );
  return (
    <section className="oc-panel">
      <div className="oc-panel-head">
        <h2>{t("visibilityTrend")}</h2>
      </div>
      {report.visibility.length === 0 ? <p className="panel-lead">{t("noChecksYet")}</p> : <EChart option={option} height={240} />}
    </section>
  );
}

function Competitors({ report, notify }: { report: RankReport; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(report.project.competitors.join("\n"));
  const save = useMutation({
    mutationFn: () =>
      api.setCompetitors(
        report.project.id,
        text
          .split(/[\n,]/)
          .map((domain) => domain.trim())
          .filter(Boolean),
      ),
    onSuccess: () => {
      setEditing(false);
      void queryClient.invalidateQueries({ queryKey: ["rank-report", report.project.id] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const top = Math.max(1, ...report.competitors.map((share) => share.visibility));
  return (
    <section className="oc-panel competitors">
      <div className="oc-panel-head row between">
        <h2>{t("shareOfVoice")}</h2>
        <Button small className="ghost" onClick={() => setEditing(!editing)}>
          {t("editCompetitors")}
        </Button>
      </div>
      {editing ? (
        <div className="competitor-editor">
          <textarea className="oc-textarea" rows={5} value={text} placeholder="rakip.com" onChange={(event) => setText(event.target.value)} />
          <p className="oc-field-hint">{t("competitorsHint")}</p>
          <div className="row end">
            <Button small onClick={() => setEditing(false)}>
              {t("cancel")}
            </Button>
            <Button small primary busy={save.isPending} onClick={() => save.mutate()}>
              {t("save")}
            </Button>
          </div>
        </div>
      ) : report.competitors.length === 0 ? (
        <p className="panel-lead">{t("noChecksYet")}</p>
      ) : (
        <ul className="share-list">
          {report.competitors.map((share) => (
            <li key={share.domain} className={share.own ? "own" : ""}>
              <div className="row between">
                <b>{share.domain}</b>
                <span className="oc-num">{percent(share.visibility / 100, lang, 1)}</span>
              </div>
              <div className="share-bar" aria-hidden="true">
                <div style={{ width: `${(share.visibility / top) * 100}%` }} />
              </div>
              <small className="faint">
                {t("shareDetail", {
                  keywords: number(share.keywords, lang, 0),
                  position: share.average_position ? number(share.average_position, lang) : "—",
                })}
              </small>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function Sparkline({ history }: { history: [string, number | null][] }) {
  const points = history.slice(-30);
  if (points.length < 2) return null;
  const width = 96;
  const height = 26;
  const worst = Math.max(10, ...points.map(([, position]) => position ?? 0));
  const x = (index: number) => (index / (points.length - 1)) * (width - 4) + 2;
  const y = (position: number) => ((position - 1) / Math.max(1, worst - 1)) * (height - 4) + 2;
  const segments: string[] = [];
  let current = "";
  points.forEach(([, position], index) => {
    if (position === null) {
      if (current) segments.push(current);
      current = "";
    } else current += `${current ? " L" : "M"}${x(index).toFixed(1)},${y(position).toFixed(1)}`;
  });
  if (current) segments.push(current);
  return (
    <svg className="sparkline" width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
      {segments.map((path) => (
        <path key={path} d={path} />
      ))}
    </svg>
  );
}

function Keywords({ report, notify }: { report: RankReport; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState("");
  const [tags, setTags] = useState("");
  const [filter, setFilter] = useState("");
  const add = useMutation({
    mutationFn: () =>
      api.addKeywords(
        report.project.id,
        text
          .split("\n")
          .map((keyword) => keyword.trim())
          .filter(Boolean),
        tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
      ),
    onSuccess: (added) => {
      setText("");
      setAdding(false);
      notify(t("keywordsAdded", { count: number(added, lang, 0) }));
      void queryClient.invalidateQueries({ queryKey: ["rank-report", report.project.id] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const remove = useMutation({
    mutationFn: (keywordId: number) => api.removeKeyword(report.project.id, keywordId),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["rank-report", report.project.id] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const tracker = (row: RankRow) => report.project.trackers.find((item) => item.id === row.tracker_id);
  const needle = filter.trim().toLocaleLowerCase(lang);
  const rows = report.rows.filter(
    (row) => !needle || row.keyword.toLocaleLowerCase(lang).includes(needle) || row.tags.some((tag) => tag.toLocaleLowerCase(lang).includes(needle)),
  );
  const multipleTrackers = report.project.trackers.length > 1;
  return (
    <section className="oc-panel keywords">
      <div className="oc-panel-head row between">
        <h2>
          {t("keywords")} <span className="faint oc-num">{number(report.project.keywords, lang, 0)}</span>
        </h2>
        <div className="row">
          <input className="oc-input small-input" placeholder={t("filterKeywords")} value={filter} onChange={(event) => setFilter(event.target.value)} />
          <Button small onClick={() => setAdding(!adding)}>
            <Plus size={14} />
            {t("addKeywords")}
          </Button>
        </div>
      </div>
      {adding && (
        <div className="keyword-editor">
          <textarea
            className="oc-textarea"
            rows={5}
            value={text}
            placeholder={t("keywordsPlaceholder")}
            onChange={(event) => setText(event.target.value)}
          />
          <div className="row">
            <input className="oc-input" placeholder={t("tagsPlaceholder")} value={tags} onChange={(event) => setTags(event.target.value)} />
            <Button small onClick={() => setAdding(false)}>
              {t("cancel")}
            </Button>
            <Button small primary busy={add.isPending} disabled={!text.trim()} onClick={() => add.mutate()}>
              {t("addKeywords")}
            </Button>
          </div>
        </div>
      )}
      {report.rows.length === 0 ? (
        <p className="panel-lead">{t("noKeywords")}</p>
      ) : (
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t("keyword")}</th>
                {multipleTrackers && <th>{t("engine")}</th>}
                <th className="num">{t("position")}</th>
                <th className="num">{t("change")}</th>
                <th className="num">{t("best")}</th>
                <th>{t("trend")}</th>
                <th>{t("rankingUrl")}</th>
                <th className="num">{t("volume")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => {
                const where = tracker(row);
                return (
                  <tr key={`${row.keyword_id}-${row.tracker_id}`}>
                    <td>
                      <b>{row.keyword}</b>
                      <div className="row-tags">
                        {row.tags.map((tag) => (
                          <span className="oc-chip" key={tag}>
                            {tag}
                          </span>
                        ))}
                        {row.cannibalised && (
                          <span className="oc-badge warning" title={t("cannibalisedHelp")}>
                            {t("cannibalised")}
                          </span>
                        )}
                      </div>
                    </td>
                    {multipleTrackers && (
                      <td className="faint">{where ? `${t(where.engine)} · ${where.country.toUpperCase()} · ${t(where.device ?? "desktop")}` : ""}</td>
                    )}
                    <td className="num oc-num position" title={row.error ?? undefined}>
                      {row.position ?? (row.error ? <span className="critical">!</span> : <span className="faint">{t("notRankingShort")}</span>)}
                    </td>
                    <td className="num">
                      <Delta value={row.change} />
                    </td>
                    <td className="num oc-num">{row.best ?? "—"}</td>
                    <td>
                      <Sparkline history={row.history} />
                    </td>
                    <td className="url-cell">
                      {row.url ? (
                        <a href={row.url} target="_blank" rel="noreferrer" title={row.url}>
                          {row.url.replace(/^https?:\/\/(www\.)?/, "")}
                        </a>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td className="num oc-num">{row.volume !== null && row.volume !== undefined ? number(row.volume, lang, 0) : "—"}</td>
                    <td className="num">
                      <Button small className="icon ghost" aria-label={t("removeKeyword")} onClick={() => remove.mutate(row.keyword_id)}>
                        <X size={14} />
                      </Button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

const blankTracker = (settings?: Settings): TrackerSpec => ({
  engine: "google",
  country: settings?.defaults.country ?? "tr",
  language: settings?.defaults.language ?? "tr",
  device: settings?.defaults.device ?? "desktop",
  depth: 20,
});

function ProjectDialog({
  settings,
  onClose,
  onCreated,
  notify,
}: {
  settings?: Settings;
  onClose: () => void;
  onCreated: (project: Project) => void;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [site, setSite] = useState("");
  const [mode, setMode] = useState<MatchMode>("domain");
  const [schedule, setSchedule] = useState<Schedule>("daily");
  const [trackers, setTrackers] = useState<TrackerSpec[]>([blankTracker(settings)]);
  const create = useMutation({
    mutationFn: (spec: ProjectSpec) => api.createProject(spec),
    onSuccess: onCreated,
    onError: (error) => notify(errorText(error), true),
  });
  const setTracker = (index: number, change: Partial<TrackerSpec>) =>
    setTrackers((old) => old.map((tracker, position) => (position === index ? { ...tracker, ...change } : tracker)));
  const valid = name.trim() && site.trim() && trackers.length > 0;
  return (
    <Modal
      title={t("newProject")}
      className="project-modal"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button
            primary
            busy={create.isPending}
            disabled={!valid}
            onClick={() => create.mutate({ name: name.trim(), target: { value: site.trim(), mode }, schedule, trackers })}
          >
            {t("createProject")}
          </Button>
        </>
      }
    >
      <div className="field-row">
        <label className="oc-field">
          <span>{t("projectName")}</span>
          <input className="oc-input" value={name} onChange={(event) => setName(event.target.value)} />
        </label>
        <label className="oc-field">
          <span>{t("trackedSite")}</span>
          <input className="oc-input" value={site} placeholder={t("yourSitePlaceholder")} onChange={(event) => setSite(event.target.value)} />
        </label>
      </div>
      <div className="field-row">
        <label className="oc-field">
          <span>{t("matchMode")}</span>
          <select className="oc-select" value={mode} onChange={(event) => setMode(event.target.value as MatchMode)}>
            <option value="domain">{t("match_domain")}</option>
            <option value="host">{t("match_host")}</option>
            <option value="prefix">{t("match_prefix")}</option>
          </select>
        </label>
        <label className="oc-field">
          <span>{t("schedule")}</span>
          <select className="oc-select" value={schedule} onChange={(event) => setSchedule(event.target.value as Schedule)}>
            <option value="daily">{t("schedule_daily")}</option>
            <option value="weekly">{t("schedule_weekly")}</option>
            <option value="manual">{t("schedule_manual")}</option>
          </select>
        </label>
      </div>
      <p className="oc-field-hint">{t("scheduleHint")}</p>
      <div className="trackers">
        <span className="type-label">{t("trackers")}</span>
        {trackers.map((tracker, index) => (
          <div className="tracker-row" key={index}>
            <select className="oc-select" aria-label={t("engine")} value={tracker.engine} onChange={(event) => setTracker(index, { engine: event.target.value as Engine })}>
              {engines.map((engine) => (
                <option key={engine} value={engine}>
                  {t(engine)}
                </option>
              ))}
            </select>
            <input className="oc-input" aria-label={t("country")} maxLength={2} value={tracker.country} onChange={(event) => setTracker(index, { country: event.target.value.toLowerCase() })} />
            <input className="oc-input" aria-label={t("languageCode")} maxLength={2} value={tracker.language} onChange={(event) => setTracker(index, { language: event.target.value.toLowerCase() })} />
            <select className="oc-select" aria-label={t("device")} value={tracker.device} onChange={(event) => setTracker(index, { device: event.target.value as Device })}>
              <option value="desktop">{t("desktop")}</option>
              <option value="mobile">{t("mobile")}</option>
            </select>
            <select className="oc-select" aria-label={t("depth")} value={tracker.depth} onChange={(event) => setTracker(index, { depth: Number(event.target.value) })}>
              {[10, 20, 30, 50, 100].map((depth) => (
                <option key={depth} value={depth}>
                  {depth}
                </option>
              ))}
            </select>
            <Button
              small
              className="icon ghost"
              aria-label={t("remove")}
              disabled={trackers.length === 1}
              onClick={() => setTrackers((old) => old.filter((_, position) => position !== index))}
            >
              <X size={14} />
            </Button>
          </div>
        ))}
        <Button small className="ghost" onClick={() => setTrackers((old) => [...old, blankTracker(settings)])}>
          <Plus size={14} />
          {t("addTracker")}
        </Button>
      </div>
    </Modal>
  );
}
