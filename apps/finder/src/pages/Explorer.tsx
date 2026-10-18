import { useEffect, useState } from "react";
import {
  ArrowDownToLine,
  Check,
  Copy,
  ExternalLink,
  Globe,
  RefreshCw,
  Search,
  Sparkles,
  X,
} from "lucide-react";
import { api } from "../api/client";
import type {
  FinderTablePage,
  RedirectCandidate,
  VerifiedUrl,
} from "../api/types";
import { useI18n, number, percent, type Key } from "../i18n";
import { categoryBadgeClass } from "./Overview";

const TABS = [
  { id: "issues", label: "tabIssues" as Key },
  { id: "error_404", label: "tab404" as Key },
  { id: "thin_soft_404", label: "tabThinSoft" as Key },
  { id: "error_4xx", label: "tab4xx" as Key },
  { id: "error_5xx", label: "tab5xx" as Key },
  { id: "sitemap_issues", label: "tabSitemapIssues" as Key },
  { id: "orphan_indexed", label: "tabOrphanIndexed" as Key },
  { id: "redirected", label: "tabRedirected" as Key },
  { id: "noindex", label: "tabNoindex" as Key },
  { id: "all", label: "tabAll" as Key },
];

const ENGINES = [
  "all",
  "multi",
  "google",
  "yandex",
  "bing",
  "duckduckgo",
  "wayback",
  "commoncrawl",
  "sitemap",
];

