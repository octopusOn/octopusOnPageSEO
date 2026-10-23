import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, Check, FileSpreadsheet, Minus, Send, Trash2 } from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Modal, Segmented, Stat, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type {
  CellMove,
  IssueMove,
  Job,
  KeywordMove,
  LinkingDomain,
  LocalMove,
  PageMove,
  ProjectRow,
  ReportSpec,
  ReportsSetup,
  SnapshotComparison,
  SnapshotEstimate,
  SnapshotKpi,
  SnapshotRow,
  SnapshotSchedule,
  SnapshotSetup,
  TrackedMove,
} from "../../api/types";
import { useI18n } from "../../i18n";
import { Entity } from "../../lib/Entity";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatCredits, formatNumber, formatPercent, formatUsd, relativeTime } from "../../lib/format";
import { ProblemBanner, refreshSpending } from "../../lib/PaidFrame";
import { downloadBase64 } from "../../lib/report";
import { useScheduleNote } from "../../lib/sync";
import { tableLabels } from "../../lib/tables";
import { ENGINE_NAMES } from "../ai/ai";
import { ReportDialog } from "../reports/ReportDialogs";
import { chosenCompetitors, defaultPair, deltaText, finishedOf, formatKpi, HEADLINE_KPIS, pathOf, placesText, TABS, TIMELINE_NUMBERS, toggled, toneOf, type Tab } from "./snapshot";

type T = (key: string, values?: Record<string, string | number>) => string;

/** What taking a snapshot costs, in the way it is paid. */
function costText(estimate: SnapshotEstimate, t: T, locale: string): string {
  if (estimate.paid === 0) return t("snapshots.costFree");
  // Nothing can be bought now: only the free parts are kept.
  if (estimate.blocked) return t("snapshots.costBlocked");
  if (estimate.via === "credit" && estimate.credits !== null) {
    if (estimate.full_credits !== null && estimate.full_credits > estimate.credits + 1e-9) {
      return t("snapshots.costShared", { credits: formatCredits(estimate.credits, locale), full: formatCredits(estimate.full_credits, locale) });
    }
    return t("common.credits", { n: formatCredits(estimate.credits, locale) });
  }
  return formatUsd(estimate.usd, locale);
}

/** Why the paid parts cannot be taken now. */
function blockedOf(estimate: SnapshotEstimate | undefined, t: T): string | null {
  const reason = estimate?.blocked;
  if (!reason || estimate.paid === 0) return null;
  return reason.code === "budget" ? `${reason.message} ${t("common.errorBudget")} ${t("snapshots.freePartsStill")}` : t("snapshots.noSource");
}

function sideName(row: { label: string | null; taken_at: string }, baseline: boolean, t: T, locale: string): string {
  const day = new Date(row.taken_at).toLocaleDateString(locale, { day: "numeric", month: "short", year: "numeric" });
  const name = row.label ?? (baseline ? t("snapshots.baseline") : null);
  return name ? `${day} · ${name}` : day;
}

function useEstimate(project: ProjectRow, competitors: string[]) {
  return useQuery({ queryKey: ["snapshots", "estimate", project.id, competitors.join()], queryFn: () => api.snapshotEstimate(project.id, competitors) });
}

