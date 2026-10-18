import {
  ArrowUpRight,
  Globe2,
  FileCheck2,
  CircleAlert,
  ExternalLink,
  ShieldCheck,
  ArrowRight,
  Link2,
  Timer,
  Layers3,
  Monitor,
} from "lucide-react";
import type { CrawlMeta, Overview as OverviewData, ResponseGroup } from "../api/types";
import { issueById, priorityOrder } from "../catalog";
import { useI18n, number, duration } from "../i18n";
import { Button } from "../components/ui";
import { responseGroups } from "../lib/responses";
import { effectiveViewport } from "../lib/viewport";
import { SeoAnatomy } from "../components/SeoAnatomy";
import { SeoScoreCard } from "../components/SeoScoreCard";

export function Overview({
  meta,
  data,
  excluded,
  onIssue,
  onExplore,
  onIssues,
  onResponse,
}: {
  meta: CrawlMeta;
  data: OverviewData;
  excluded: string[];
  onIssue: (id: string) => void;
  onExplore: () => void;
  onIssues: () => void;
  onResponse: (group: ResponseGroup) => void;
}) {
  const { t, lang } = useI18n();
  const hasScorePlan = !!data.seo_score && [2,3,4,5].includes(data.seo_score.version) && data.seo_score.value !== null;
  const fmt = (n: number) => number(n, lang);
  const viewport = meta.config.rendering?.mode === "javascript" ? effectiveViewport(meta.config) : null;
  const issues = data.issues
    .filter((i) => !excluded.includes(i.id))
    .sort(
      (a, b) =>
        (priorityOrder[issueById.get(a.id)?.priority ?? "low"] ?? 2) -
          (priorityOrder[issueById.get(b.id)?.priority ?? "low"] ?? 2) ||
        b.count - a.count,
    );
  const findings = issues.reduce((sum, i) => sum + i.count, 0);
  const percent = data.html
    ? Math.round((data.indexable / data.html) * 100)
    : 0;
  const total = data.internal + data.external;
  const responsePercent = (count: number) => {
    const value = total ? (count / total) * 100 : 0;
    return value > 0 && value < 0.1 ? `<${fmt(0.1)}` : fmt(value);
  };
  const signals = responseGroups.map((group) => ({...group, count: data.responses?.find((row) => row.group === group.group)?.count ?? 0}));
  return (
    <div className="overview-page">
      <SeoScoreCard score={data.seo_score} meta={meta} onIssue={onIssue} onIssues={onIssues}/>
      <div className="metric-grid">
        {[
          {
            label: t("internal"),
            value: fmt(data.internal),
            hint: `${fmt(data.html)} ${t("htmlPages")}`,
            icon: Globe2,
            tone: "teal",
          },
          {
            label: t("indexable"),
            value: fmt(data.indexable),
            hint: `${percent}% · ${t("htmlPages")}`,
            icon: FileCheck2,
            tone: "blue",
          },
          {
            label: t("findings"),
            value: fmt(findings),
            hint: `${fmt(issues.length)} ${t("checks")}`,
            icon: CircleAlert,
            tone: "orange",
          },
          {
            label: t("external"),
            value: fmt(data.external),
            hint: `${fmt(data.average_response_ms)} ms · ${t("responseTime")}`,
            icon: ExternalLink,
            tone: "violet",
          },
        ].map(({ label, value, hint, icon: Icon, tone }) => (
          <article className="oc-stat" key={label}>
            <div className="row between">
              <span className="oc-stat-label">{label}</span>
              <span className={`oc-tile ${tone}`}>
                <Icon size={16} />
              </span>
            </div>
            <strong className="oc-stat-value">{value}</strong>
            <div className="oc-stat-hint">{hint}</div>
          </article>
        ))}
      </div>
      <SeoAnatomy crawlId={meta.id} score={data.seo_score} onIssue={onIssue}/>
      <div className="overview-middle">
        <section className="oc-panel signals-panel">
          <header className="panel-heading">
            <h2>{t("indexability")}</h2>
            <span className="oc-badge good">
              <ShieldCheck size={12} />
              {t(
                meta.config.rendering?.mode === "javascript"
                  ? "javascript"
                  : "rawHtml",
              )}
            </span>
          </header>
          <div className="index-content">
            <div
              className="index-ring"
              style={{
                background: `conic-gradient(var(--chart-1) ${Math.min(100, percent)}%,var(--surface-2) 0)`,
              }}
            >
              <div>
                <strong>
                  {percent}
                  <small>%</small>
                </strong>
                <span>{t("indexableLabel")}</span>
              </div>
            </div>
            <div className="index-legend">
              <div>
                <i className="dot good-bg" />
                <span>{t("indexableLabel")}</span>
                <b>{fmt(data.indexable)}</b>
              </div>
              <div>
                <i className="dot muted-bg" />
                <span>{t("nonIndexable")}</span>
                <b>{fmt(Math.max(0, data.html - data.indexable))}</b>
              </div>
              <button className="text-button" onClick={onExplore}>
                {t("explorer")}
                <ArrowRight size={14} />
              </button>
            </div>
          </div>
        </section>
        <section className="oc-panel response-panel">
          <header className="panel-heading">
            <h2>{t("responseCodes")}</h2>
            <span className="faint mono">HTTP</span>
          </header>
          <div className="response-total">
            <strong>{fmt(total)}</strong>
            <span>{t("urls")}</span>
          </div>
          {data.responses ? <>
          <div className="stack-bar" aria-label={t("responseCodes")}>
            {signals.filter((s) => s.count > 0).map((s) => (
              <span
                key={s.group}
                title={`${t(s.label)}: ${fmt(s.count)}`}
                style={{
                  flexGrow: s.count,
                  background: s.color,
                }}
              />
            ))}
          </div>
          <div className="response-legend">
            {signals.map((s) => (
              <button key={s.group} disabled={!s.count} onClick={() => onResponse(s.group)} aria-label={`${t(s.label)}: ${fmt(s.count)} ${t("urls")}`}>
                <i className="dot" style={{ background: s.color }} />
                <span>{t(s.label)}</span>
                <b>{fmt(s.count)}</b>
                <small>
                  {responsePercent(s.count)}%
                </small>
                <ArrowRight size={12}/>
              </button>
            ))}
          </div>
          <p className="response-note">{t("responseGroupsNote")}</p>
          </> : <p className="response-note">{t("responseGroupsUnavailable")}</p>}
        </section>
      </div>
      <div className="overview-bottom">
        <section className="oc-panel priority-panel">
          <header className="panel-heading">
            <div>
              <span className="eyebrow">{t("technicalHealth")}</span>
              <h2>{t(hasScorePlan ? "seoArchiveFindings" : "priorities")}</h2>
            </div>
            <Button small className="ghost" onClick={onIssues}>
              {t("allIssues")}
              <ArrowUpRight size={14} />
            </Button>
          </header>
          {hasScorePlan && <p className="response-note">{t("seoArchiveFindingsNote")}</p>}
          <div className="priority-list">
            {issues.slice(0, 5).map((item) => {
              const issue = issueById.get(item.id);
              if (!issue) return null;
              return (
                <button key={item.id} onClick={() => onIssue(item.id)}>
                  <span className={`priority-mark ${issue.priority}`}>
                    <CircleAlert size={15} />
                  </span>
                  <span className="priority-name">
                    {issue[lang].title}
                    <small>
                      {t(issue.priority as "high" | "medium" | "low")} ·{" "}
                      {t(issue.kind as "issue" | "warning" | "opportunity")}
                    </small>
                  </span>
                  <span className="issue-count">
                    {fmt(item.count)}
                    <small>{t("urls")}</small>
                  </span>
                  <ArrowRight size={15} className="faint" />
                </button>
              );
            })}
            {!issues.length && <p className="muted padded">{t("noIssues")}</p>}
          </div>
        </section>
        <section className="oc-panel crawl-summary">
          <header className="panel-heading">
            <h2>{t("crawlSummary")}</h2>
            <Layers3 size={17} className="faint" />
          </header>
          <dl>
            <div>
              <dt>
                <Globe2 size={14} />
                {t("crawled")}
              </dt>
              <dd>
                {fmt(meta.progress.crawled)} {t("urls")}
              </dd>
            </div>
            <div>
              <dt>
                <Timer size={14} />
                {t("duration")}
              </dt>
              <dd>{duration(meta.progress.elapsed_ms)}</dd>
            </div>
            <div>
              <dt>
                <Link2 size={14} />
                {t("speed")}
              </dt>
              <dd>
                {fmt(
                  meta.progress.crawled /
                    Math.max(1, meta.progress.elapsed_ms / 1000),
                )}
              </dd>
            </div>
            <div>
              <dt>
                <ShieldCheck size={14} />
                {t("robotsPolicy")}
              </dt>
              <dd>
                {meta.config.robots === "respect"
                  ? t("respectRobots")
                  : meta.config.robots}
              </dd>
            </div>
            {viewport && <div>
              <dt><Monitor size={14}/>{t("renderViewport")}</dt>
              <dd>{viewport.width} × {viewport.height} · {t(viewport.mobile ? "viewportMobile" : "viewportDesktop")}</dd>
            </div>}
          </dl>
          <div className="summary-foot">
            <ShieldCheck size={16} />
            <span>{t("nativeNote")}</span>
          </div>
        </section>
      </div>
    </div>
  );
}
