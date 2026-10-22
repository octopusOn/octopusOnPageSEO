import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Bot, CheckCircle2, ChevronLeft, ChevronRight, CircleAlert, CircleX, ExternalLink, Play, ScanSearch, Search, Trash2 } from "lucide-react";
import { Button, Empty, ErrorState, Field, Loading, Modal, Segmented, Stat } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AuditReport, AuditSchedule, AuditSetup, GeoReadiness, GeoSignal, IssueKind, IssueReport, Job, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { useScheduleNote } from "../../lib/sync";
import { explain } from "../../lib/errors";
import { formatCredits, formatNumber, formatPercent, relativeTime } from "../../lib/format";
import { refreshSpending } from "../../lib/PaidFrame";
import { ALL_ISSUES, changeOf, creditsPerThousand, filterCategories, KINDS, pageChoices, scoreChange, totals, type IssueFilters } from "./audit";

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

/** Schedule, page limit, what crawling costs, and the audit itself. */
function SetupBar({ project, setup, running }: { project: ProjectRow; setup: AuditSetup; running: Job | undefined }) {
  const { t, locale } = useI18n();
  const scheduleNote = useScheduleNote();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const save = useMutation({
    mutationFn: (next: { schedule: AuditSchedule; maxPages: number }) => api.saveAuditSettings(project.id, next.schedule, next.maxPages),
    onSuccess: (next) => queryClient.setQueryData(["audit", "setup", project.id], next),
    onError: (thrown) => fail(thrown),
  });
  const start = useMutation({
    mutationFn: () => api.startAudit(project.id),
    onSuccess: () => {
      notify(t("audit.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      void queryClient.invalidateQueries({ queryKey: ["audit"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const { settings, meter } = setup;
  const next =
    settings.schedule === "manual" || !setup.next_audit_at ? t("audit.manualOnly") : t("audit.nextAudit", { time: relativeTime(setup.next_audit_at, locale) });
  const choices = pageChoices(setup.max_pages_limit);
  if (!choices.includes(settings.max_pages)) choices.push(settings.max_pages);
  choices.sort((left, right) => left - right);
  return (
    <section className="oc-panel rank-setup">
      <div className="rank-controls">
        <Field label={t("audit.schedule")} hint={scheduleNote}>
          <Segmented<AuditSchedule>
            label={t("audit.schedule")}
            value={settings.schedule}
            disabled={save.isPending}
            onChange={(schedule) => save.mutate({ schedule, maxPages: settings.max_pages })}
            options={[
              { value: "weekly", label: t("audit.scheduleWeekly") },
              { value: "monthly", label: t("audit.scheduleMonthly") },
              { value: "manual", label: t("audit.scheduleManual") },
            ]}
          />
        </Field>
        <Field label={t("audit.maxPages")}>
          <select className="oc-select" value={settings.max_pages} disabled={save.isPending} onChange={(event) => save.mutate({ schedule: settings.schedule, maxPages: Number(event.target.value) })}>
            {choices.map((pages) => (
              <option key={pages} value={pages}>
                {t("audit.pagesOption", { n: formatNumber(pages, locale) })}
              </option>
            ))}
          </select>
        </Field>
      </div>
      <div className="rank-run">
        <div className="rank-run-text">
          <span>{next}</span>
          <span className="faint small">
            {meter
              ? t("audit.meterText", { free: formatNumber(meter.free_per_month, locale), credits: formatCredits(creditsPerThousand(meter), locale) })
              : t("audit.localText")}
          </span>
        </div>
        <Button primary small busy={start.isPending} disabled={Boolean(running)} onClick={() => start.mutate()}>
          <Play size={14} />
          {t("audit.start")}
        </Button>
      </div>
      {running && (
        <div className="rank-progress" aria-live="polite">
          <div className={`oc-progress ${running.step === "waiting" ? "indeterminate" : ""}`} aria-hidden="true">
            <div style={running.step === "waiting" ? undefined : { width: `${running.total ? Math.min(100, (running.done / running.total) * 100) : 5}%` }} />
          </div>
          <span className="small">
            {running.step === "waiting"
              ? t("audit.waiting")
              : running.step === "finalizing"
                ? t("audit.finalizing")
                : t("audit.crawling", { done: formatNumber(running.done, locale), total: formatNumber(running.total, locale) })}
          </span>
          <Button small busy={cancel.isPending} onClick={() => cancel.mutate(running.id)}>
            {t("audit.stop")}
          </Button>
        </div>
      )}
    </section>
  );
}

function Change({ value, invert = false }: { value: number | null; invert?: boolean }) {
  const { locale } = useI18n();
  if (value === null || value === 0) return null;
  const better = invert ? value < 0 : value > 0;
  return (
    <span className={`rank-change ${better ? "up" : "down"}`}>
      {value > 0 ? <ArrowUp size={12} aria-hidden="true" /> : <ArrowDown size={12} aria-hidden="true" />}
      {one(Math.abs(value), locale)}
    </span>
  );
}

const STATUS_ICON = { good: CheckCircle2, warning: CircleAlert, poor: CircleX } as const;

function SignalRow({ signal }: { signal: GeoSignal }) {
  const { t, locale } = useI18n();
  const Icon = STATUS_ICON[signal.status];
  return (
    <li className={`geo-signal ${signal.status}`}>
      <Icon size={18} aria-hidden="true" />
      <div className="geo-signal-text">
        <strong>{t(`audit.geo_${signal.id.replace(/-/g, "_")}`)}</strong>
        <span className="faint small">{t(`audit.geoHint_${signal.id.replace(/-/g, "_")}`)}</span>
        {signal.examples.length > 0 && (
          <details>
            <summary className="small">{t("audit.geoExamples", { n: formatNumber(signal.total - signal.passed, locale) })}</summary>
            <ul className="geo-examples">
              {signal.examples.map((example) => (
                <li key={example}>
                  {example.startsWith("http") ? (
                    <a href={example} target="_blank" rel="noreferrer noopener">
                      {example}
                    </a>
                  ) : (
                    example
                  )}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>
      <div className="geo-signal-value">
        <span className="bar-track" aria-hidden="true">
          <span className="bar-fill" style={{ width: `${signal.value * 100}%` }} />
        </span>
        <span className="small">
          {formatNumber(signal.passed, locale)}/{formatNumber(signal.total, locale)}
        </span>
      </div>
    </li>
  );
}

function GeoPanel({ geo }: { geo: GeoReadiness }) {
  const { t } = useI18n();
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("audit.geoTitle")}</h3>
          <span className="faint">{t("audit.geoHint")}</span>
        </div>
      </header>
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("audit.geoSignals")}</h4>
          <ul className="geo-signals">
            {geo.signals.map((signal) => (
              <SignalRow key={signal.id} signal={signal} />
            ))}
          </ul>
        </section>
        <section className="sub-panel">
          <h4>{t("audit.crawlersTitle")}</h4>
          {geo.robots === "unreachable" && <p className="oc-banner warning small">{t("audit.robotsUnreachable")}</p>}
          {geo.robots === "missing" && <p className="faint small">{t("audit.robotsMissing")}</p>}
          <ul className="crawler-list">
            {geo.crawlers.map((crawler) => (
              <li key={crawler.id} title={crawler.rule ?? undefined}>
                <Bot size={15} aria-hidden="true" />
                <span>
                  <strong>{crawler.name}</strong>
                  <small className="faint">
                    {crawler.operator} · {t(`audit.purpose_${crawler.purpose}`)}
                  </small>
                </span>
                <span className={`oc-badge ${crawler.allowed ? (crawler.purpose === "training" ? "info" : "good") : crawler.purpose === "training" ? "info" : "critical"}`}>
                  {crawler.allowed ? t("audit.allowed") : t("audit.blocked")}
                </span>
              </li>
            ))}
          </ul>
          {geo.schema_types.length > 0 && (
            <>
              <h4>{t("audit.schemaTypes")}</h4>
              <div className="feature-chips">
                {geo.schema_types.map(([name, pages]) => (
                  <span key={name} className="oc-chip">
                    {name} <strong>{pages}</strong>
                  </span>
                ))}
              </div>
            </>
          )}
        </section>
      </div>
    </section>
  );
}

function IssueExplorer({ report, onOpen }: { report: AuditReport; onOpen: (issue: IssueReport) => void }) {
  const { t, locale } = useI18n();
  const [filters, setFilters] = useState<IssueFilters>(ALL_ISSUES);
  const shown = useMemo(() => filterCategories(report.categories, filters), [report.categories, filters]);
  const counts = useMemo(() => totals(report.categories), [report.categories]);
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("audit.issuesTitle")}</h3>
          <span className="faint">{t("audit.issuesHint")}</span>
        </div>
      </header>
      <div className="page-stack">
        <div className="filters">
          <Segmented<IssueKind | "all">
            label={t("audit.kind")}
            value={filters.kind}
            onChange={(kind) => setFilters({ ...filters, kind })}
            options={[
              { value: "all", label: t("audit.kindAll") },
              ...KINDS.map((kind) => ({ value: kind, label: `${t(`audit.kind_${kind}`)} (${counts[kind]})` })),
            ]}
          />
          <label className="filter-search">
            <Search size={15} aria-hidden="true" />
            <span className="sr-only">{t("audit.filterSearch")}</span>
            <input className="oc-input" value={filters.query} placeholder={t("audit.filterSearch")} onChange={(event) => setFilters({ ...filters, query: event.target.value })} />
          </label>
          {report.previous && (
            <label className="oc-check">
              <input type="checkbox" checked={!filters.openOnly} onChange={(event) => setFilters({ ...filters, openOnly: !event.target.checked })} />
              <span>{t("audit.showFixed")}</span>
            </label>
          )}
        </div>
        {shown.length === 0 && <p className="faint">{t("audit.noIssues")}</p>}
        {shown.map((category) => (
          <section key={category.id} className="issue-group">
            <h4>{category.name}</h4>
            <ul className="issue-list">
              {category.issues.map((issue) => {
                const change = changeOf(issue);
                return (
                  <li key={issue.id}>
                    <button type="button" className="issue-row" onClick={() => onOpen(issue)} disabled={issue.count === 0}>
                      <span className={`oc-badge ${issue.kind === "issue" ? "critical" : issue.kind === "warning" ? "warning" : "info"}`}>{t(`audit.kind_${issue.kind}`)}</span>
                      <span className="issue-title">{issue.title}</span>
                      <span className="faint small">{t(`audit.priority_${issue.priority}`)}</span>
                      <strong className="issue-count">{issue.count === 0 ? t("audit.fixed") : formatNumber(issue.count, locale)}</strong>
                      <Change value={change} invert />
                    </button>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
    </section>
  );
}

/** A check: what it means, how to fix it, and where it was found. */
function IssueDialog({ project, auditId, issue, onClose }: { project: ProjectRow; auditId: number; issue: IssueReport; onClose: () => void }) {
  const { t, lang, locale } = useI18n();
  const [offset, setOffset] = useState(0);
  const detail = useQuery({
    queryKey: ["audit", "issue", project.id, auditId, issue.id, offset, lang],
    queryFn: () => api.auditIssue(project.id, auditId, issue.id, offset, lang),
  });
  const data = detail.data;
  const address = data ? Math.max(0, data.columns.findIndex((column) => column === "address" || column === "url")) : 0;
  return (
    <Modal title={issue.title} className="wide" closeLabel={t("common.close")} onClose={onClose} footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      {detail.isLoading && <Loading label={t("common.loading")} />}
      {detail.error && <div className="oc-banner error">{explain(detail.error, t).message}</div>}
      {data && (
        <div className="form-grid">
          <p>{data.description}</p>
          <section>
            <h4>{t("audit.howToFix")}</h4>
            <p>{data.fix}</p>
          </section>
          <section>
            <h4>{t("audit.foundOn", { n: formatNumber(data.total, locale) })}</h4>
            <ul className="issue-urls">
              {data.rows.map((row, index) => {
                const url = row[address] ?? "";
                return (
                  <li key={`${url}-${index}`}>
                    <a href={url} target="_blank" rel="noreferrer noopener">
                      {url} <ExternalLink size={12} aria-hidden="true" />
                    </a>
                  </li>
                );
              })}
            </ul>
            {data.total > data.rows.length && (
              <div className="table-toolbar">
                <Button small disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - 100))}>
                  <ChevronLeft size={14} />
                  {t("common.pagePrevious")}
                </Button>
                <span className="faint small">
                  {t("common.pageRange", { from: formatNumber(offset + 1, locale), to: formatNumber(offset + data.rows.length, locale), total: formatNumber(data.total, locale) })}
                </span>
                <Button small disabled={offset + data.rows.length >= data.total} onClick={() => setOffset(offset + 100)}>
                  {t("common.pageNext")}
                  <ChevronRight size={14} />
                </Button>
              </div>
            )}
          </section>
        </div>
      )}
    </Modal>
  );
}

function ReportView({ project, report, onOpen }: { project: ProjectRow; report: AuditReport; onOpen: (issue: IssueReport) => void }) {
  const { t, locale } = useI18n();
  const { audit, overview, geo, previous } = report;
  if (audit.status === "running") {
    return (
      <div className="oc-card">
        <Empty icon={<ScanSearch size={28} />} title={t("audit.runningTitle")}>
          <p>{t("audit.runningText", { domain: project.domain })}</p>
        </Empty>
      </div>
    );
  }
  return (
    <div className="page-stack">
      {audit.status === "failed" && <div className="oc-banner error">{t("audit.failed", { error: audit.error ?? "" })}</div>}
      {audit.status === "stopped" && <div className="oc-banner warning">{audit.error ? t("audit.stoppedBecause", { error: audit.error }) : t("audit.stopped")}</div>}
      {audit.status === "interrupted" && <div className="oc-banner warning">{t("audit.interrupted")}</div>}
      {overview && (
        <div className="oc-stats">
          <Stat
            label={t("audit.health")}
            value={audit.health === null ? "—" : one(audit.health, locale)}
            hint={
              <>
                {t("audit.healthHint")} <Change value={scoreChange(audit.health, previous?.health)} />
              </>
            }
          />
          <Stat
            label={t("audit.geoScore")}
            value={audit.geo === null ? "—" : one(audit.geo, locale)}
            hint={
              <>
                {t("audit.geoScoreHint")} <Change value={scoreChange(audit.geo, previous?.geo)} />
              </>
            }
          />
          <Stat
            label={t("audit.pages")}
            value={formatNumber(overview.html, locale)}
            hint={t("audit.pagesHint", { indexable: formatNumber(overview.indexable, locale), urls: formatNumber(overview.internal, locale) })}
          />
          <Stat
            label={t("audit.findings")}
            value={formatNumber(audit.issues, locale)}
            hint={t("audit.findingsHint", { urls: formatNumber(overview.affected_urls, locale), errors: formatNumber(overview.errors, locale) })}
          />
        </div>
      )}
      {audit.metered_pages > 0 && (
        <p className="faint small">
          {t("audit.metered", { pages: formatNumber(audit.metered_pages, locale), share: formatPercent(audit.pages ? (audit.metered_pages / audit.pages) * 100 : 0, locale) })}
        </p>
      )}
      {geo && geo.signals.length > 0 && <GeoPanel geo={geo} />}
      {overview && <IssueExplorer report={report} onOpen={onOpen} />}
    </div>
  );
}

export function SiteAudit({ project }: { project: ProjectRow }) {
  const { t, lang, locale } = useI18n();
  const { jobs, fail } = useShell();
  const queryClient = useQueryClient();
  const setup = useQuery({ queryKey: ["audit", "setup", project.id], queryFn: () => api.auditSetup(project.id) });
  const [auditId, setAuditId] = useState<number | null>(null);
  const [issue, setIssue] = useState<IssueReport | null>(null);
  const running = jobs.find((job) => job.kind === "site-audit" && job.project_id === project.id && job.status === "running");
  const report = useQuery({
    queryKey: ["audit", "report", project.id, auditId, lang],
    queryFn: () => api.auditReport(project.id, auditId, lang),
    enabled: Boolean(setup.data && setup.data.audits.length > 0),
  });
  const remove = useMutation({
    mutationFn: (id: number) => api.deleteAudit(project.id, id),
    onSuccess: () => {
      setAuditId(null);
      void queryClient.invalidateQueries({ queryKey: ["audit"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
  });
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  const shown = report.data?.audit;
  return (
    <div className="page-stack">
      <SetupBar project={project} setup={data} running={running} />
      {data.audits.length === 0 ? (
        <div className="oc-card">
          <Empty icon={<ScanSearch size={28} />} title={t("audit.emptyTitle")}>
            <p>{t("audit.emptyText", { domain: project.domain })}</p>
          </Empty>
        </div>
      ) : (
        <>
          <div className="audit-picker">
            <Field label={t("audit.which")}>
              <select className="oc-select" value={shown?.id ?? ""} onChange={(event) => setAuditId(Number(event.target.value))}>
                {data.audits.map((row) => (
                  <option key={row.id} value={row.id}>
                    {new Date(row.started_at).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" })} · {t(`audit.status_${row.status}`)}
                    {row.health === null ? "" : ` · ${one(row.health, locale)}`}
                  </option>
                ))}
              </select>
            </Field>
            {shown && shown.status !== "running" && (
              <Button
                small
                busy={remove.isPending}
                onClick={() => {
                  if (window.confirm(t("audit.deleteConfirm"))) remove.mutate(shown.id);
                }}
              >
                <Trash2 size={14} />
                {t("audit.delete")}
              </Button>
            )}
          </div>
          {report.isLoading && <Loading label={t("common.loading")} />}
          {report.error && <ErrorState title={t("common.loadFailed")} message={explain(report.error, t).message} retry={() => void report.refetch()} retryLabel={t("common.retry")} />}
          {report.data && <ReportView project={project} report={report.data} onOpen={setIssue} />}
        </>
      )}
      {issue && shown && <IssueDialog project={project} auditId={shown.id} issue={issue} onClose={() => setIssue(null)} />}
    </div>
  );
}
