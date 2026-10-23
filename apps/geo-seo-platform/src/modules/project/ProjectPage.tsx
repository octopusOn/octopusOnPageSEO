// A project's dashboard: every module's headline numbers with their change and a short line,
// the opportunities worth doing first, what the schedule does next and this month's spending.
// The second tab lists every opportunity.

import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowDown, ArrowRight, ArrowUp, CalendarClock, CheckCircle2, CircleX, Lightbulb, Wallet } from "lucide-react";
import { Button, ErrorState, Loading } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { DatedValue, ProjectDashboard, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatCompact, formatNumber, formatPercent, formatUsd, relativeTime } from "../../lib/format";
import type { PageId } from "../../lib/route";
import { Sparkline } from "../../lib/Sparkline";
import { vitalText } from "../audit/measures";
import { moduleOf } from "../registry";
import { AlertsTab, RecentAlerts } from "../alerts/AlertsTab";
import { MissingSources, OpportunitiesTab, OpportunityRow } from "./Opportunities";
import { seriesChange, spendPage } from "./opportunity";

const TABS = ["overview", "opportunities", "alerts"] as const;
type Tab = (typeof TABS)[number];

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

/** A change with its direction; `invert` when lower is better. */
function Delta({ value, percent = false, invert = false }: { value: number | null; percent?: boolean; invert?: boolean }) {
  const { locale } = useI18n();
  if (value === null || !Number.isFinite(value) || value === 0) return null;
  const better = invert ? value < 0 : value > 0;
  return (
    <span className={`rank-change ${better ? "up" : "down"}`}>
      {value > 0 ? <ArrowUp size={12} aria-hidden="true" /> : <ArrowDown size={12} aria-hidden="true" />}
      {percent ? formatPercent(Math.abs(value), locale) : one(Math.abs(value), locale)}
    </span>
  );
}

function percentChange(now: number, before: number | null): number | null {
  if (before === null || before === 0) return null;
  return Math.round(((now - before) / before) * 1000) / 10;
}

/** One module's numbers. */
function Card({ page, title, value, hint, change, series, sparkLabel, invert, children }: { page: PageId; title: string; value: ReactNode; hint?: ReactNode; change?: ReactNode; series?: DatedValue[]; sparkLabel?: string; invert?: boolean; children?: ReactNode }) {
  const { t } = useI18n();
  const { go } = useShell();
  const entry = moduleOf(page);
  const Icon = entry.icon;
  return (
    <section className="dash-card">
      <header>
        <span className={`oc-tile small ${entry.tone}`} aria-hidden="true">
          <Icon size={14} />
        </span>
        <h4>{title}</h4>
        <Button small className="ghost icon" onClick={() => go({ page })} aria-label={t("project.openModule", { module: t(entry.label) })} title={t("project.openModule", { module: t(entry.label) })}>
          <ArrowRight size={14} />
        </Button>
      </header>
      <div className="dash-value">
        <strong>{value}</strong>
        {change}
      </div>
      {hint && <span className="faint small">{hint}</span>}
      {series && <Sparkline points={series} label={sparkLabel ?? title} invert={invert} />}
      {children}
    </section>
  );
}

/** A module without data yet: what to do to fill it. */
function SetUp({ page, title, text }: { page: PageId; title: string; text: string }) {
  const { go } = useShell();
  const entry = moduleOf(page);
  const Icon = entry.icon;
  return (
    <button type="button" className="dash-card empty" onClick={() => go({ page })}>
      <header>
        <span className={`oc-tile small ${entry.tone}`} aria-hidden="true">
          <Icon size={14} />
        </span>
        <h4>{title}</h4>
      </header>
      <span className="faint small">{text}</span>
    </button>
  );
}

