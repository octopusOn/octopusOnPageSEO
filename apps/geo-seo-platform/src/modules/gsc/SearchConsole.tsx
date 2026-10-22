import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ArrowDown, ArrowUp, CheckCircle2, ChartLine, Download, ExternalLink, Link2Off, LogIn, LogOut, RefreshCw, Search, Sparkles, Target, TrendingUp } from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Modal, Segmented, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { GoogleStatus, GscPage, GscQuery, GscReport, GscSchedule, GscSetup, GscSync, Job, ProjectRow, SearchRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { useShell } from "../../lib/context";
import { useScheduleNote } from "../../lib/sync";
import { explain } from "../../lib/errors";
import { formatCompact, formatNumber, formatPercent, relativeTime } from "../../lib/format";
import { download, toCsv } from "../../lib/report";
import { shortUrl, tableLabels } from "../../lib/tables";
import { ALL_QUERIES, auditState, dailySeries, DAYS, filterQueries, pageCsv, percentChange, queryCsv, type QueryFilters } from "./gsc";

const GOOGLE_CLIENTS = "https://console.cloud.google.com/apis/credentials";

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

function money(value: number, currency: string | null, locale: string): string {
  return currency ? new Intl.NumberFormat(locale, { style: "currency", currency, maximumFractionDigits: 0 }).format(value) : formatNumber(value, locale);
}

/** A change with its direction; `invert` when lower is better (positions). */
function Change({ value, percent = false, invert = false }: { value: number | null; percent?: boolean; invert?: boolean }) {
  const { locale } = useI18n();
  if (value === null || value === 0) return null;
  const better = invert ? value < 0 : value > 0;
  return (
    <span className={`rank-change ${better ? "up" : "down"}`}>
      {value > 0 ? <ArrowUp size={12} aria-hidden="true" /> : <ArrowDown size={12} aria-hidden="true" />}
      {percent ? formatPercent(Math.abs(value), locale) : one(Math.abs(value), locale)}
    </span>
  );
}

/** The user's own Google OAuth client (desktop): id and secret. */
function ClientForm({ status, onDone, onCancel }: { status: GoogleStatus; onDone: () => void; onCancel?: () => void }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [clientId, setClientId] = useState(status.own_client_id ?? "");
  const [secret, setSecret] = useState("");
  const save = useMutation({
    mutationFn: () => api.setGoogleClient(clientId, secret),
    onSuccess: () => {
      notify(t("gsc.clientSaved"));
      void queryClient.invalidateQueries({ queryKey: ["gsc"] });
      onDone();
    },
    onError: (thrown) => fail(thrown),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    save.mutate();
  };
  return (
    <form className="form-grid client-form" onSubmit={submit}>
      <p className="faint small">
        {t("gsc.clientHint")}{" "}
        <a href={GOOGLE_CLIENTS} target="_blank" rel="noreferrer noopener">
          {t("gsc.clientConsole")} <ExternalLink size={12} aria-hidden="true" />
        </a>
      </p>
      <Field label={t("gsc.clientId")}>
        <input className="oc-input" value={clientId} onChange={(event) => setClientId(event.target.value)} placeholder="123-abc.apps.googleusercontent.com" spellCheck={false} />
      </Field>
      <Field label={t("gsc.clientSecret")}>
        <input className="oc-input" type="password" value={secret} onChange={(event) => setSecret(event.target.value)} autoComplete="off" spellCheck={false} />
      </Field>
      <div className="key-actions">
        <Button type="submit" primary small busy={save.isPending} disabled={!clientId.trim() || !secret.trim()}>
          {t("common.save")}
        </Button>
        {onCancel && (
          <Button small onClick={onCancel}>
            {t("common.cancel")}
          </Button>
        )}
      </div>
    </form>
  );
}

/** The Google connection: the client (desktop), signing in and out. */
function GoogleCard({ status, signingIn }: { status: GoogleStatus; signingIn: Job | undefined }) {
  const { t } = useI18n();
  const { notify, mode, fail } = useShell();
  const queryClient = useQueryClient();
  const [analytics, setAnalytics] = useState(true);
  const [editing, setEditing] = useState(false);
  const connect = useMutation({
    mutationFn: () => api.startGoogleLogin(analytics),
    onSuccess: (login) => {
      if (login.url) {
        window.location.assign(login.url);
        return;
      }
      notify(t("gsc.continueInBrowser"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const disconnect = useMutation({
    mutationFn: api.googleLogout,
    onSuccess: () => {
      notify(t("gsc.disconnected"));
      void queryClient.invalidateQueries({ queryKey: ["gsc"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const desktop = mode !== "web";
  return (
    <section className="oc-panel google-card">
      <header className="card-head">
        <span className="oc-tile blue">
          <ChartLine size={16} />
        </span>
        <div>
          <h3 className="oc-card-title">{t("gsc.googleTitle")}</h3>
          <p className={status.connected ? "good" : "faint"}>{status.connected ? t("gsc.connected") : t("gsc.notConnected")}</p>
        </div>
        {status.connected && (
          <div className="feature-chips">
            <span className={`oc-badge ${status.search_console ? "good" : "critical"}`}>Search Console</span>
            <span className={`oc-badge ${status.analytics ? "good" : "info"}`}>{status.analytics ? "Google Analytics" : t("gsc.noAnalytics")}</span>
          </div>
        )}
      </header>
      {!status.configured && !desktop && <p className="oc-banner warning">{t("gsc.webNotSetUp")}</p>}
      {((!status.configured && desktop) || editing) && <ClientForm status={status} onDone={() => setEditing(false)} onCancel={status.configured ? () => setEditing(false) : undefined} />}
      {status.configured && !editing && (
        <div className="google-actions">
          {signingIn ? (
            <>
              <span className="small">{t("gsc.waitingForGoogle")}</span>
              <Button small busy={cancel.isPending} onClick={() => cancel.mutate(signingIn.id)}>
                {t("common.cancel")}
              </Button>
            </>
          ) : status.connected ? (
            <>
              {!status.analytics && (
                <Button
                  small
                  busy={connect.isPending}
                  onClick={() => {
                    setAnalytics(true);
                    connect.mutate();
                  }}
                >
                  <Sparkles size={14} />
                  {t("gsc.addAnalytics")}
                </Button>
              )}
              <Button small busy={disconnect.isPending} onClick={() => disconnect.mutate()}>
                <LogOut size={14} />
                {t("gsc.disconnect")}
              </Button>
            </>
          ) : (
            <>
              <label className="oc-check">
                <input type="checkbox" checked={analytics} onChange={(event) => setAnalytics(event.target.checked)} />
                <span>{t("gsc.withAnalytics")}</span>
              </label>
              <Button primary small busy={connect.isPending} onClick={() => connect.mutate()}>
                <LogIn size={14} />
                {t("gsc.connect")}
              </Button>
            </>
          )}
          {desktop && (
            <Button small className="ghost" onClick={() => setEditing(true)}>
              {status.own_client_id ? t("gsc.changeClient") : t("gsc.ownClient")}
            </Button>
          )}
        </div>
      )}
      {status.configured && <p className="faint small">{t("gsc.privacy")}</p>}
    </section>
  );
}

/** Which properties the project reads, how often, and the sync itself. */
function PropertyBar({ project, setup, running }: { project: ProjectRow; setup: GscSetup; running: Job | undefined }) {
  const { t, locale } = useI18n();
  const scheduleNote = useScheduleNote();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const { settings } = setup;
  const properties = useQuery({ queryKey: ["gsc", "properties", project.id], queryFn: () => api.googleProperties(project.id), staleTime: 5 * 60_000 });
  const [property, setProperty] = useState(settings.property ?? "");
  const [ga4, setGa4] = useState(settings.ga4_property ?? "");
  useEffect(() => {
    if (!property && properties.data?.suggested) setProperty(properties.data.suggested);
  }, [properties.data, property]);
  const save = useMutation({
    mutationFn: (patch: Partial<{ property: string; ga4: string; days: number; schedule: GscSchedule }>) => {
      const chosen = patch.ga4 ?? ga4;
      return api.saveGscSettings(project.id, {
        property: (patch.property ?? property) || null,
        ga4_property: chosen || null,
        ga4_name: properties.data?.analytics.find((entry) => entry.id === chosen)?.name ?? null,
        days: patch.days ?? settings.days,
        schedule: patch.schedule ?? settings.schedule,
      });
    },
    onSuccess: (next) => queryClient.setQueryData(["gsc", "setup", project.id], next),
    onError: (thrown) => fail(thrown),
  });
  const sync = useMutation({
    mutationFn: () => api.startGscSync(project.id),
    onSuccess: () => {
      notify(t("gsc.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const changed = property !== (settings.property ?? "") || ga4 !== (settings.ga4_property ?? "");
  const next = settings.schedule === "manual" || !setup.next_sync_at ? t("gsc.manualOnly") : t("gsc.nextSync", { time: relativeTime(setup.next_sync_at, locale) });
  const sites = properties.data?.search_console ?? [];
  return (
    <section className="oc-panel rank-setup">
      {properties.error && <div className="oc-banner error">{explain(properties.error, t).message}</div>}
      <div className="rank-controls">
        <Field label={t("gsc.property")}>
          <select className="oc-select property-select" value={property} onChange={(event) => setProperty(event.target.value)} disabled={properties.isLoading}>
            <option value="">{properties.isLoading ? t("common.loading") : t("gsc.chooseProperty")}</option>
            {settings.property && !sites.some((site) => site.site_url === settings.property) && <option value={settings.property}>{settings.property}</option>}
            {sites.map((site) => (
              <option key={site.site_url} value={site.site_url}>
                {site.site_url}
                {site.site_url === properties.data?.suggested ? ` · ${t("gsc.suggested")}` : ""}
              </option>
            ))}
          </select>
        </Field>
        {setup.google.analytics && (
          <Field label={t("gsc.analyticsProperty")}>
            <select className="oc-select property-select" value={ga4} onChange={(event) => setGa4(event.target.value)}>
              <option value="">{t("gsc.noAnalyticsProperty")}</option>
              {settings.ga4_property && !properties.data?.analytics.some((entry) => entry.id === settings.ga4_property) && (
                <option value={settings.ga4_property}>{settings.ga4_name ?? settings.ga4_property}</option>
              )}
              {properties.data?.analytics.map((entry) => (
                <option key={entry.id} value={entry.id}>
                  {entry.account} · {entry.name}
                </option>
              ))}
            </select>
          </Field>
        )}
        {changed && (
          <Button primary small busy={save.isPending} disabled={!property} onClick={() => save.mutate({})}>
            {t("common.save")}
          </Button>
        )}
      </div>
      {properties.data?.analytics_error && <p className="faint small">{properties.data.analytics_error}</p>}
      <div className="rank-controls">
        <Field label={t("gsc.days")}>
          <Segmented<string>
            label={t("gsc.days")}
            value={String(settings.days)}
            disabled={save.isPending}
            onChange={(value) => save.mutate({ days: Number(value) })}
            options={DAYS.map((days) => ({ value: String(days), label: t("gsc.daysOption", { n: days }) }))}
          />
        </Field>
        <Field label={t("gsc.schedule")} hint={scheduleNote}>
          <Segmented<GscSchedule>
            label={t("gsc.schedule")}
            value={settings.schedule}
            disabled={save.isPending}
            onChange={(schedule) => save.mutate({ schedule })}
            options={[
              { value: "daily", label: t("gsc.scheduleDaily") },
              { value: "weekly", label: t("gsc.scheduleWeekly") },
              { value: "manual", label: t("gsc.scheduleManual") },
            ]}
          />
        </Field>
      </div>
      <div className="rank-run">
        <div className="rank-run-text">
          <span>{settings.property ? next : t("gsc.chooseFirst")}</span>
          <span className="faint small">{t("gsc.free")}</span>
        </div>
        <Button primary small busy={sync.isPending} disabled={Boolean(running) || !settings.property || changed} onClick={() => sync.mutate()}>
          <RefreshCw size={14} />
          {t("gsc.syncNow")}
        </Button>
      </div>
      {running && (
        <div className="rank-progress" aria-live="polite">
          <div className="oc-progress" aria-hidden="true">
            <div style={{ width: `${running.total ? (running.done / running.total) * 100 : 5}%` }} />
          </div>
          <span className="small">
            {t("gsc.syncing", { done: formatNumber(running.done, locale), total: formatNumber(running.total, locale) })} <span className="faint">{running.step ? t(`gsc.step_${running.step.replace(/-/g, "_")}`) : ""}</span>
          </span>
          <Button small busy={cancel.isPending} onClick={() => cancel.mutate(running.id)}>
            {t("gsc.stop")}
          </Button>
        </div>
      )}
    </section>
  );
}

function DailyChart({ report }: { report: GscReport }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    const series = dailySeries(report.daily, report.period[0], report.previous[0]);
    const sessions = series.sessions.some((value) => value !== null);
    return {
      grid: { left: 44, right: 52, top: 36, bottom: 28 },
      legend: { top: 0, left: 0, type: "scroll" },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: series.dates.map((date) => new Date(date).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", min: 0, ...axis },
        { type: "value", min: 0, ...axis, splitLine: { show: false } },
      ],
      series: [
        { name: t("gsc.clicks"), type: "line", smooth: true, symbol: "none", lineStyle: { width: 2.5 }, areaStyle: { opacity: 0.12 }, data: series.clicks },
        { name: t("gsc.clicksBefore"), type: "line", smooth: true, symbol: "none", connectNulls: true, lineStyle: { width: 1.5, type: "dashed" }, data: series.clicksBefore },
        { name: t("gsc.impressions"), type: "line", smooth: true, symbol: "none", yAxisIndex: 1, lineStyle: { width: 1.5 }, data: series.impressions },
        ...(sessions ? [{ name: t("gsc.sessions"), type: "line", smooth: true, symbol: "none", lineStyle: { width: 1.5, type: "dotted" }, data: series.sessions }] : []),
      ],
    };
  }, [report, tokens, t, locale]);
  return <EChart option={option} height={280} />;
}

/** The pages Search Console showed for a query. */
function QueryDialog({ project, query, onClose }: { project: ProjectRow; query: string; onClose: () => void }) {
  const { t, locale } = useI18n();
  const pages = useQuery({ queryKey: ["gsc", "query", project.id, query], queryFn: () => api.gscQuery(project.id, query) });
  return (
    <Modal title={query} className="wide" closeLabel={t("common.close")} onClose={onClose} footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      {pages.isLoading && <Loading label={t("common.loading")} />}
      {pages.error && <div className="oc-banner error">{explain(pages.error, t).message}</div>}
      {pages.data && (
        <div className="form-grid">
          <p className="faint small">{pages.data.length > 1 ? t("gsc.queryPagesMany", { n: pages.data.length }) : t("gsc.queryPagesHint")}</p>
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t("gsc.colPage")}</th>
                <th className="number">{t("gsc.clicks")}</th>
                <th className="number">{t("gsc.impressions")}</th>
                <th className="number">{t("gsc.position")}</th>
              </tr>
            </thead>
            <tbody>
              {pages.data.map((row) => (
                <tr key={row.page}>
                  <td>
                    <a href={row.page} target="_blank" rel="noreferrer noopener" title={row.page}>
                      {shortUrl(row.page)}
                    </a>
                  </td>
                  <td className="number">{formatNumber(row.clicks, locale)}</td>
                  <td className="number">{formatNumber(row.impressions, locale)}</td>
                  <td className="number">{one(row.position, locale)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Modal>
  );
}

function QueryTable({ project, rows, onOpen }: { project: ProjectRow; rows: GscQuery[]; onOpen: (query: string) => void }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [filters, setFilters] = useState<QueryFilters>(ALL_QUERIES);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const shown = useMemo(() => filterQueries(rows, filters), [rows, filters]);
  const chosen = rows.filter((row) => selected.has(row.query) && !row.tracked).map((row) => row.query);
  const track = useMutation({
    mutationFn: () => api.trackKeywords(project.id, chosen.map((keyword) => ({ keyword, volume: null }))),
    onSuccess: (tracked) => {
      notify(tracked.added ? t("rank.tracked", { n: tracked.added }) : t("rank.alreadyTracked"));
      setSelected(new Set());
      void queryClient.invalidateQueries({ queryKey: ["rank"] });
      void queryClient.invalidateQueries({ queryKey: ["gsc", "report", project.id] });
    },
    onError: (thrown) => fail(thrown),
  });
  const toggle = (query: string) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(query)) next.delete(query);
      else next.add(query);
      return next;
    });
  const columns: Column<GscQuery>[] = [
    {
      key: "select",
      label: "",
      width: "36px",
      render: (row) => <input type="checkbox" checked={selected.has(row.query)} disabled={row.tracked} onChange={() => toggle(row.query)} aria-label={t("keywords.select", { keyword: row.query })} />,
    },
    {
      key: "query",
      label: t("gsc.colQuery"),
      render: (row) => (
        <span className="ai-prompt">
          <button type="button" className="link-button cell-main" onClick={() => onOpen(row.query)} title={t("gsc.openQuery")}>
            {row.query}
          </button>
          {row.tracked && (
            <span className="signal good" title={t("gsc.trackedHint")}>
              <TrendingUp size={13} aria-hidden="true" />
            </span>
          )}
          {row.near_top && <span className="oc-badge info">{t("gsc.nearTop")}</span>}
          {row.low_ctr && <span className="oc-badge warning">{t("gsc.lowCtr")}</span>}
          {row.pages > 1 && (
            <span className="signal warning" title={t("gsc.manyPages", { n: row.pages })}>
              <AlertTriangle size={13} aria-hidden="true" />
              {row.pages}
            </span>
          )}
        </span>
      ),
      sort: (row) => row.query,
    },
    {
      key: "clicks",
      label: t("gsc.clicks"),
      render: (row) => (
        <span className="rank-position">
          {formatNumber(row.clicks, locale)} <Change value={row.clicks_change} />
        </span>
      ),
      sort: (row) => row.clicks,
      numeric: true,
    },
    { key: "impressions", label: t("gsc.impressions"), render: (row) => formatNumber(row.impressions, locale), sort: (row) => row.impressions, numeric: true },
    { key: "ctr", label: t("gsc.ctr"), render: (row) => formatPercent(row.ctr, locale), sort: (row) => row.ctr, numeric: true },
    {
      key: "position",
      label: t("gsc.position"),
      render: (row) => (
        <span className="rank-position">
          {one(row.position, locale)} <Change value={row.position_change} />
        </span>
      ),
      sort: (row) => row.position,
      numeric: true,
    },
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("gsc.queriesTitle")}</h3>
          <span className="faint">{t("gsc.queriesHint")}</span>
        </div>
      </header>
      <div className="page-stack">
        <div className="filters">
          <label className="filter-search">
            <Search size={15} aria-hidden="true" />
            <span className="sr-only">{t("gsc.filterSearch")}</span>
            <input className="oc-input" value={filters.query} placeholder={t("gsc.filterSearch")} onChange={(event) => setFilters({ ...filters, query: event.target.value })} />
          </label>
          <label className="oc-check" title={t("gsc.nearTopHint")}>
            <input type="checkbox" checked={filters.nearTop} onChange={(event) => setFilters({ ...filters, nearTop: event.target.checked })} />
            <span>{t("gsc.onlyNearTop")}</span>
          </label>
          <label className="oc-check">
            <input type="checkbox" checked={filters.untracked} onChange={(event) => setFilters({ ...filters, untracked: event.target.checked })} />
            <span>{t("gsc.onlyUntracked")}</span>
          </label>
        </div>
        <div className="table-toolbar">
          <Button small disabled={chosen.length === 0} busy={track.isPending} onClick={() => track.mutate()} title={t("gsc.trackHint")}>
            <Target size={14} />
            {t("rank.trackSelected", { n: chosen.length })}
          </Button>
          <Button
            small
            disabled={shown.length === 0}
            onClick={() =>
              download(
                `search-console-queries-${project.domain}.csv`,
                toCsv([t("gsc.colQuery"), t("gsc.clicks"), t("gsc.clicksChange"), t("gsc.impressions"), t("gsc.ctr"), t("gsc.position"), t("gsc.positionChange"), t("gsc.colPages"), t("gsc.colTracked")], queryCsv(shown)),
              )
            }
          >
            <Download size={14} />
            {t("overview.export")}
          </Button>
        </div>
        <DataTable rows={shown} columns={columns} rowKey={(row) => row.query} initialSort={{ key: "clicks", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      </div>
    </section>
  );
}

function AuditBadge({ page }: { page: GscPage }) {
  const { t, locale } = useI18n();
  const state = auditState(page.audit);
  if (!state || !page.audit) return null;
  const tone = state === "ok" ? "good" : state === "findings" ? "info" : "critical";
  const label =
    state === "findings" ? t("gsc.audit_findings", { n: formatNumber(page.audit.findings, locale) }) : state === "error" && page.audit.status ? `${t("gsc.audit_error")} ${page.audit.status}` : t(`gsc.audit_${state}`);
  return <span className={`oc-badge ${tone}`}>{label}</span>;
}

function PageTable({ project, report }: { project: ProjectRow; report: GscReport }) {
  const { t, locale } = useI18n();
  const analytics = report.pages.some((row) => row.sessions !== null);
  const audited = report.pages.some((row) => row.audit !== null);
  const columns: Column<GscPage>[] = [
    {
      key: "page",
      label: t("gsc.colPage"),
      render: (row) => (
        <span className="page-cell">
          <a className="cell-url" href={row.page} target="_blank" rel="noreferrer noopener" title={row.page}>
            {shortUrl(row.page)}
          </a>
          {audited && <AuditBadge page={row} />}
        </span>
      ),
      sort: (row) => row.page,
    },
    {
      key: "clicks",
      label: t("gsc.clicks"),
      render: (row) => (
        <span className="rank-position">
          {formatNumber(row.clicks, locale)} <Change value={row.clicks_change} />
        </span>
      ),
      sort: (row) => row.clicks,
      numeric: true,
    },
    { key: "impressions", label: t("gsc.impressions"), render: (row) => formatNumber(row.impressions, locale), sort: (row) => row.impressions, numeric: true },
    { key: "ctr", label: t("gsc.ctr"), render: (row) => formatPercent(row.ctr, locale), sort: (row) => row.ctr, numeric: true },
    { key: "position", label: t("gsc.position"), render: (row) => one(row.position, locale), sort: (row) => row.position, numeric: true },
    ...(analytics
      ? [
          { key: "sessions", label: t("gsc.sessions"), render: (row: GscPage) => (row.sessions === null ? "—" : formatNumber(row.sessions, locale)), sort: (row: GscPage) => row.sessions, numeric: true },
          { key: "events", label: t("gsc.keyEvents"), render: (row: GscPage) => (row.key_events === null ? "—" : formatNumber(row.key_events, locale)), sort: (row: GscPage) => row.key_events, numeric: true },
          { key: "revenue", label: t("gsc.revenue"), render: (row: GscPage) => (row.revenue === null ? "—" : money(row.revenue, report.currency, locale)), sort: (row: GscPage) => row.revenue, numeric: true },
        ]
      : []),
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("gsc.pagesTitle")}</h3>
          <span className="faint">{analytics ? t("gsc.pagesHintAnalytics") : t("gsc.pagesHint")}</span>
          {audited && <span className="faint small">{t("gsc.pagesAuditHint")}</span>}
        </div>
        <div className="report-actions">
          <Button
            small
            disabled={report.pages.length === 0}
            onClick={() =>
              download(
                `search-console-pages-${project.domain}.csv`,
                toCsv([t("gsc.colPage"), t("gsc.clicks"), t("gsc.clicksChange"), t("gsc.impressions"), t("gsc.ctr"), t("gsc.position"), t("gsc.sessions"), t("gsc.keyEvents"), t("gsc.revenue")], pageCsv(report.pages)),
              )
            }
          >
            <Download size={14} />
            {t("overview.export")}
          </Button>
        </div>
      </header>
      <DataTable rows={report.pages} columns={columns} rowKey={(row) => row.page} initialSort={{ key: "clicks", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </section>
  );
}

function SplitBars({ rows, label, name }: { rows: SearchRow[]; label: string; name: (key: string) => string }) {
  const { locale } = useI18n();
  const total = rows.reduce((sum, row) => sum + row.clicks, 0) || 1;
  const most = Math.max(1, ...rows.map((row) => row.clicks));
  return (
    <ol className="bars split" aria-label={label}>
      {rows.map((row) => (
        <li key={row.key}>
          <span className="bar-label" title={row.key}>
            {name(row.key)}
          </span>
          <span className="bar-track" aria-hidden="true">
            <span className="bar-fill" style={{ width: `${(row.clicks / most) * 100}%` }} />
          </span>
          <span className="bar-value">{formatPercent((row.clicks / total) * 100, locale)}</span>
        </li>
      ))}
    </ol>
  );
}

function Syncs({ syncs }: { syncs: GscSync[] }) {
  const { t, locale } = useI18n();
  if (syncs.length === 0) return null;
  const tone = (sync: GscSync) => (sync.status === "done" ? "good" : sync.status === "running" ? "accent" : sync.status === "partial" || sync.status === "cancelled" ? "warning" : "critical");
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("gsc.syncsTitle")}</h3>
        </div>
      </header>
      <ul className="run-list">
        {syncs.map((sync) => (
          <li key={sync.id}>
            <span className={`oc-badge ${tone(sync)}`}>{t(`gsc.sync_${sync.status}`)}</span>
            <span>{relativeTime(sync.started_at, locale)}</span>
            <span className="faint small">{sync.scheduled ? t("gsc.syncScheduled") : t("gsc.syncManual")}</span>
            {sync.period_start && sync.period_end && <span className="small">{t("gsc.syncLine", { start: sync.period_start, end: sync.period_end, rows: formatNumber(sync.rows, locale) })}</span>}
            {sync.error && <span className="faint small run-error">{sync.error}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

function ReportView({ project, report, onOpen }: { project: ProjectRow; report: GscReport; onOpen: (query: string) => void }) {
  const { t, locale } = useI18n();
  const { go } = useShell();
  const { now, before } = report;
  const deviceName = (key: string) => t(`gsc.device_${key}`);
  const countryName = (key: string) => key.toUpperCase();
  const positionChange = now.position !== null && before.position !== null ? Math.round((now.position - before.position) * 10) / 10 : null;
  return (
    <div className="page-stack">
      <p className="faint small">
        {t("gsc.periodLine", { start: report.period[0], end: report.period[1], before: `${report.previous[0]} – ${report.previous[1]}` })}
        {report.sync.limited && <> · {t("gsc.limited")}</>}
      </p>
      <div className="oc-stats">
        <Stat label={t("gsc.clicks")} value={formatCompact(now.clicks, locale)} hint={<Change value={percentChange(now.clicks, before.clicks)} percent />} />
        <Stat label={t("gsc.impressions")} value={formatCompact(now.impressions, locale)} hint={<Change value={percentChange(now.impressions, before.impressions)} percent />} />
        <Stat label={t("gsc.ctr")} value={formatPercent(now.ctr, locale)} hint={<Change value={Math.round((now.ctr - before.ctr) * 10) / 10} />} />
        <Stat label={t("gsc.position")} value={now.position === null ? "—" : one(now.position, locale)} hint={<Change value={positionChange} invert />} />
        {report.traffic_now && (
          <Stat
            label={t("gsc.organicSessions")}
            value={formatCompact(report.traffic_now.sessions, locale)}
            hint={
              <>
                {t("gsc.keyEventsLine", { n: formatNumber(report.traffic_now.key_events, locale) })}{" "}
                <Change value={report.traffic_before ? percentChange(report.traffic_now.sessions, report.traffic_before.sessions) : null} percent />
              </>
            }
          />
        )}
      </div>
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("gsc.dailyTitle")}</h3>
            <span className="faint">{t("gsc.dailyHint")}</span>
          </div>
        </header>
        {report.daily.length > 1 ? <DailyChart report={report} /> : <p className="faint">{t("gsc.noDays")}</p>}
      </section>
      <div className="two-columns">
        <section className="oc-panel report">
          <header className="report-head">
            <div className="report-title">
              <h3 className="oc-panel-title">{t("gsc.joinTitle")}</h3>
            </div>
          </header>
          <ul className="check-list join-list">
            <li>
              <TrendingUp size={15} aria-hidden="true" />
              <span>{t("gsc.trackedLine", { seen: formatNumber(report.tracked[0], locale), total: formatNumber(report.tracked[1], locale) })}</span>
              <Button small className="ghost" onClick={() => go({ page: "rank" })}>
                {t("gsc.openRank")}
              </Button>
            </li>
            <li>
              {report.audit && report.audit.problem_pages > 0 ? <AlertTriangle size={15} className="warn" aria-hidden="true" /> : <CheckCircle2 size={15} className="good" aria-hidden="true" />}
              <span>{report.audit ? t("gsc.auditLine", { n: formatNumber(report.audit.problem_pages, locale) }) : t("gsc.noAudit")}</span>
              <Button small className="ghost" onClick={() => go({ page: "audit" })}>
                {t("gsc.openAudit")}
              </Button>
            </li>
            <li>
              {report.cannibalised.length > 0 ? <AlertTriangle size={15} className="warn" aria-hidden="true" /> : <CheckCircle2 size={15} className="good" aria-hidden="true" />}
              <span>{t("gsc.cannibalisedLine", { n: formatNumber(report.cannibalised.length, locale) })}</span>
            </li>
          </ul>
        </section>
        <section className="oc-panel report">
          <header className="report-head">
            <div className="report-title">
              <h3 className="oc-panel-title">{t("gsc.splitTitle")}</h3>
            </div>
          </header>
          <div className="split-grid">
            <section>
              <h4>{t("gsc.devices")}</h4>
              <SplitBars rows={report.devices} label={t("gsc.devices")} name={deviceName} />
            </section>
            <section>
              <h4>{t("gsc.countries")}</h4>
              <SplitBars rows={report.countries.slice(0, 8)} label={t("gsc.countries")} name={countryName} />
            </section>
          </div>
        </section>
      </div>
      <QueryTable project={project} rows={report.queries} onOpen={onOpen} />
      <PageTable project={project} report={report} />
      {report.cannibalised.length > 0 && (
        <section className="oc-panel report">
          <header className="report-head">
            <div className="report-title">
              <h3 className="oc-panel-title">{t("gsc.cannibalisedTitle")}</h3>
              <span className="faint">{t("gsc.cannibalisedHint")}</span>
            </div>
          </header>
          <ul className="cannibal-list">
            {report.cannibalised.map((entry) => (
              <li key={entry.query}>
                <button type="button" className="link-button cell-main" onClick={() => onOpen(entry.query)}>
                  {entry.query}
                </button>
                <span className="faint small">{t("gsc.impressionsLine", { n: formatNumber(entry.impressions, locale) })}</span>
                <ol>
                  {entry.pages.map((page) => (
                    <li key={page.page}>
                      <a href={page.page} target="_blank" rel="noreferrer noopener" title={page.page}>
                        {shortUrl(page.page)}
                      </a>
                      <span className="faint small">{t("gsc.pageLine", { clicks: formatNumber(page.clicks, locale), impressions: formatNumber(page.impressions, locale), position: one(page.position, locale) })}</span>
                    </li>
                  ))}
                </ol>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

export function SearchConsole({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { jobs } = useShell();
  const setup = useQuery({ queryKey: ["gsc", "setup", project.id], queryFn: () => api.gscSetup(project.id) });
  const connected = setup.data?.google.connected ?? false;
  const report = useQuery({ queryKey: ["gsc", "report", project.id], queryFn: () => api.gscReport(project.id), enabled: connected || Boolean(setup.data?.syncs.length) });
  const [query, setQuery] = useState<string | null>(null);
  const running = jobs.find((job) => job.kind === "search-console" && job.project_id === project.id && job.status === "running");
  const signingIn = jobs.find((job) => job.kind === "google-login" && job.status === "running");
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  return (
    <div className="page-stack">
      <GoogleCard status={data.google} signingIn={signingIn} />
      {connected && <PropertyBar key={data.settings.property ?? ""} project={project} setup={data} running={running} />}
      {report.isLoading && <Loading label={t("common.loading")} />}
      {report.error && <ErrorState title={t("common.loadFailed")} message={explain(report.error, t).message} retry={() => void report.refetch()} retryLabel={t("common.retry")} />}
      {report.data && <ReportView project={project} report={report.data} onOpen={setQuery} />}
      {report.isSuccess && !report.data && connected && (
        <div className="oc-card">
          <Empty icon={<ChartLine size={28} />} title={t("gsc.emptyTitle")}>
            <p>{data.settings.property ? t("gsc.emptyText") : t("gsc.chooseFirst")}</p>
          </Empty>
        </div>
      )}
      {!connected && !report.data && (
        <div className="oc-card">
          <Empty icon={<Link2Off size={28} />} title={t("gsc.connectTitle")}>
            <p>{t("gsc.connectText", { domain: project.domain })}</p>
          </Empty>
        </div>
      )}
      <Syncs syncs={data.syncs} />
      {query && <QueryDialog project={project} query={query} onClose={() => setQuery(null)} />}
    </div>
  );
}