export function Explorer({
  scanId,
  category,
  onCategory,
  engine,
  onEngine,
  search,
  onSearch,
  maxWords,
  onMaxWords,
  sortBy,
  onSortBy,
  offset,
  limit,
  onOffset,
  pageData,
  selectedRow,
  onSelectRow,
  onExport,
  onCopyText,
  onReverify,
  onUpdateTarget,
}: {
  scanId: string;
  category: string;
  onCategory: (cat: string) => void;
  engine: string;
  onEngine: (eng: string) => void;
  search: string;
  onSearch: (q: string) => void;
  maxWords: number | null;
  onMaxWords: (val: number | null) => void;
  sortBy: string;
  onSortBy: (val: string) => void;
  offset: number;
  limit: number;
  onOffset: (next: number) => void;
  pageData: FinderTablePage;
  selectedRow: VerifiedUrl | null;
  onSelectRow: (row: VerifiedUrl | null) => void;
  onExport: () => void;
  onCopyText: (text: string) => void;
  onReverify: (url?: string) => void;
  onUpdateTarget: (url: string, target: string) => void;
}) {
  const { t, lang } = useI18n();
  const [targetDraft, setTargetDraft] = useState<string>("");
  const [candidates, setCandidates] = useState<RedirectCandidate[]>([]);

  useEffect(() => {
    if (!selectedRow || !scanId) {
      setTargetDraft("");
      setCandidates([]);
      return;
    }
    setTargetDraft(selectedRow.suggested_target);
    let cancelled = false;
    void api.redirectCandidates(scanId, selectedRow.url).then((res) => {
      if (!cancelled) setCandidates(res);
    });
    return () => {
      cancelled = true;
    };
  }, [scanId, selectedRow]);

  return (
    <div className="finder-page explorer-layout">
      <div className="explorer-main">
        <div className="oc-pagehead">
          <div className="oc-pagehead-text">
            <h1 className="oc-pagehead-title">{t("explorer")}</h1>
            <p className="oc-pagehead-lead">
              {t("urlCount", { count: number(pageData.total, lang) })}
            </p>
          </div>
          <div className="oc-pagehead-actions" style={{ flexWrap: "wrap" }}>
            <div className="oc-search">
              <Search size={15} />
              <input
                value={search}
                onChange={(e) => onSearch(e.target.value)}
                placeholder={t("searchPlaceholder")}
              />
            </div>
            <input
              type="number"
              min={0}
              className="oc-input"
              style={{ width: 120 }}
              value={maxWords ?? ""}
              onChange={(e) => {
                const raw = e.target.value.trim();
                onMaxWords(raw === "" ? null : Math.max(0, Number(raw)));
              }}
              placeholder={t("maxWordsFilter")}
            />
            <select
              className="oc-select"
              style={{ width: 220 }}
              value={sortBy}
              onChange={(e) => onSortBy(e.target.value)}
            >
              <option value="priority">{t("sortPriority")}</option>
              <option value="engines_desc">{t("sortEnginesDesc")}</option>
              <option value="words_asc">{t("sortWordsAsc")}</option>
              <option value="words_desc">{t("sortWordsDesc")}</option>
              <option value="time_desc">{t("sortTimeDesc")}</option>
              <option value="hops_desc">{t("sortHopsDesc")}</option>
              <option value="status_desc">{t("sortStatusDesc")}</option>
              <option value="url_asc">{t("sortUrlAsc")}</option>
            </select>
            <button
              className="oc-btn"
              onClick={() =>
                onCopyText(pageData.rows.map((r) => r.url).join("\n"))
              }
            >
              <Copy size={15} />
              {t("copyVisibleUrls")}
            </button>
            <button className="oc-btn" onClick={() => onReverify()}>
              <RefreshCw size={15} />
              {t("reverifyIssues")}
            </button>
            <button className="oc-btn" onClick={onExport}>
              <ArrowDownToLine size={15} />
              {t("exportReport")}
            </button>
          </div>
        </div>

        <div className="oc-tabs" style={{ marginBottom: 12 }}>
          {TABS.map((tab) => (
            <button
              key={tab.id}
              className={`oc-tab ${category === tab.id ? "active" : ""}`}
              onClick={() => onCategory(tab.id)}
            >
              {t(tab.label)}
            </button>
          ))}
        </div>

        <div className="oc-chips" style={{ marginBottom: 14 }}>
          {ENGINES.map((eng) => (
            <button
              key={eng}
              className={`oc-chip ${engine === eng || (!engine && eng === "all") ? "active-chip" : ""}`}
              onClick={() => onEngine(eng === "all" ? "" : eng)}
            >
              {eng === "all"
                ? t("allEngines")
                : eng === "multi"
                  ? t("multiEngineChip")
                  : eng.toUpperCase()}
            </button>
          ))}
        </div>

        <div className="oc-panel" style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
          <div className="oc-table-wrap" style={{ flex: 1, overflow: "auto" }}>
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t("colUrl")}</th>
                  <th>{t("colEngines")}</th>
                  <th className="num">{t("colStatus")}</th>
                  <th>{t("colCategory")}</th>
                  <th className="num">{t("colWords")}</th>
                  <th>{t("colTitle")}</th>
                  <th className="num">{t("colTime")}</th>
                </tr>
              </thead>
              <tbody>
                {pageData.rows.map((row) => (
                  <tr
                    key={row.url}
                    className={`clickable-row ${selectedRow?.url === row.url ? "selected" : ""}`}
                    onClick={() => onSelectRow(row)}
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
                    <td className="num mono">{row.response_time_ms} ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {pageData.total > limit && (
            <div
              className="row between"
              style={{
                padding: "10px 16px",
                borderTop: "1px solid var(--oc-line)",
              }}
            >
              <span className="muted mono" style={{ fontSize: 12 }}>
                {offset + 1}–{Math.min(offset + limit, pageData.total)} /{" "}
                {number(pageData.total, lang)}
              </span>
              <div className="row" style={{ gap: 8 }}>
                <button
                  className="oc-btn small"
                  disabled={offset === 0}
                  onClick={() => onOffset(Math.max(0, offset - limit))}
                >
                  {t("prevPage")}
                </button>
                <button
                  className="oc-btn small"
                  disabled={offset + limit >= pageData.total}
                  onClick={() => onOffset(offset + limit)}
                >
                  {t("nextPage")}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      {selectedRow && (
        <aside className="inspector-drawer oc-panel">
          <div className="row between" style={{ marginBottom: 14 }}>
            <h3 className="type-card-title">{t("inspectorTitle")}</h3>
            <button
              className="oc-btn ghost icon"
              onClick={() => onSelectRow(null)}
              aria-label={t("close")}
            >
              <X size={16} />
            </button>
          </div>

          <div className="inspector-url mono">
            <span>{selectedRow.url}</span>
            <a
              href={selectedRow.url}
              target="_blank"
              rel="noreferrer"
              title={t("openUrl")}
            >
              <ExternalLink size={14} />
            </a>
          </div>

          <div
            className="row between"
            style={{ marginTop: 12, gap: 8, flexWrap: "wrap" }}
          >
            <div>
              <span className={categoryBadgeClass(selectedRow.category)}>
                {t(`cat_${selectedRow.category}` as Key)}
              </span>
              <span className="muted" style={{ marginLeft: 8, fontSize: 12 }}>
                {t(`reason_${selectedRow.reason}` as Key) || selectedRow.reason}
              </span>
            </div>
            <button
              className="oc-btn small"
              onClick={() => onReverify(selectedRow.url)}
            >
              <RefreshCw size={13} />
              {t("reverifySingle")}
            </button>
          </div>

          <div
            className="row"
            style={{ marginTop: 10, gap: 6, flexWrap: "wrap" }}
          >
            <a
              className="oc-btn small ghost"
              href={`https://web.archive.org/web/*/${selectedRow.url}`}
              target="_blank"
              rel="noreferrer"
            >
              <ExternalLink size={12} />
              {t("waybackBtn")}
            </a>
            <a
              className="oc-btn small ghost"
              href={`https://www.google.com/search?q=${encodeURIComponent(`site:${selectedRow.url}`)}`}
              target="_blank"
              rel="noreferrer"
            >
              <ExternalLink size={12} />
              {t("googleSerpBtn")}
            </a>
            <a
              className="oc-btn small ghost"
              href={`https://yandex.com.tr/search/?text=${encodeURIComponent(`url:${selectedRow.url}`)}`}
              target="_blank"
              rel="noreferrer"
            >
              <ExternalLink size={12} />
              {t("yandexSerpBtn")}
            </a>
            <a
              className="oc-btn small ghost"
              href={`https://www.bing.com/search?q=${encodeURIComponent(`site:${selectedRow.url}`)}`}
              target="_blank"
              rel="noreferrer"
            >
              <ExternalLink size={12} />
              {t("bingSerpBtn")}
            </a>
          </div>

          <div className="inspector-section">
            <div className="type-label">{t("serpSnapshot")}</div>
            <div className="serp-preview-box">
              <div className="serp-preview-title">
                {selectedRow.serp_title || t("noSerpTitle")}
              </div>
              <div className="serp-preview-url mono">{selectedRow.url}</div>
              {selectedRow.serp_snippet && (
                <div className="serp-preview-snippet">
                  {selectedRow.serp_snippet}
                </div>
              )}
              <div className="serp-meta-line">
                <span>
                  <b>{t("colEngines")}:</b> {selectedRow.engines}
                </span>
                <span>
                  <b>{t("foundByQuery")}:</b>{" "}
                  <code>{selectedRow.found_by_query}</code>
                </span>
              </div>
            </div>
          </div>

          <div className="inspector-section">
            <div className="type-label">{t("liveCheck")}</div>
            <div className="kv-list">
              <div>
                <span>{t("colStatus")}</span>
                <b className="mono">
                  {selectedRow.initial_status} → {selectedRow.final_status} (
                  {selectedRow.response_time_ms} ms)
                </b>
              </div>
              <div>
                <span>{t("redirectHopsLabel")}</span>
                <b className="mono">
                  {selectedRow.redirect_hops}
                  {selectedRow.redirect_hops > 0
                    ? ` (${selectedRow.final_url})`
                    : ""}
                </b>
              </div>
              <div>
                <span>{t("wordCountLabel")}</span>
                <b className="mono">{selectedRow.word_count}</b>
              </div>
              <div>
                <span>{t("sitemapStatusLabel")}</span>
                <b>
                  {selectedRow.engines.includes("sitemap")
                    ? t("inSitemap")
                    : t("notInSitemap")}
                </b>
              </div>
              <div>
                <span>{t("canonicalLabel")}</span>
                <b className="mono">{selectedRow.canonical || "—"}</b>
              </div>
              <div>
                <span>{t("directivesLabel")}</span>
                <b>{selectedRow.noindex ? "noindex" : "index, follow"}</b>
              </div>
              <div>
                <span>{t("liveTitleLabel")}</span>
                <b>{selectedRow.live_title || "—"}</b>
              </div>
              <div>
                <span>{t("liveH1Label")}</span>
                <b>{selectedRow.h1 || "—"}</b>
              </div>
            </div>
          </div>

          {selectedRow.category !== "healthy_200" && (
            <div className="inspector-section">
              <div className="type-label">{t("suggestedRedirect")}</div>
              <div className="row" style={{ gap: 6, marginBottom: 8 }}>
                <input
                  className="oc-input mono"
                  style={{ flex: 1, fontSize: 12 }}
                  value={targetDraft}
                  onChange={(e) => setTargetDraft(e.target.value)}
                />
                <button
                  className="oc-btn small primary"
                  onClick={() => onUpdateTarget(selectedRow.url, targetDraft)}
                >
                  <Check size={13} />
                  {t("saveTarget")}
                </button>
              </div>
              <button
                type="button"
                className="oc-btn small ghost"
                style={{ width: "100%", marginBottom: 8 }}
                onClick={() => {
                  const bulkUrls = pageData.rows
                    .filter((r) => r.category !== "healthy_200")
                    .map((r) => r.url)
                    .join("\n");
                  if (bulkUrls) {
                    onUpdateTarget(bulkUrls, targetDraft);
                  }
                }}
              >
                <Sparkles size={13} />
                {t("applyToFilteredBtn")}
              </button>
              <div className="redirect-rule-box mono">
                <div>
                  Redirect 301 {new URL(selectedRow.url).pathname}{" "}
                  {targetDraft || selectedRow.suggested_target}
                </div>
                <button
                  className="oc-btn small"
                  onClick={() =>
                    onCopyText(
                      `Redirect 301 ${new URL(selectedRow.url).pathname} ${targetDraft || selectedRow.suggested_target}`,
                    )
                  }
                >
                  <Copy size={13} />
                  {t("copyRules")}
                </button>
              </div>

              {candidates.length > 0 && (
                <div style={{ marginTop: 12 }}>
                  <div
                    className="type-label"
                    style={{ display: "flex", alignItems: "center", gap: 6 }}
                  >
                    <Sparkles size={12} />
                    {t("smartCandidates")}
                  </div>
                  <div className="stack" style={{ gap: 6, marginTop: 6 }}>
                    {candidates.map((cand) => (
                      <button
                        key={cand.target_url}
                        type="button"
                        className="oc-btn ghost"
                        style={{
                          justifyContent: "space-between",
                          textAlign: "left",
                          padding: "7px 10px",
                          height: "auto",
                        }}
                        onClick={() => {
                          setTargetDraft(cand.target_url);
                          onUpdateTarget(selectedRow.url, cand.target_url);
                        }}
                      >
                        <div style={{ minWidth: 0, flex: 1 }}>
                          <div
                            className="mono truncate-cell"
                            style={{ fontSize: 11.5 }}
                          >
                            {cand.target_url}
                          </div>
                          <div className="muted" style={{ fontSize: 11 }}>
                            {cand.kind === "category_fallback"
                              ? t("parentDirectory")
                              : cand.title}
                            {cand.matched_tokens.length > 0 &&
                              ` · (${cand.matched_tokens.join(", ")})`}
                          </div>
                        </div>
                        <span
                          className={`oc-badge ${cand.similarity >= 60 ? "good" : "info"}`}
                        >
                          {percent(cand.similarity, lang)}
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {selectedRow.category === "healthy_200" &&
            selectedRow.redirect_hops >= 2 && (
              <div className="inspector-section">
                <div className="type-label">{t("suggestedRedirect")}</div>
                <div className="redirect-rule-box mono">
                  <div>
                    Redirect 301 {new URL(selectedRow.url).pathname}{" "}
                    {selectedRow.final_url}
                  </div>
                  <button
                    className="oc-btn small"
                    onClick={() =>
                      onCopyText(
                        `Redirect 301 ${new URL(selectedRow.url).pathname} ${selectedRow.final_url}`,
                      )
                    }
                  >
                    <Copy size={13} />
                    {t("copyRules")}
                  </button>
                </div>
              </div>
            )}
        </aside>
      )}
    </div>
  );
}