function Cards({ data }: { data: ProjectDashboard }) {
  const { t, locale } = useI18n();
  const { organic, links, rank, search, audit, ai, local, vitals, content } = data;
  return (
    <div className="dash-grid">
      {organic ? (
        <Card
          page="overview"
          title={t("project.organic")}
          value={formatCompact(organic.metrics.traffic, locale)}
          change={<Delta value={seriesChange(organic.traffic)} percent />}
          hint={t("project.organicHint", { keywords: formatCompact(organic.metrics.keywords, locale), time: relativeTime(organic.fetched_at, locale) })}
          series={organic.traffic}
        />
      ) : (
        <SetUp page="overview" title={t("project.organic")} text={t("project.organicSetUp")} />
      )}
      {links ? (
        <Card
          page="backlinks"
          title={t("project.links")}
          value={formatCompact(links.links.referring_domains, locale)}
          change={<Delta value={seriesChange(links.referring_domains)} percent />}
          hint={t("project.linksHint", { rank: links.links.rank === null ? "—" : one(links.links.rank, locale), backlinks: formatCompact(links.links.backlinks, locale) })}
          series={links.referring_domains}
        />
      ) : (
        <SetUp page="backlinks" title={t("project.links")} text={t("project.linksSetUp")} />
      )}
      {rank ? (
        <Card
          page="rank"
          title={t("project.rank")}
          value={rank.visibility === null ? "—" : formatPercent(rank.visibility, locale)}
          change={<Delta value={rank.visibility_change} />}
          hint={t("project.rankHint", { top10: formatNumber(rank.top10, locale), checked: formatNumber(rank.checked, locale), position: rank.average_position === null ? "—" : one(rank.average_position, locale) })}
          series={rank.series}
        >
          <span className="dash-moves small">
            <span className="rank-change up">
              <ArrowUp size={12} aria-hidden="true" />
              {formatNumber(rank.improved, locale)}
            </span>
            <span className="rank-change down">
              <ArrowDown size={12} aria-hidden="true" />
              {formatNumber(rank.declined, locale)}
            </span>
            {rank.lost > 0 && <span className="faint">{t("project.rankLost", { n: formatNumber(rank.lost, locale) })}</span>}
          </span>
        </Card>
      ) : (
        <SetUp page="rank" title={t("project.rank")} text={t("project.rankSetUp")} />
      )}
      {search ? (
        <Card
          page="gsc"
          title={t("project.search")}
          value={formatCompact(search.clicks, locale)}
          change={<Delta value={percentChange(search.clicks, search.clicks_before)} percent />}
          hint={t("project.searchHint", { impressions: formatCompact(search.impressions, locale), ctr: formatPercent(search.ctr, locale), position: search.position === null ? "—" : one(search.position, locale) })}
          series={search.series}
        />
      ) : (
        <SetUp page="gsc" title={t("project.search")} text={t("project.searchSetUp")} />
      )}
      {audit ? (
        <Card
          page="audit"
          title={t("project.audit")}
          value={audit.health === null ? "—" : one(audit.health, locale)}
          change={<Delta value={audit.health !== null && audit.health_before !== null ? Math.round((audit.health - audit.health_before) * 10) / 10 : null} />}
          hint={t("project.auditHint", { issues: formatNumber(audit.issues, locale), pages: formatNumber(audit.pages, locale) })}
          series={audit.series}
        />
      ) : (
        <SetUp page="audit" title={t("project.audit")} text={t("project.auditSetUp")} />
      )}
      {ai ? (
        <Card
          page="ai"
          title={t("project.ai")}
          value={formatPercent(ai.mention_rate, locale)}
          change={<Delta value={seriesChange(ai.series)} percent />}
          hint={t("project.aiHint", { share: formatPercent(ai.share_of_voice, locale), answers: formatNumber(ai.answers, locale) })}
          series={ai.series}
        />
      ) : (
        <SetUp page="ai" title={t("project.ai")} text={t("project.aiSetUp")} />
      )}
      {local ? (
        <Card
          page="local"
          title={t("project.local")}
          value={formatPercent(local.solv, locale)}
          change={<Delta value={seriesChange(local.series)} percent />}
          hint={t("project.localHint", { arp: local.arp === null ? "—" : one(local.arp, locale), top3: formatNumber(local.top3, locale), points: formatNumber(local.points, locale) })}
          series={local.series}
        />
      ) : (
        <SetUp page="local" title={t("project.local")} text={t("project.localSetUp")} />
      )}
      {vitals ? (
        <Card page="audit" title={t("project.vitals")} value={vitals.devices[0]?.passes === null || vitals.devices[0] === undefined ? "—" : vitals.devices[0].passes ? t("audit.vitalsPass") : t("audit.vitalsFail")}>
          <ul className="dash-vitals">
            {vitals.devices.map((device) => (
              <li key={device.device}>
                {device.passes === null ? <span className="faint">—</span> : device.passes ? <CheckCircle2 size={13} className="good" aria-hidden="true" /> : <CircleX size={13} className="warn" aria-hidden="true" />}
                <span>{t(`audit.device_${device.device}`)}</span>
                <span className="faint small">
                  LCP {vitalText("lcp", device.lcp, locale)} · INP {vitalText("inp", device.inp, locale)} · CLS {vitalText("cls", device.cls, locale)}
                </span>
              </li>
            ))}
          </ul>
        </Card>
      ) : (
        <SetUp page="audit" title={t("project.vitals")} text={t("project.vitalsSetUp")} />
      )}
      {content.briefs > 0 || content.scored_pages > 0 ? (
        <Card
          page="content"
          title={t("project.content")}
          value={content.average_score === null ? formatNumber(content.briefs, locale) : one(content.average_score, locale)}
          hint={t("project.contentHint", { briefs: formatNumber(content.briefs, locale), pages: formatNumber(content.scored_pages, locale) })}
        />
      ) : (
        <SetUp page="content" title={t("project.content")} text={t("project.contentSetUp")} />
      )}
    </div>
  );
}

