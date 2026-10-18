import { useEffect, useState } from "react";
import {
  ArrowDownToLine,
  ArrowLeftRight,
  Copy,
  FolderOpen,
  Trash2,
} from "lucide-react";
import { api } from "../api/client";
import { downloadTextFile } from "../api/demo";
import type { DiffStatus, ScanComparison, ScanMeta } from "../api/types";
import { useI18n, number, percent, type Key } from "../i18n";

function diffBadge(diff: DiffStatus, t: (k: Key) => string) {
  switch (diff) {
    case "resolved":
      return <span className="oc-badge good">{t("diffResolved")}</span>;
    case "new_issue":
      return <span className="oc-badge critical">{t("diffNewIssue")}</span>;
    case "persistent_issue":
      return <span className="oc-badge warning">{t("diffPersistent")}</span>;
  }
}

export function Sessions({
  scans,
  selectedId,
  onSelect,
  onDelete,
  onCopyText,
}: {
  scans: ScanMeta[];
  selectedId: string;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onCopyText?: (text: string) => void;
}) {
  const { t, lang } = useI18n();
  const [baseId, setBaseId] = useState<string>(() => scans[1]?.id ?? scans[0]?.id ?? "");
  const [targetId, setTargetId] = useState<string>(() => scans[0]?.id ?? "");
  const [comparison, setComparison] = useState<ScanComparison | null>(null);
  const [diffFilter, setDiffFilter] = useState<string>("all");

  useEffect(() => {
    const first = scans[0];
    const second = scans[1];
    if (first && second && (!baseId || !targetId)) {
      setBaseId(second.id);
      setTargetId(first.id);
    }
  }, [scans, baseId, targetId]);

  useEffect(() => {
    if (!baseId || !targetId || baseId === targetId) {
      setComparison(null);
      return;
    }
    let cancelled = false;
    void api.compareScans(baseId, targetId).then((res) => {
      if (!cancelled) setComparison(res);
    });
    return () => {
      cancelled = true;
    };
  }, [baseId, targetId]);

  const filteredDiffRows =
    comparison?.rows.filter(
      (r) => diffFilter === "all" || r.diff === diffFilter,
    ) ?? [];

  const handleExportDiffCsv = () => {
    if (!comparison) return;
    const esc = (v: string | number) => `"${String(v).replace(/"/g, '""')}"`;
    const csv = [
      "url,engines,diff,base_status,target_status,base_category,target_category,suggested_target",
      ...filteredDiffRows.map((r) =>
        [
          esc(r.url),
          esc(r.engines),
          esc(r.diff),
          r.base_status,
          r.target_status,
          esc(r.base_category),
          esc(r.target_category),
          esc(r.suggested_target),
        ].join(","),
      ),
    ].join("\n");
    downloadTextFile(
      `${comparison.domain}-diff-${comparison.base_scan_id}-vs-${comparison.target_scan_id}.csv`,
      csv,
      "text/csv;charset=utf-8",
    );
  };

  return (
    <div className="finder-page">
      <div className="oc-pagehead">
        <div className="oc-pagehead-text">
          <h1 className="oc-pagehead-title">{t("sessions")}</h1>
          <p className="oc-pagehead-lead">{t("sessionsLead")}</p>
        </div>
      </div>

      <div className="oc-panel" style={{ marginBottom: 20 }}>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t("domainLabel")}</th>
                <th>{t("colDate")}</th>
                <th className="num">{t("statDiscovered")}</th>
                <th className="num">{t("stat404")}</th>
                <th className="num">{t("statThinSoft404")}</th>
                <th className="num">{t("statOther4xx5xx")}</th>
                <th className="num">{t("statHealthScore")}</th>
                <th className="num" />
              </tr>
            </thead>
            <tbody>
              {scans.map((s) => {
                const health = Math.round(
                  (s.count_healthy / Math.max(1, s.verified_urls)) * 100,
                );
                return (
                  <tr
                    key={s.id}
                    className={selectedId === s.id ? "selected" : ""}
                  >
                    <td className="mono">{s.domain}</td>
                    <td className="muted mono" style={{ fontSize: 12 }}>
                      {new Date(s.created_at_ms).toLocaleDateString(
                        lang === "tr" ? "tr-TR" : "en-US",
                      )}
                    </td>
                    <td className="num">{number(s.verified_urls, lang)}</td>
                    <td className="num">
                      <span className="oc-badge critical">{s.count_404}</span>
                    </td>
                    <td className="num">
                      <span className="oc-badge warning">
                        {s.count_soft_404 + s.count_thin_200}
                      </span>
                    </td>
                    <td className="num">{s.count_4xx + s.count_5xx}</td>
                    <td className="num">
                      <span
                        className={`oc-badge ${health >= 80 ? "good" : health >= 50 ? "warning" : "critical"}`}
                      >
                        {percent(health, lang)}
                      </span>
                    </td>
                    <td className="num">
                      <div className="row end" style={{ gap: 6 }}>
                        <button
                          className="oc-btn small"
                          onClick={() => onSelect(s.id)}
                        >
                          <FolderOpen size={13} />
                          {t("openScan")}
                        </button>
                        <button
                          className="oc-btn danger small"
                          onClick={() => onDelete(s.id)}
                        >
                          <Trash2 size={13} />
                          {t("deleteScan")}
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      {scans.length >= 2 && (
        <div className="oc-panel" style={{ padding: 20 }}>
          <div className="row between" style={{ marginBottom: 14 }}>
            <h2 className="type-card-title">
              <ArrowLeftRight
                size={17}
                style={{ verticalAlign: "middle", marginRight: 8 }}
              />
              {t("compareTitle")}
            </h2>
            {comparison && (
              <div className="row" style={{ gap: 8 }}>
                {onCopyText && (
                  <>
                    <button
                      className="oc-btn small"
                      onClick={() =>
                        onCopyText(
                          filteredDiffRows
                            .filter((r) => r.diff !== "resolved")
                            .map(
                              (r) =>
                                `Redirect 301 ${new URL(r.url).pathname} ${r.suggested_target}`,
                            )
                            .join("\n"),
                        )
                      }
                    >
                      <Copy size={14} />
                      {t("copyRules")}
                    </button>
                    <button
                      className="oc-btn small"
                      onClick={() =>
                        onCopyText(filteredDiffRows.map((r) => r.url).join("\n"))
                      }
                    >
                      <Copy size={14} />
                      {t("copyVisibleUrls")}
                    </button>
                  </>
                )}
                <button className="oc-btn small" onClick={handleExportDiffCsv}>
                  <ArrowDownToLine size={14} />
                  {t("exportDiffCsv")}
                </button>
              </div>
            )}
          </div>

          <div className="form-two-col" style={{ marginBottom: 16 }}>
            <label className="oc-field">
              <span>{t("compareBase")}</span>
              <select
                className="oc-select mono"
                value={baseId}
                onChange={(e) => setBaseId(e.target.value)}
              >
                {scans.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.domain} ({s.id})
                  </option>
                ))}
              </select>
            </label>
            <label className="oc-field">
              <span>{t("compareTarget")}</span>
              <select
                className="oc-select mono"
                value={targetId}
                onChange={(e) => setTargetId(e.target.value)}
              >
                {scans.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.domain} ({s.id})
                  </option>
                ))}
              </select>
            </label>
          </div>

          {comparison && (
            <>
              <div className="oc-stats" style={{ marginBottom: 16 }}>
                <div className="oc-stat good">
                  <div className="oc-stat-label">{t("diffResolved")}</div>
                  <div className="oc-stat-value oc-num">
                    {number(comparison.resolved_count, lang)}
                  </div>
                </div>
                <div className="oc-stat critical">
                  <div className="oc-stat-label">{t("diffNewIssue")}</div>
                  <div className="oc-stat-value oc-num">
                    {number(comparison.new_issue_count, lang)}
                  </div>
                </div>
                <div className="oc-stat warning">
                  <div className="oc-stat-label">{t("diffPersistent")}</div>
                  <div className="oc-stat-value oc-num">
                    {number(comparison.persistent_issue_count, lang)}
                  </div>
                </div>
              </div>

              <div className="oc-chips" style={{ marginBottom: 12 }}>
                {[
                  { id: "all", label: t("diffAll") },
                  { id: "resolved", label: t("diffResolved") },
                  { id: "new_issue", label: t("diffNewIssue") },
                  { id: "persistent_issue", label: t("diffPersistent") },
                ].map((chip) => (
                  <button
                    key={chip.id}
                    className={`oc-chip ${diffFilter === chip.id ? "active-chip" : ""}`}
                    onClick={() => setDiffFilter(chip.id)}
                  >
                    {chip.label}
                  </button>
                ))}
              </div>

              <div className="oc-table-wrap">
                <table className="oc-table">
                  <thead>
                    <tr>
                      <th>{t("colUrl")}</th>
                      <th>{t("colEngines")}</th>
                      <th>{t("colChange")}</th>
                      <th className="num">{t("colStatusChange")}</th>
                      <th>{t("suggestedRedirect")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredDiffRows.map((row) => (
                      <tr key={row.url}>
                        <td className="mono url-cell">{row.url}</td>
                        <td>
                          <span className="engine-pill small">{row.engines}</span>
                        </td>
                        <td>{diffBadge(row.diff, t)}</td>
                        <td className="num mono">
                          {row.base_status || "—"} →{" "}
                          {row.target_status || t("droppedFromIndex")}
                        </td>
                        <td className="mono">{row.suggested_target}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
