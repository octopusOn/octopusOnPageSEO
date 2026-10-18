import { useState } from "react";
import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  FileWarning,
  Globe,
  Play,
  Plus,
  RefreshCw,
  ServerCrash,
  Sparkles,
} from "lucide-react";
import type {
  FinderJob,
  FindingCategory,
  IndexEngine,
  ScanConfig,
  ScanOverview,
  VerifiedUrl,
} from "../api/types";
import { computeScanProgress } from "../api/demo";
import { useI18n, number, percent, usd, type Key } from "../i18n";

const QUICK_ENGINES: { id: IndexEngine; label: string }[] = [
  { id: "google", label: "Google" },
  { id: "yandex", label: "Yandex" },
  { id: "bing", label: "Bing" },
  { id: "duckduckgo", label: "DuckDuckGo" },
];

export function categoryBadgeClass(cat: FindingCategory): string {
  switch (cat) {
    case "error_404":
    case "error_5xx":
      return "oc-badge critical";
    case "error_4xx":
    case "soft_404":
    case "thin_200":
      return "oc-badge warning";
    case "network_error":
      return "oc-badge info";
    case "healthy_200":
      return "oc-badge good";
  }
}

export function Overview({
  overview,
  config,
  onChangeConfig,
  runningJob,
  isScanning = false,
  onOpenExplorer,
  onInspectUrl,
  onNewScan,
  onQuickStartScan,
  onOpenRedirects,
  onReverify,
  onUpdateTarget,
}: {
  overview: ScanOverview;
  config?: ScanConfig;
  onChangeConfig?: (next: ScanConfig) => void;
  runningJob?: FinderJob | null;
  isScanning?: boolean;
  onOpenExplorer: (category: string, engine?: string, searchPrefix?: string) => void;
  onInspectUrl: (row: VerifiedUrl) => void;
  onNewScan: (domainOverride?: string) => void;
  onQuickStartScan?: (domain: string) => void;
  onOpenRedirects: () => void;
  onReverify: () => void;
  onUpdateTarget?: (url: string, target: string) => void;
}) {
  const { t, lang } = useI18n();
  const [quickDomain, setQuickDomain] = useState("");
  const { meta, health_score, engines, clusters = [], top_issues } = overview;
  const other4xx5xx = meta.count_4xx + meta.count_5xx;
  const thinAndSoft = meta.count_soft_404 + meta.count_thin_200;
  const isSampleScan = meta.id.startsWith("finder-demo-");
  const progressInfo = runningJob ? computeScanProgress(runningJob, t) : null;

  const toggleQuickEngine = (eng: IndexEngine) => {
    if (!config || !onChangeConfig || isScanning) return;
    const nextEngines = config.engines.includes(eng)
      ? config.engines.filter((e) => e !== eng)
      : [...config.engines, eng];
    onChangeConfig({ ...config, engines: nextEngines });
  };

  return (
    <div className="finder-page">
      <div className="oc-pagehead">
        <div className="oc-pagehead-text">
          <div className="row" style={{ gap: 10 }}>
            <h1 className="oc-pagehead-title">{t("overview")}</h1>
            {isSampleScan && (
              <span className="oc-badge warning">{t("sampleReport")}</span>
            )}
          </div>
          <p className="oc-pagehead-lead">
            {meta.domain} · {number(meta.verified_urls, lang)}{" "}
            {t("statDiscovered")} · {meta.queries_executed} SERP
            {meta.credits && meta.credits.sources.length > 0 && (
              <>
                {" "}
                · {t("creditsUsed")}: {number(meta.credits.requests, lang)}{" "}
                {t("creditRequests")},{" "}
                {usd(meta.credits.charged_micros / 1_000_000, lang)}
              </>
            )}
          </p>
          {meta.credits?.stopped && (
            <div className="oc-banner warning" style={{ marginTop: 8 }}>
              {t("creditsStopped")}: {meta.credits.stopped}
            </div>
          )}
        </div>
        <div className="oc-pagehead-actions">
          <button className="oc-btn" onClick={onReverify} disabled={isScanning}>
            <RefreshCw size={15} />
            {t("reverifyIssues")}
          </button>
          <button className="oc-btn" onClick={onOpenRedirects}>
            {t("redirectRules")}
          </button>
          <button
            className="oc-btn primary"
            onClick={() => onNewScan(quickDomain.trim() || undefined)}
            disabled={isScanning}
          >
            <Plus size={16} />
            {isScanning ? t("scanRunning") : t("newScan")}
          </button>
        </div>
      </div>

      {onQuickStartScan && (
        <div className="oc-panel" style={{ marginBottom: 16 }}>
          <form
            className="row between"
            style={{ gap: 12, flexWrap: "wrap" }}
            onSubmit={(e) => {
              e.preventDefault();
              if (isScanning) return;
              if (quickDomain.trim()) {
                onQuickStartScan(quickDomain.trim());
                setQuickDomain("");
              } else {
                onNewScan();
              }
            }}
          >
            <div style={{ flex: "1 1 280px" }}>
              <div className="type-card-title" style={{ marginBottom: 4 }}>
                {t("quickStartTitle")}
              </div>
              <div className="muted" style={{ fontSize: 12 }}>
                {t("quickStartLead")}
              </div>
            </div>
            <div className="row" style={{ gap: 8, flex: "1 1 380px", justifyContent: "flex-end" }}>
              <input
                className="oc-input mono"
                style={{ maxWidth: 280 }}
                value={quickDomain}
                disabled={isScanning}
                onChange={(e) => setQuickDomain(e.target.value)}
                placeholder={
                  runningJob
                    ? t("scanningDomain", { domain: runningJob.domain })
                    : t("quickDomainPlaceholder")
                }
              />
              <button
                type="button"
                className="oc-btn"
                disabled={isScanning}
                onClick={() => onNewScan(quickDomain.trim() || undefined)}
              >
                {t("detailedSettings")}
              </button>
              <button
                type="submit"
                className="oc-btn primary"
                disabled={isScanning}
              >
                <Play size={14} />
                {progressInfo
                  ? t("scanningPercent", {
                      percent: percent(progressInfo.percent, lang),
                    })
                  : isScanning
                    ? t("startingScan")
                    : t("scanNow")}
              </button>
            </div>
          </form>

          {config && onChangeConfig && (
            <div
              className="row between"
              style={{
                marginTop: 12,
                paddingTop: 12,
                borderTop: "1px solid var(--oc-line-subtle)",
                gap: 14,
                flexWrap: "wrap",
              }}
            >
              <div className="row" style={{ gap: 12, flexWrap: "wrap" }}>
                <span className="muted" style={{ fontSize: 12, fontWeight: 600 }}>
                  {t("quickEngines")}
                </span>
                {QUICK_ENGINES.map((eng) => (
                  <label key={eng.id} className="oc-check" style={{ fontSize: 12.5 }}>
                    <input
                      type="checkbox"
                      disabled={isScanning}
                      checked={config.engines.includes(eng.id)}
                      onChange={() => toggleQuickEngine(eng.id)}
                    />
                    <span>{eng.label}</span>
                  </label>
                ))}
              </div>

              <div className="row" style={{ gap: 12, flexWrap: "wrap" }}>
                <span className="muted" style={{ fontSize: 12, fontWeight: 600 }}>
                  {t("quickExtras")}
                </span>
                <label className="oc-check" style={{ fontSize: 12.5 }}>
                  <input
                    type="checkbox"
                    disabled={isScanning}
                    checked={config.use_sitemaps}
                    onChange={(e) =>
                      onChangeConfig({ ...config, use_sitemaps: e.target.checked })
                    }
                  />
                  <span>{t("quickSitemaps")}</span>
                </label>
                <label className="oc-check" style={{ fontSize: 12.5 }}>
                  <input
                    type="checkbox"
                    disabled={isScanning}
                    checked={config.use_wayback}
                    onChange={(e) =>
                      onChangeConfig({ ...config, use_wayback: e.target.checked })
                    }
                  />
                  <span>Wayback</span>
                </label>
                <label className="oc-check" style={{ fontSize: 12.5 }}>
                  <input
                    type="checkbox"
                    disabled={isScanning}
                    checked={config.use_commoncrawl}
                    onChange={(e) =>
                      onChangeConfig({ ...config, use_commoncrawl: e.target.checked })
                    }
                  />
                  <span>Common Crawl</span>
                </label>
                <label className="oc-check" style={{ fontSize: 12.5 }}>
                  <input
                    type="checkbox"
                    disabled={isScanning}
                    checked={config.expand_subpaths}
                    onChange={(e) =>
                      onChangeConfig({ ...config, expand_subpaths: e.target.checked })
                    }
                  />
                  <span>{t("quickSubpaths")}</span>
                </label>
                <label className="oc-check" style={{ fontSize: 12.5 }}>
                  <input
                    type="checkbox"
                    disabled={isScanning}
                    checked={config.include_subdomains}
                    onChange={(e) =>
                      onChangeConfig({
                        ...config,
                        include_subdomains: e.target.checked,
                      })
                    }
                  />
                  <span>{t("quickSubdomains")}</span>
                </label>
              </div>
            </div>
          )}

          {progressInfo && runningJob && (
            <div style={{ marginTop: 12 }}>
              <div
                className="scan-progress-track"
                role="progressbar"
                aria-valuenow={progressInfo.percent}
                aria-valuemin={0}
                aria-valuemax={100}
              >
                <div
                  className="scan-progress-fill"
                  style={{ width: `${progressInfo.percent}%` }}
                />
              </div>
              <div className="scan-progress-meta">
                <span>
                  <b>{progressInfo.phaseTitle}</b> · {progressInfo.detailText}
                </span>
                <span className="mono">
                  {runningJob.progress.current_item || "..."} (
                  {percent(progressInfo.percent, lang)})
                </span>
              </div>
            </div>
          )}
        </div>
      )}

      <div className="oc-stats">
        <div
          className="oc-stat interactive-card"
          onClick={() => onOpenExplorer("all")}
        >
          <div className="oc-stat-label">{t("statDiscovered")}</div>
          <div className="oc-stat-value oc-num">
            {number(meta.verified_urls, lang)}
          </div>
          <div className="oc-stat-hint">
            Google, Yandex, Bing, DDG, Wayback, CC, Sitemap
          </div>
        </div>

        <div
          className="oc-stat critical interactive-card"
          onClick={() => onOpenExplorer("error_404")}
        >
          <div className="oc-stat-label">{t("stat404")}</div>
          <div className="oc-stat-value oc-num">
            {number(meta.count_404, lang)}
          </div>
          <div className="oc-stat-hint">HTTP 404 & 410 + 301 → 404</div>
        </div>

        <div
          className="oc-stat warning interactive-card"
          onClick={() => onOpenExplorer("thin_soft_404")}
        >
          <div className="oc-stat-label">{t("statThinSoft404")}</div>
          <div className="oc-stat-value oc-num">
            {number(thinAndSoft, lang)}
          </div>
          <div className="oc-stat-hint">
            {t("wordsBelow", { count: meta.config.min_word_count })} / Soft 404
          </div>
        </div>

        <div
          className="oc-stat interactive-card"
          onClick={() => onOpenExplorer("error_4xx")}
        >
          <div className="oc-stat-label">{t("statOther4xx5xx")}</div>
          <div className="oc-stat-value oc-num">
            {number(other4xx5xx, lang)}
          </div>
          <div className="oc-stat-hint">
            4xx ({meta.count_4xx}) · 5xx ({meta.count_5xx})
          </div>
        </div>

        <div className="oc-stat good">
          <div className="oc-stat-label">{t("statHealthScore")}</div>
          <div className="oc-stat-value oc-num">
            {percent(health_score, lang)}
          </div>
          <div className="oc-stat-hint">
            {t("healthyCount", { count: number(meta.count_healthy, lang) })}
          </div>
        </div>
      </div>

      <div className="overview-two-col">
        <div className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="type-card-title">{t("engineBreakdown")}</h2>
          </div>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t("colEngines")}</th>
                  <th className="num">{t("statDiscovered")}</th>
                  <th className="num">{t("tabIssues")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {engines.map((eng) => (
                  <tr
                    key={eng.engine}
                    className="clickable-row"
                    onClick={() => onOpenExplorer("issues", eng.engine)}
                  >
                    <td>
                      <span className="engine-pill">{eng.engine}</span>
                    </td>
                    <td className="num">{number(eng.total_urls, lang)}</td>
                    <td className="num">
                      {eng.issue_urls > 0 ? (
                        <span className="oc-badge critical">
                          {number(eng.issue_urls, lang)}
                        </span>
                      ) : (
                        <span className="oc-badge good">0</span>
                      )}
                    </td>
                    <td className="num">
                      <ArrowRight size={14} className="muted" />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        <div className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="type-card-title">{t("detectionRulesTitle")}</h2>
          </div>
          <div className="category-summary-list">
            <button
              className="category-summary-row"
              onClick={() => onOpenExplorer("error_404")}
            >
              <FileWarning size={18} className="critical-icon" />
              <div>
                <b>{t("cat_error_404")}</b>
                <small>{t("reason_http_404")}</small>
              </div>
              <span className="oc-badge critical">{meta.count_404}</span>
            </button>

            <button
              className="category-summary-row"
              onClick={() => onOpenExplorer("soft_404")}
            >
              <AlertTriangle size={18} className="warning-icon" />
              <div>
                <b>{t("cat_soft_404")}</b>
                <small>{t("reason_soft_404_phrase")}</small>
              </div>
              <span className="oc-badge warning">{meta.count_soft_404}</span>
            </button>

            <button
              className="category-summary-row"
              onClick={() => onOpenExplorer("thin_200")}
            >
              <Sparkles size={18} className="warning-icon" />
              <div>
                <b>
                  {t("cat_thin_200")} (
                  {t("wordsBelow", { count: meta.config.min_word_count })})
                </b>
                <small>{t("reason_thin_content_200")}</small>
              </div>
              <span className="oc-badge warning">{meta.count_thin_200}</span>
            </button>

            <button
              className="category-summary-row"
              onClick={() => onOpenExplorer("error_5xx")}
            >
              <ServerCrash size={18} className="critical-icon" />
              <div>
                <b>
                  {t("cat_error_4xx")} / {t("cat_error_5xx")}
                </b>
                <small>
                  4xx: {meta.count_4xx} · 5xx: {meta.count_5xx}
                </small>
              </div>
              <span className="oc-badge info">{other4xx5xx}</span>
            </button>

            <button
              className="category-summary-row"
              onClick={() => onOpenExplorer("healthy_200")}
            >
              <CheckCircle2 size={18} className="good-icon" />
              <div>
                <b>{t("cat_healthy_200")}</b>
                <small>{t("reason_healthy_200")}</small>
              </div>
              <span className="oc-badge good">{meta.count_healthy}</span>
            </button>
          </div>
        </div>
      </div>

      {clusters.length > 0 && (
        <div className="oc-panel" style={{ marginTop: 18 }}>
          <div className="oc-panel-head">
            <h2 className="type-card-title">{t("clustersTitle")}</h2>
          </div>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t("colPrefix")}</th>
                  <th className="num">{t("statDiscovered")}</th>
                  <th className="num">{t("stat404")}</th>
                  <th className="num">{t("statThinSoft404")}</th>
                  <th className="num">{t("statOther4xx5xx")}</th>
                  <th className="num">{t("colAvgWords")}</th>
                  <th className="num">{t("colTime")}</th>
                  <th>{t("colActionHint")}</th>
                </tr>
              </thead>
              <tbody>
                {clusters.map((cl) => (
                  <tr
                    key={cl.prefix}
                    className="clickable-row"
                    onClick={() =>
                      onOpenExplorer(
                        cl.issue_urls > 0 ? "issues" : "all",
                        undefined,
                        cl.prefix,
                      )
                    }
                  >
                    <td className="mono">
                      <b>{cl.prefix}/*</b>
                    </td>
                    <td className="num">{number(cl.total_urls, lang)}</td>
                    <td className="num">
                      {cl.count_404 > 0 ? (
                        <span className="oc-badge critical">
                          {number(cl.count_404, lang)}
                        </span>
                      ) : (
                        <span className="muted">0</span>
                      )}
                    </td>
                    <td className="num">
                      {cl.count_soft_thin > 0 ? (
                        <span className="oc-badge warning">
                          {number(cl.count_soft_thin, lang)}
                        </span>
                      ) : (
                        <span className="muted">0</span>
                      )}
                    </td>
                    <td className="num">
                      {cl.count_4xx_5xx > 0 ? (
                        <span className="oc-badge info">
                          {number(cl.count_4xx_5xx, lang)}
                        </span>
                      ) : (
                        <span className="muted">0</span>
                      )}
                    </td>
                    <td className="num mono">
                      {number(cl.avg_word_count, lang)}
                    </td>
                    <td className="num mono">{cl.avg_response_ms} ms</td>
                    <td>
                      <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
                        <span
                          className={`oc-badge ${
                            cl.action_hint === "healthy_section"
                              ? "good"
                              : cl.action_hint === "redirect_section_301"
                                ? "critical"
                                : "warning"
                          }`}
                        >
                          {t(`hint_${cl.action_hint}` as Key)}
                        </span>
                        {cl.issue_urls > 0 &&
                          cl.prefix !== "/" &&
                          onUpdateTarget && (
                            <button
                              className="oc-btn small ghost"
                              onClick={(e) => {
                                e.stopPropagation();
                                onUpdateTarget(
                                  `${cl.prefix}/*`,
                                  `https://${meta.domain}${cl.prefix}/`,
                                );
                              }}
                            >
                              {t("applyCluster301Btn")}
                            </button>
                          )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      <div className="oc-panel" style={{ marginTop: 18 }}>
        <div className="oc-panel-head row between">
          <h2 className="type-card-title">{t("topIssuesTitle")}</h2>
          <button
            className="oc-btn small"
            onClick={() => onOpenExplorer("issues")}
          >
            {t("explorer")}
            <ArrowRight size={14} />
          </button>
        </div>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t("colUrl")}</th>
                <th>{t("colEngines")}</th>
                <th className="num">{t("colStatus")}</th>
                <th>{t("colCategory")}</th>
                <th className="num">{t("colWords")}</th>
                <th>{t("colTitle")}</th>
              </tr>
            </thead>
            <tbody>
              {top_issues.map((row) => (
                <tr
                  key={row.url}
                  className="clickable-row"
                  onClick={() => onInspectUrl(row)}
                >
                  <td className="mono url-cell">{row.url}</td>
                  <td>
                    <div className="engine-badges">
                      {row.engines
                        .split(",")
                        .filter(Boolean)
                        .map((eng) => (
                          <span key={eng} className="engine-pill small">
                            <Globe size={11} />
                            {eng}
                          </span>
                        ))}
                    </div>
                  </td>
                  <td className="num mono">
                    {row.reason === "checking"
                      ? "⏳"
                      : row.redirect_hops > 0
                        ? `${row.initial_status} → ${row.final_status}`
                        : row.final_status}
                  </td>
                  <td>
                    {row.reason === "checking" ? (
                      <span className="oc-badge info">{t("checkPending")}</span>
                    ) : (
                      <span className={categoryBadgeClass(row.category)}>
                        {t(`cat_${row.category}` as Key)}
                      </span>
                    )}
                  </td>
                  <td className="num">{number(row.word_count, lang)}</td>
                  <td className="truncate-cell">
                    {row.serp_title || row.live_title || "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