function Overview({ project }: { project: ProjectRow }) {
  const { t, lang, locale } = useI18n();
  const { go, params } = useShell();
  const dashboard = useQuery({ queryKey: ["project-dashboard", project.id, lang], queryFn: () => api.projectDashboard(project.id, lang) });
  if (dashboard.isLoading) return <Loading label={t("project.loading")} />;
  if (dashboard.error || !dashboard.data) return <ErrorState title={t("common.loadFailed")} message={explain(dashboard.error, t).message} retry={() => void dashboard.refetch()} retryLabel={t("common.retry")} />;
  const data = dashboard.data;
  return (
    <div className="page-stack">
      <Cards data={data} />
      <div className="dash-columns">
        <section className="oc-panel report">
          <header className="report-head">
            <div className="report-title">
              <h3 className="oc-panel-title">
                <Lightbulb size={15} aria-hidden="true" /> {t("project.topOpportunities")}
              </h3>
              <span className="faint">{t("opportunities.summary", { n: formatNumber(data.opportunities_open, locale), gain: formatNumber(Math.round(data.opportunities_gain), locale) })}</span>
            </div>
            <div className="report-actions">
              <Button small onClick={() => go({ params: { ...params, tab: "opportunities" } })}>
                {t("project.allOpportunities")}
              </Button>
            </div>
          </header>
          {data.opportunities.length ? (
            <ul className="opportunity-list compact">
              {data.opportunities.map((item) => (
                <OpportunityRow key={item.key} project={project} item={item} compact />
              ))}
            </ul>
          ) : (
            <p className="faint">{t("opportunities.noneText")}</p>
          )}
        </section>
        <div className="page-stack">
          <section className="oc-panel report">
            <header className="report-head">
              <div className="report-title">
                <h3 className="oc-panel-title">
                  <CalendarClock size={15} aria-hidden="true" /> {t("project.schedule")}
                </h3>
              </div>
            </header>
            {data.schedule.length ? (
              <ul className="check-list">
                {data.schedule.map((item) => (
                  <li key={item.module}>
                    <span>{t(moduleOf(item.module).label)}</span>
                    <span className="faint small">{relativeTime(item.at, locale)}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="faint small">{t("project.noSchedule")}</p>
            )}
          </section>
          <section className="oc-panel report">
            <header className="report-head">
              <div className="report-title">
                <h3 className="oc-panel-title">
                  <Wallet size={15} aria-hidden="true" /> {t("project.spend")}
                </h3>
                <span className="faint">{formatUsd(data.spend_total, locale)}</span>
              </div>
            </header>
            {data.spend.length ? (
              <ul className="check-list">
                {data.spend.map(([module, usd]) => (
                  <li key={module}>
                    <span>{spendPage(module) === "dashboard" ? module : t(moduleOf(spendPage(module)).label)}</span>
                    <strong>{formatUsd(usd, locale)}</strong>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="faint small">{t("project.noSpend")}</p>
            )}
          </section>
          <RecentAlerts project={project} />
        </div>
      </div>
      <MissingSourcesHint project={project} />
    </div>
  );
}

/** Asks for the opportunity sources once more cheaply from the list's cache. */
function MissingSourcesHint({ project }: { project: ProjectRow }) {
  const { lang } = useI18n();
  const list = useQuery({ queryKey: ["opportunities", project.id, lang, false], queryFn: () => api.opportunities(project.id, lang, false), staleTime: 60_000 });
  return list.data ? <MissingSources sources={list.data.sources} /> : null;
}

export function ProjectPage({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go, params } = useShell();
  const tab: Tab = (TABS as readonly string[]).includes(params.tab ?? "") ? (params.tab as Tab) : "overview";
  const labels: Record<Tab, string> = { overview: t("project.tabOverview"), opportunities: t("project.tabOpportunities"), alerts: t("project.tabAlerts") };
  return (
    <div className="page-stack">
      <div className="oc-tabs" role="tablist" aria-label={t("common.navProject")}>
        {TABS.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => go({ params: { ...params, tab: entry } })}>
            {labels[entry]}
          </button>
        ))}
      </div>
      {tab === "overview" ? <Overview project={project} /> : tab === "opportunities" ? <OpportunitiesTab project={project} /> : <AlertsTab project={project} />}
    </div>
  );
}