/** The label, the price and, the first time, taking one every month on its own. */
function TakeDialog({ project, setup, competitors, onClose }: { project: ProjectRow; setup: SnapshotSetup; competitors: string[]; onClose: () => void }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const first = setup.snapshots.length === 0;
  const [label, setLabel] = useState(first ? t("snapshots.baseline") : "");
  const [monthly, setMonthly] = useState(first);
  const estimate = useEstimate(project, competitors);
  const take = useMutation({
    mutationFn: async () => {
      // Started first: the snapshot it takes now counts as this month's.
      const job = await api.startSnapshot(project.id, { competitors, label: label.trim() || undefined });
      if (monthly && setup.settings.schedule === "manual") await api.saveSnapshotSettings(project.id, { schedule: "monthly", competitors });
      return job;
    },
    onSuccess: () => {
      notify(t("snapshots.started"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      void queryClient.invalidateQueries({ queryKey: ["snapshots"] });
      onClose();
    },
    onError: (thrown) => fail(thrown),
  });
  const data = estimate.data;
  const blocked = blockedOf(data, t);
  return (
    <Modal
      title={t("snapshots.takeTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={take.isPending} disabled={!data} onClick={() => take.mutate()}>
            <Camera size={15} />
            {data ? t("snapshots.takeConfirm", { cost: costText(data, t, locale) }) : t("snapshots.take")}
          </Button>
        </>
      }
    >
      <div className="dialog-stack">
        <p className="small">{t("snapshots.takeWhat")}</p>
        {competitors.length > 0 && <p className="faint small">{t("snapshots.takeCompetitors", { list: competitors.join(", ") })}</p>}
        <Field label={t("snapshots.takeLabel")} hint={t("snapshots.takeLabelHint")}>
          <input className="oc-input" value={label} maxLength={120} onChange={(event) => setLabel(event.target.value)} placeholder={t("snapshots.takeLabelPlaceholder")} />
        </Field>
        {estimate.isLoading && <Loading label={t("common.loading")} />}
        {estimate.error && <ProblemBanner error={estimate.error} retry={() => void estimate.refetch()} />}
        {data && !blocked && (
          <div className="snapshot-price">
            <strong>{costText(data, t, locale)}</strong>
            <span className="faint small">{t("snapshots.costParts", { paid: data.paid, kept: data.kept })}</span>
          </div>
        )}
        {blocked && (
          <div className="oc-banner warning" role="status">
            <span>{blocked}</span>
          </div>
        )}
        {setup.settings.schedule === "manual" && (
          <label className="oc-check">
            <input type="checkbox" checked={monthly} onChange={(event) => setMonthly(event.target.checked)} />
            <span>
              {t("snapshots.takeMonthly")}
              <small className="faint"> {t("snapshots.takeMonthlyHint")}</small>
            </span>
          </label>
        )}
      </div>
    </Modal>
  );
}

/** Schedule, competitors, what a snapshot costs and taking one. */
function SetupBar({ project, setup, running, onTake }: { project: ProjectRow; setup: SnapshotSetup; running: Job | undefined; onTake: () => void }) {
  const { t, locale } = useI18n();
  const scheduleNote = useScheduleNote();
  const { go, fail } = useShell();
  const queryClient = useQueryClient();
  const competitors = chosenCompetitors(setup);
  const estimate = useEstimate(project, competitors);
  const save = useMutation({
    mutationFn: (patch: { schedule?: SnapshotSchedule; competitors?: string[] }) =>
      api.saveSnapshotSettings(project.id, { schedule: setup.settings.schedule, competitors, ...patch }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["snapshots"] }),
    onError: (thrown) => fail(thrown),
  });
  const cancel = useMutation({ mutationFn: (id: string) => api.cancelJob(id) });
  const blocked = blockedOf(estimate.data, t);
  const next =
    setup.settings.schedule === "manual" || setup.next_at === null
      ? t("snapshots.manualOnly")
      : t("snapshots.nextAuto", { time: relativeTime(new Date(Math.max(setup.next_at * 1000, Date.now())).toISOString(), locale) });
  return (
    <section className="oc-panel rank-setup">
      <div className="rank-controls">
        <Field label={t("snapshots.schedule")} hint={scheduleNote ?? t("snapshots.scheduleHint")}>
          <Segmented<SnapshotSchedule>
            label={t("snapshots.schedule")}
            value={setup.settings.schedule}
            disabled={save.isPending}
            onChange={(schedule) => save.mutate({ schedule })}
            options={[
              { value: "monthly", label: t("snapshots.scheduleMonthly") },
              { value: "weekly", label: t("snapshots.scheduleWeekly") },
              { value: "manual", label: t("snapshots.scheduleManual") },
            ]}
          />
        </Field>
        <Field label={t("snapshots.competitors")} hint={setup.competitors.length ? t("snapshots.competitorsHint") : t("snapshots.noCompetitors")}>
          <div className="rank-places" role="group" aria-label={t("snapshots.competitors")}>
            {setup.competitors.map((domain) => {
              const on = competitors.includes(domain);
              return (
                <button
                  key={domain}
                  type="button"
                  className={`oc-chip chip-button ${on ? "accent" : ""}`}
                  aria-pressed={on}
                  disabled={save.isPending}
                  onClick={() => save.mutate({ competitors: toggled(competitors, domain, setup.competitors) })}
                >
                  {on ? <Check size={13} aria-hidden="true" /> : <Minus size={13} aria-hidden="true" />}
                  <span>{domain}</span>
                </button>
              );
            })}
          </div>
        </Field>
      </div>
      <div className="rank-run">
        <div className="rank-run-text">
          <span>{next}</span>
          {estimate.data && !blocked && (
            <span className="faint small">
              {t("snapshots.costLine", { cost: costText(estimate.data, t, locale), paid: estimate.data.paid, kept: estimate.data.kept })}
            </span>
          )}
        </div>
        <Button primary small busy={false} disabled={Boolean(running)} onClick={onTake}>
          <Camera size={14} />
          {t("snapshots.take")}
        </Button>
      </div>
      {running && (
        <div className="rank-progress" aria-live="polite">
          <div className="oc-progress" aria-hidden="true">
            <div style={{ width: `${running.total ? (running.done / running.total) * 100 : 5}%` }} />
          </div>
          <span className="small">
            {t("snapshots.taking", { done: running.done, total: running.total })} <span className="faint">{running.step ? t(`snapshots.step_${running.step}`) : ""}</span>
          </span>
          <Button small busy={cancel.isPending} onClick={() => cancel.mutate(running.id)}>
            {t("snapshots.cancel")}
          </Button>
        </div>
      )}
      {blocked && (
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

/** Traffic, keywords and authority over the snapshots. */
function TimelineChart({ rows }: { rows: SnapshotRow[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    const points = [...rows].reverse();
    const line = (key: "traffic" | "keywords" | "oa" | "health", right: boolean) => ({
      name: t(`snapshots.kpi_${key}`),
      type: "line",
      yAxisIndex: right ? 1 : 0,
      symbol: "circle",
      symbolSize: 7,
      connectNulls: true,
      lineStyle: right ? { width: 1.5, type: "dashed" } : { width: 3 },
      data: points.map((row) => row.headline?.[key] ?? null),
    });
    return {
      grid: { left: 56, right: 44, top: 36, bottom: 28 },
      legend: { top: 0, left: 0, type: "scroll" },
      tooltip: { trigger: "axis", valueFormatter: (value: unknown) => (typeof value === "number" ? formatNumber(value, locale) : "—") },
      xAxis: { type: "category", data: points.map((row) => new Date(row.taken_at).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", ...axis },
        { type: "value", min: 0, max: 100, ...axis, splitLine: { show: false } },
      ],
      series: [line("traffic", false), line("keywords", false), line("oa", true), line("health", true)],
    };
  }, [rows, tokens, t, locale]);
  return <EChart option={option} height={240} />;
}

function statusTone(status: SnapshotRow["status"]): string {
  if (status === "done") return "good";
  if (status === "running") return "accent";
  return status === "partial" || status === "interrupted" ? "warning" : "critical";
}

function Timeline({ project, setup }: { project: ProjectRow; setup: SnapshotSetup }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const remove = useMutation({
    mutationFn: (id: number) => api.deleteSnapshot(project.id, id),
    onSuccess: () => {
      notify(t("snapshots.deleted"));
      void queryClient.invalidateQueries({ queryKey: ["snapshots"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const finished = finishedOf(setup.snapshots);
  return (
    <section className="oc-panel report snapshot-panel">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("snapshots.timelineTitle")}</h3>
          <span className="faint">{t("snapshots.timelineHint")}</span>
        </div>
      </header>
      {finished.length > 1 && <TimelineChart rows={finished} />}
      <ul className="run-list snapshot-list">
        {setup.snapshots.map((row) => {
          const baseline = row.id === setup.baseline_id;
          return (
            <li key={row.id}>
              <span className={`oc-badge ${statusTone(row.status)}`}>{t(`snapshots.status_${row.status}`)}</span>
              <strong>{sideName(row, baseline, t, locale)}</strong>
              <span className="faint small">
                {row.kind === "scheduled" ? t("snapshots.kindScheduled") : t("snapshots.kindManual")} · {formatUsd(row.cost_usd, locale)}
              </span>
              {row.headline && (
                <span className="snapshot-numbers small">
                  {TIMELINE_NUMBERS.filter((key) => row.headline?.[key] !== null).map((key) => (
                    <span key={key}>
                      <span className="faint">{t(`snapshots.kpi_${key}`)}</span> {formatKpi(key, row.headline?.[key] ?? null, locale)}
                    </span>
                  ))}
                </span>
              )}
              {row.error && <span className="faint small run-error">{row.error}</span>}
              <Button
                small
                className="icon ghost"
                busy={remove.isPending && remove.variables === row.id}
                disabled={row.status === "running"}
                aria-label={t("snapshots.delete")}
                title={t("snapshots.delete")}
                onClick={() => {
                  if (window.confirm(t("snapshots.deleteConfirm", { name: sideName(row, baseline, t, locale) }))) remove.mutate(row.id);
                }}
              >
                <Trash2 size={14} />
              </Button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function KpiCard({ kpi }: { kpi: SnapshotKpi }) {
  const { t, locale } = useI18n();
  const moved = deltaText(kpi, locale);
  return (
    <Stat
      label={t(`snapshots.kpi_${kpi.key}`)}
      value={formatKpi(kpi.key, kpi.after, locale)}
      tone={toneOf(kpi)}
      hint={
        <>
          {moved && <strong>{moved}</strong>} {t("snapshots.was", { value: formatKpi(kpi.key, kpi.before, locale) })}
        </>
      }
    />
  );
}

function KpiTable({ kpis }: { kpis: SnapshotKpi[] }) {
  const { t, locale } = useI18n();
  const columns: Column<SnapshotKpi>[] = [
    { key: "metric", label: t("snapshots.colMetric"), render: (kpi) => t(`snapshots.kpi_${kpi.key}`), sort: (kpi) => t(`snapshots.kpi_${kpi.key}`) },
    { key: "group", label: t("snapshots.colGroup"), render: (kpi) => <span className="faint small">{t(`snapshots.group_${kpi.group}`)}</span>, sort: (kpi) => kpi.group },
    { key: "before", label: t("snapshots.colBefore"), render: (kpi) => formatKpi(kpi.key, kpi.before, locale), sort: (kpi) => kpi.before, numeric: true },
    { key: "after", label: t("snapshots.colAfter"), render: (kpi) => formatKpi(kpi.key, kpi.after, locale), sort: (kpi) => kpi.after, numeric: true },
    {
      key: "change",
      label: t("snapshots.colChange"),
      render: (kpi) => <span className={toneOf(kpi) ?? "faint"}>{deltaText(kpi, locale) ?? "—"}</span>,
      sort: (kpi) => kpi.percent ?? kpi.change,
      numeric: true,
    },
  ];
  return <DataTable rows={kpis} columns={columns} rowKey={(kpi) => kpi.key} empty={t("snapshots.nothingHere")} labels={tableLabels(t, locale)} />;
}

function Summary({ comparison }: { comparison: SnapshotComparison }) {
  const { t } = useI18n();
  const headline = HEADLINE_KPIS.map((key) => comparison.kpis.find((kpi) => kpi.key === key)).filter((kpi): kpi is SnapshotKpi => Boolean(kpi && kpi.after !== null));
  return (
    <div className="page-stack">
      <ul className="snapshot-summary">
        {comparison.summary.map((sentence) => (
          <li key={sentence.text} className={`tone-${sentence.tone}`}>
            {sentence.text}
          </li>
        ))}
      </ul>
      {headline.length > 0 && (
        <div className="oc-stats">
          {headline.slice(0, 8).map((kpi) => (
            <KpiCard key={kpi.key} kpi={kpi} />
          ))}
        </div>
      )}
      <section className="sub-panel">
        <h4>{t("snapshots.allNumbers")}</h4>
        <KpiTable kpis={comparison.kpis} />
      </section>
    </div>
  );
}

function NotKept() {
  const { t } = useI18n();
  return <p className="faint">{t("snapshots.notKept")}</p>;
}

function Moves<Row>({ title, rows, columns, rowKey }: { title: string; rows: Row[]; columns: Column<Row>[]; rowKey: (row: Row) => string }) {
  const { t, locale } = useI18n();
  return (
    <section className="sub-panel">
      <h4>
        {title} <span className="faint small">{formatNumber(rows.length, locale)}</span>
      </h4>
      <DataTable rows={rows} columns={columns} rowKey={rowKey} empty={t("snapshots.nothingHere")} labels={tableLabels(t, locale)} />
    </section>
  );
}

function position(value: number | null): string {
  return value === null ? "—" : String(value);
}

function KeywordsTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  const keywords = comparison.keywords;
  if (!keywords) return <NotKept />;
  const moved: Column<KeywordMove>[] = [
    { key: "keyword", label: t("snapshots.colKeyword"), render: (row) => <Entity kind="keyword" value={row.keyword}><span className="cell-main">{row.keyword}</span></Entity>, sort: (row) => row.keyword },
    { key: "before", label: t("snapshots.colBefore"), render: (row) => position(row.before), sort: (row) => row.before, numeric: true },
    { key: "after", label: t("snapshots.colAfter"), render: (row) => position(row.after), sort: (row) => row.after, numeric: true },
    { key: "change", label: t("snapshots.colChange"), render: (row) => <span className={(row.change ?? 0) > 0 ? "good" : "bad"}>{placesText(row.change)}</span>, sort: (row) => row.change, numeric: true },
    { key: "traffic", label: t("snapshots.colTraffic"), render: (row) => formatNumber(row.traffic_after, locale), sort: (row) => row.traffic_after, numeric: true },
    { key: "page", label: t("snapshots.colPage"), render: (row) => <span className="faint small">{pathOf(row.url)}</span>, sort: (row) => row.url },
  ];
  const single = (side: "before" | "after"): Column<KeywordMove>[] => [
    moved[0]!,
    { key: "position", label: t("snapshots.colPosition"), render: (row) => position(row[side]), sort: (row) => row[side], numeric: true },
    {
      key: "traffic",
      label: t("snapshots.colTraffic"),
      render: (row) => formatNumber(side === "after" ? row.traffic_after : row.traffic_before, locale),
      sort: (row) => (side === "after" ? row.traffic_after : row.traffic_before),
      numeric: true,
    },
    moved[5]!,
  ];
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat label={t("snapshots.enteredTop10")} value={formatNumber(keywords.entered_top10, locale)} tone={keywords.entered_top10 > 0 ? "good" : undefined} />
        <Stat label={t("snapshots.leftTop10")} value={formatNumber(keywords.left_top10, locale)} tone={keywords.left_top10 > 0 ? "bad" : undefined} />
        <Stat label={t("snapshots.newKeywords")} value={formatNumber(keywords.gained, locale)} />
        <Stat label={t("snapshots.lostKeywords")} value={formatNumber(keywords.lost, locale)} />
        <Stat label={t("snapshots.improvedKeywords")} value={formatNumber(keywords.improved, locale)} />
        <Stat label={t("snapshots.declinedKeywords")} value={formatNumber(keywords.declined, locale)} />
      </div>
      <div className="two-columns">
        <Moves title={t("snapshots.improvedKeywords")} rows={keywords.improved_keywords} columns={moved} rowKey={(row) => row.keyword} />
        <Moves title={t("snapshots.declinedKeywords")} rows={keywords.declined_keywords} columns={moved} rowKey={(row) => row.keyword} />
        <Moves title={t("snapshots.newKeywords")} rows={keywords.new_keywords} columns={single("after")} rowKey={(row) => row.keyword} />
        <Moves title={t("snapshots.lostKeywords")} rows={keywords.lost_keywords} columns={single("before")} rowKey={(row) => row.keyword} />
      </div>
      <p className="faint small">{t("snapshots.keywordsNote")}</p>
    </div>
  );
}

function PagesTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  if (!comparison.pages) return <NotKept />;
  const columns: Column<PageMove>[] = [
    { key: "page", label: t("snapshots.colPage"), render: (row) => <span title={row.url}>{pathOf(row.url)}</span>, sort: (row) => row.url },
    { key: "before", label: t("snapshots.colBefore"), render: (row) => formatNumber(row.before, locale), sort: (row) => row.before, numeric: true },
    { key: "after", label: t("snapshots.colAfter"), render: (row) => formatNumber(row.after, locale), sort: (row) => row.after, numeric: true },
    { key: "change", label: t("snapshots.colChange"), render: (row) => <span className={row.change > 0 ? "good" : "bad"}>{placesText(Math.round(row.change))}</span>, sort: (row) => row.change, numeric: true },
  ];
  return (
    <div className="two-columns">
      <Moves title={t("snapshots.pagesUp")} rows={comparison.pages.up} columns={columns} rowKey={(row) => row.url} />
      <Moves title={t("snapshots.pagesDown")} rows={comparison.pages.down} columns={columns} rowKey={(row) => row.url} />
    </div>
  );
}

function BacklinksTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  const domains = comparison.domains;
  const kpis = comparison.kpis.filter((kpi) => kpi.group === "links");
  if (!domains && kpis.length === 0) return <NotKept />;
  const columns: Column<LinkingDomain>[] = [
    { key: "domain", label: t("snapshots.colDomain"), render: (row) => <Entity kind="domain" value={row.domain}><span className="cell-main">{row.domain}</span></Entity>, sort: (row) => row.domain },
    { key: "rank", label: t("snapshots.colAuthority"), render: (row) => (row.rank === null ? "—" : formatNumber(row.rank, locale)), sort: (row) => row.rank, numeric: true },
  ];
  const riskyColumns: Column<string>[] = [
    {
      key: "domain",
      label: t("snapshots.colDomain"),
      render: (domain) => (
        <Entity kind="domain" value={domain}>
          <span className="cell-main">{domain}</span>
        </Entity>
      ),
      sort: (domain) => domain,
    },
  ];
  return (
    <div className="page-stack">
      {kpis.length > 0 && (
        <div className="oc-stats">
          {kpis.map((kpi) => (
            <KpiCard key={kpi.key} kpi={kpi} />
          ))}
        </div>
      )}
      {domains && (
        <div className="two-columns">
          <Moves title={t("snapshots.domainsGained", { n: formatNumber(domains.gained, locale) })} rows={domains.gained_domains} columns={columns} rowKey={(row) => row.domain} />
          <Moves title={t("snapshots.domainsLost", { n: formatNumber(domains.lost, locale) })} rows={domains.lost_domains} columns={columns} rowKey={(row) => row.domain} />
        </div>
      )}
      {comparison.link_audit && (
        <div className="two-columns">
          <Moves title={t("snapshots.riskyNew")} rows={comparison.link_audit.new_risky} columns={riskyColumns} rowKey={(domain) => domain} />
          <Moves title={t("snapshots.riskyCleared")} rows={comparison.link_audit.cleared} columns={riskyColumns} rowKey={(domain) => domain} />
        </div>
      )}
    </div>
  );
}

function LocalTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  const local = comparison.local;
  const kpis = comparison.kpis.filter((kpi) => kpi.group === "local");
  if (!local && kpis.length === 0) return <NotKept />;
  const rank = (value: number | null) => (value === null ? "—" : formatKpi("local_arp", value, locale));
  const columns: Column<LocalMove>[] = [
    { key: "keyword", label: t("snapshots.colKeyword"), render: (row) => <span className="cell-main">{row.keyword}</span>, sort: (row) => row.keyword },
    { key: "solv", label: t("snapshots.colSolv"), render: (row) => `${formatPercent(row.solv_before, locale)} → ${formatPercent(row.solv_after, locale)}`, sort: (row) => row.solv_after, numeric: true },
    {
      key: "moved",
      label: t("snapshots.colChange"),
      render: (row) => {
        const moved = row.solv_after - row.solv_before;
        return Math.abs(moved) < 0.05 ? "—" : <span className={moved > 0 ? "good" : "bad"}>{`${moved > 0 ? "+" : ""}${formatKpi("local_arp", moved, locale)}`}</span>;
      },
      sort: (row) => row.solv_after - row.solv_before,
      numeric: true,
    },
    { key: "arp", label: t("snapshots.colArp"), render: (row) => `${rank(row.arp_before)} → ${rank(row.arp_after)}`, sort: (row) => row.arp_after, numeric: true },
  ];
  return (
    <div className="page-stack">
      {kpis.length > 0 && (
        <div className="oc-stats">
          {kpis.map((kpi) => (
            <KpiCard key={kpi.key} kpi={kpi} />
          ))}
        </div>
      )}
      {local && <Moves title={t("snapshots.localKeywords", { up: formatNumber(local.improved, locale), down: formatNumber(local.declined, locale) })} rows={local.keywords} columns={columns} rowKey={(row) => row.keyword} />}
    </div>
  );
}

function RankTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  const tracked = comparison.tracked;
  if (!tracked) return <NotKept />;
  const columns: Column<TrackedMove>[] = [
    { key: "keyword", label: t("snapshots.colKeyword"), render: (row) => <span className="cell-main">{row.keyword}</span>, sort: (row) => row.keyword },
    { key: "before", label: t("snapshots.colBefore"), render: (row) => position(row.before), sort: (row) => row.before, numeric: true },
    { key: "after", label: t("snapshots.colAfter"), render: (row) => position(row.after), sort: (row) => row.after, numeric: true },
    { key: "change", label: t("snapshots.colChange"), render: (row) => <span className={(row.change ?? 0) > 0 ? "good" : "bad"}>{placesText(row.change)}</span>, sort: (row) => row.change, numeric: true },
  ];
  const kpis = comparison.kpis.filter((kpi) => kpi.group === "rank");
  return (
    <div className="page-stack">
      <div className="oc-stats">
        {kpis.map((kpi) => (
          <KpiCard key={kpi.key} kpi={kpi} />
        ))}
        <Stat label={t("snapshots.trackedUp")} value={formatNumber(tracked.improved, locale)} tone={tracked.improved > 0 ? "good" : undefined} />
        <Stat label={t("snapshots.trackedDown")} value={formatNumber(tracked.declined, locale)} tone={tracked.declined > 0 ? "bad" : undefined} />
      </div>
      <Moves title={t("snapshots.trackedMoves")} rows={tracked.moves} columns={columns} rowKey={(row) => row.keyword} />
    </div>
  );
}

function AuditTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  const audit = comparison.audit;
  if (!audit) return <NotKept />;
  const columns: Column<IssueMove>[] = [
    { key: "issue", label: t("snapshots.colIssue"), render: (row) => row.title, sort: (row) => row.title },
    { key: "before", label: t("snapshots.colBefore"), render: (row) => formatNumber(row.before, locale), sort: (row) => row.before, numeric: true },
    { key: "after", label: t("snapshots.colAfter"), render: (row) => formatNumber(row.after, locale), sort: (row) => row.after, numeric: true },
  ];
  const kpis = comparison.kpis.filter((kpi) => kpi.group === "audit");
  return (
    <div className="page-stack">
      <div className="oc-stats">
        {kpis.map((kpi) => (
          <KpiCard key={kpi.key} kpi={kpi} />
        ))}
      </div>
      <div className="two-columns">
        <Moves title={t("snapshots.issuesFixed")} rows={audit.fixed} columns={columns} rowKey={(row) => row.id} />
        <Moves title={t("snapshots.issuesNew")} rows={audit.new_issues} columns={columns} rowKey={(row) => row.id} />
        <Moves title={t("snapshots.issuesBetter")} rows={audit.better} columns={columns} rowKey={(row) => row.id} />
        <Moves title={t("snapshots.issuesWorse")} rows={audit.worse} columns={columns} rowKey={(row) => row.id} />
      </div>
    </div>
  );
}

function AiTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  const ai = comparison.ai;
  if (!ai) return <NotKept />;
  const cells = (side: "before" | "after"): Column<CellMove>[] => [
    { key: "prompt", label: t("snapshots.colQuestion"), render: (row) => row.prompt, sort: (row) => row.prompt },
    { key: "engine", label: t("snapshots.colAssistant"), render: (row) => ENGINE_NAMES[row.engine] ?? row.engine, sort: (row) => row.engine },
    { key: "position", label: t("snapshots.colPosition"), render: (row) => position(row[side]), sort: (row) => row[side], numeric: true },
  ];
  const kpis = comparison.kpis.filter((kpi) => kpi.group === "ai");
  return (
    <div className="page-stack">
      <div className="oc-stats">
        {kpis.map((kpi) => (
          <KpiCard key={kpi.key} kpi={kpi} />
        ))}
      </div>
      <div className="two-columns">
        <Moves title={t("snapshots.aiGained")} rows={ai.gained} columns={cells("after")} rowKey={(row) => `${row.engine}-${row.prompt}`} />
        <Moves title={t("snapshots.aiLost")} rows={ai.lost} columns={cells("before")} rowKey={(row) => `${row.engine}-${row.prompt}`} />
      </div>
      {ai.brands.length > 0 && (
        <section className="sub-panel">
          <h4>{t("snapshots.aiBrands")}</h4>
          <ol className="bars domains" aria-label={t("snapshots.aiBrands")}>
            {ai.brands.map((brand) => (
              <li key={brand.key} className={brand.own ? "own" : undefined}>
                <span className="bar-label" title={brand.key}>
                  {brand.key}
                </span>
                <span className="bar-track" aria-hidden="true">
                  <span className="bar-fill" style={{ width: `${Math.min(100, brand.after)}%` }} />
                </span>
                <span className="bar-value">
                  {formatPercent(brand.after, locale)} <small className="faint">{t("snapshots.was", { value: formatPercent(brand.before, locale) })}</small>
                </span>
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  );
}

function CompetitorsTab({ comparison }: { comparison: SnapshotComparison }) {
  const { t, locale } = useI18n();
  if (comparison.sites.length < 2) return <p className="faint">{t("snapshots.noRivals")}</p>;
  const keys = ["traffic", "keywords", "top10", "oa", "referring_domains"];
  return (
    <div className="oc-table-wrap">
      <table className="oc-table snapshot-sites">
        <thead>
          <tr>
            <th>{t("snapshots.colDomain")}</th>
            {keys.map((key) => (
              <th key={key} className="num">
                {t(`snapshots.kpi_${key}`)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {comparison.sites.map((site) => (
            <tr key={site.domain} className={site.own ? "own" : undefined}>
              <td>
                <strong>{site.domain}</strong>
              </td>
              {keys.map((key) => {
                const kpi = site.kpis.find((entry) => entry.key === key);
                const moved = kpi ? deltaText(kpi, locale) : null;
                return (
                  <td key={key} className="num">
                    {kpi ? formatKpi(key, kpi.after, locale) : "—"} {moved && kpi && <small className={toneOf(kpi)}>{moved}</small>}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** A new report that sends the comparison after each snapshot. */
function SendAsReport({ project, onClose }: { project: ProjectRow; onClose: () => void }) {
  const { t } = useI18n();
  const { go } = useShell();
  const setup = useQuery({ queryKey: ["reports", "setup", project.id], queryFn: () => api.reportsSetup(project.id) });
  if (!setup.data) return null;
  const reports: ReportsSetup = setup.data;
  const preset: Partial<ReportSpec> = {
    name: t("snapshots.reportName"),
    widgets: ["comparison", "comparison_previous"],
    schedule: reports.can_email ? "snapshot" : "manual",
  };
  return <ReportDialog project={project} setup={reports} report={null} preset={preset} onClose={onClose} onSaved={() => go({ page: "reports", project: project.id })} />;
}

function ComparePanel({ project, setup }: { project: ProjectRow; setup: SnapshotSetup }) {
  const { t, lang, locale } = useI18n();
  const { fail } = useShell();
  const finished = finishedOf(setup.snapshots);
  const [pair, setPair] = useState(() => defaultPair(setup));
  const [tab, setTab] = useState<Tab>("summary");
  const [sending, setSending] = useState(false);
  const known = pair && finished.some((row) => row.id === pair.from) && finished.some((row) => row.id === pair.to) ? pair : defaultPair(setup);
  const comparison = useQuery({
    queryKey: ["snapshots", "compare", project.id, known?.from, known?.to, lang],
    queryFn: () => api.compareSnapshots(project.id, { from: known!.from, to: known!.to, lang }),
    enabled: known !== null && known.from !== known.to,
  });
  const excel = useMutation({
    mutationFn: () => api.snapshotXlsx(project.id, { from: known!.from, to: known!.to, lang }),
    onSuccess: (file) => downloadBase64(file.name, file.data, file.content_type),
    onError: (thrown) => fail(thrown),
  });
  if (!known) return null;
  const choose = (side: "from" | "to", id: number) => setPair({ ...known, [side]: id });
  const data = comparison.data;
  return (
    <section className="oc-panel report snapshot-panel">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("snapshots.compareTitle")}</h3>
          {data && <span className="faint">{t("snapshots.daysBetween", { n: data.days })}</span>}
        </div>
        <div className="report-actions">
          <Button small busy={excel.isPending} disabled={!data} onClick={() => excel.mutate()}>
            <FileSpreadsheet size={14} />
            {t("snapshots.exportExcel")}
          </Button>
          <Button small onClick={() => setSending(true)}>
            <Send size={14} />
            {t("snapshots.sendAsReport")}
          </Button>
        </div>
      </header>
      <div className="snapshot-pair">
        {(["from", "to"] as const).map((side) => (
          <Field key={side} label={side === "from" ? t("snapshots.compareFrom") : t("snapshots.compareTo")}>
            <select className="oc-select" value={known[side]} onChange={(event) => choose(side, Number(event.target.value))}>
              {finished.map((row) => (
                <option key={row.id} value={row.id}>
                  {sideName(row, row.id === setup.baseline_id, t, locale)}
                </option>
              ))}
            </select>
          </Field>
        ))}
      </div>
      {known.from === known.to && <p className="faint">{t("snapshots.chooseTwo")}</p>}
      <div className="oc-tabs" role="tablist" aria-label={t("snapshots.compareTitle")}>
        {TABS.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => setTab(entry)}>
            {t(`snapshots.tab_${entry}`)}
          </button>
        ))}
      </div>
      <div role="tabpanel" className="snapshot-tab">
        {comparison.isLoading && <Loading label={t("common.loading")} />}
        {comparison.error && <ProblemBanner error={comparison.error} retry={() => void comparison.refetch()} />}
        {data && tab === "summary" && <Summary comparison={data} />}
        {data && tab === "keywords" && <KeywordsTab comparison={data} />}
        {data && tab === "pages" && <PagesTab comparison={data} />}
        {data && tab === "backlinks" && <BacklinksTab comparison={data} />}
        {data && tab === "rank" && <RankTab comparison={data} />}
        {data && tab === "audit" && <AuditTab comparison={data} />}
        {data && tab === "ai" && <AiTab comparison={data} />}
        {data && tab === "local" && <LocalTab comparison={data} />}
        {data && tab === "competitors" && <CompetitorsTab comparison={data} />}
      </div>
      {sending && <SendAsReport project={project} onClose={() => setSending(false)} />}
    </section>
  );
}

export function Snapshots({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { jobs } = useShell();
  const queryClient = useQueryClient();
  const setup = useQuery({ queryKey: ["snapshots", "setup", project.id], queryFn: () => api.snapshotSetup(project.id) });
  const [taking, setTaking] = useState(false);
  const running = jobs.find((job) => job.kind === "snapshot" && job.project_id === project.id && job.status === "running");
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  const finished = finishedOf(data.snapshots);
  return (
    <div className="page-stack">
      <SetupBar project={project} setup={data} running={running} onTake={() => setTaking(true)} />
      {data.snapshots.length === 0 ? (
        <div className="oc-card">
          <Empty icon={<Camera size={28} />} title={t("snapshots.emptyTitle")}>
            <p>{t("snapshots.emptyText", { domain: project.domain })}</p>
            <Button primary disabled={Boolean(running)} onClick={() => setTaking(true)}>
              <Camera size={15} />
              {t("snapshots.takeFirst")}
            </Button>
          </Empty>
        </div>
      ) : (
        <>
          {finished.length === 1 && (
            <div className="oc-banner info" role="status">
              <span>{data.settings.schedule === "manual" ? t("snapshots.oneMoreManual") : t("snapshots.oneMoreAuto")}</span>
            </div>
          )}
          {finished.length > 1 && <ComparePanel key={finished[0]!.id} project={project} setup={data} />}
          <Timeline project={project} setup={data} />
        </>
      )}
      {taking && (
        <TakeDialog
          project={project}
          setup={data}
          competitors={chosenCompetitors(data)}
          onClose={() => {
            setTaking(false);
            refreshSpending(queryClient);
          }}
        />
      )}
    </div>
  );
}
