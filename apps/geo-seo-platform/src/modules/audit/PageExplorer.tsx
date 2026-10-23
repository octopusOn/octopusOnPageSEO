// Every page an audit crawled: searched, filtered by status and indexability, sorted on the
// crawl itself, a hundred at a time; a page opens with what was read from it, its findings and
// the pages linking to it.

import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronLeft, ChevronRight, Download, Search } from "lucide-react";
import { Button, Loading, Modal, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { Entity } from "../../lib/Entity";
import { explain } from "../../lib/errors";
import { formatNumber } from "../../lib/format";
import { download, toCsv } from "../../lib/report";
import { pick } from "./measures";

/** Columns shown, in order. */
const SHOWN = ["address", "status_code", "indexability", "title", "crawl_depth", "unique_inlinks", "word_count", "response_time_ms"] as const;

/** Columns the crawl can sort by. */
const SORTS = ["address", "crawl_depth", "unique_inlinks", "word_count", "response_time_ms", "status_code"] as const;

const INDEXABILITY = ["", "indexable", "noindex", "canonicalised", "redirected", "client-error", "server-error", "blocked-by-robots"] as const;

/** Facts shown in a page's window, in order. */
const FACTS = ["status_code", "indexability", "title", "title_length", "meta_description", "meta_description_length", "h1", "h1_count", "h2_count", "canonical", "meta_robots", "redirect_url", "crawl_depth", "inlinks", "unique_inlinks", "outlinks", "external_outlinks", "word_count", "size_bytes", "response_time_ms", "lang", "last_modified"];

const ROWS = 100;

function PageWindow({ project, auditId, url, onClose }: { project: ProjectRow; auditId: number; url: string; onClose: () => void }) {
  const { t, lang, locale } = useI18n();
  const page = useQuery({ queryKey: ["audit", "page", project.id, auditId, url, lang], queryFn: () => api.auditPage(project.id, auditId, url, lang) });
  const facts = new Map(page.data?.facts ?? []);
  return (
    <Modal title={url.replace(/^https?:\/\/(www\.)?/, "")} closeLabel={t("common.close")} onClose={onClose}>
      {page.isLoading && <Loading label={t("common.loading")} />}
      {page.error && <p className="warn">{explain(page.error, t).message}</p>}
      {page.data && (
        <div className="page-stack page-window">
          <Entity kind="url" value={url}>
            <span className="cell-url">{url}</span>
          </Entity>
          <dl className="page-facts">
            {FACTS.filter((name) => (facts.get(name) ?? "") !== "").map((name) => (
              <div key={name}>
                <dt>{t(`audit.fact_${name}`)}</dt>
                <dd>{facts.get(name)}</dd>
              </div>
            ))}
          </dl>
          <section>
            <h4>{t("audit.pageFindings", { n: page.data.findings.length })}</h4>
            {page.data.findings.length ? (
              <ul className="check-list">
                {page.data.findings.map((finding) => (
                  <li key={finding.id} className={finding.priority === "high" ? "bad" : undefined}>
                    <span className={`oc-badge ${finding.priority === "high" ? "critical" : finding.priority === "medium" ? "warning" : "info"}`}>{t(`audit.priority_${finding.priority}`)}</span>
                    <span>{finding.title}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="faint small">{t("audit.pageClean")}</p>
            )}
          </section>
          <section>
            <h4>{t("audit.pageInlinks", { n: formatNumber(page.data.inlinks_total, locale) })}</h4>
            <ul className="plain-list small">
              {page.data.inlinks.map((link) => (
                <li key={link.url}>
                  <span className="cell-url">{link.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
                  {link.anchor && <span className="faint"> · “{link.anchor}”</span>}
                </li>
              ))}
            </ul>
          </section>
        </div>
      )}
    </Modal>
  );
}

export function PageExplorer({ project, auditId }: { project: ProjectRow; auditId: number | null }) {
  const { t, locale } = useI18n();
  const [text, setText] = useState("");
  const [search, setSearch] = useState("");
  const [statusClass, setStatusClass] = useState("0");
  const [indexability, setIndexability] = useState<string>("");
  const [sort, setSort] = useState<string>("address");
  const [descending, setDescending] = useState(false);
  const [offset, setOffset] = useState(0);
  const [open, setOpen] = useState<string | null>(null);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSearch(text.trim());
      setOffset(0);
    }, 400);
    return () => window.clearTimeout(timer);
  }, [text]);
  const ask = { offset, limit: ROWS, sort, descending, search, status_class: Number(statusClass), indexability };
  const pages = useQuery({ queryKey: ["audit", "pages", project.id, auditId, ask], queryFn: () => api.auditPages(project.id, auditId, ask), placeholderData: (previous) => previous });
  const data = pages.data;
  const rows = data ? pick(data.columns, data.rows, SHOWN) : [];
  const shownAudit = data?.audit_id ?? auditId;
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("audit.pagesTitle")}</h3>
          <span className="faint">{t("audit.pagesExplorerHint")}</span>
        </div>
        <div className="report-actions">
          <Button small disabled={rows.length === 0} onClick={() => download(`audit-pages-${project.domain}.csv`, toCsv(SHOWN.map((name) => t(`audit.fact_${name}`)), rows))}>
            <Download size={14} />
            {t("overview.export")}
          </Button>
        </div>
      </header>
      <div className="filters">
        <label className="filter-search">
          <Search size={15} aria-hidden="true" />
          <span className="sr-only">{t("audit.pagesSearch")}</span>
          <input className="oc-input" value={text} placeholder={t("audit.pagesSearch")} onChange={(event) => setText(event.target.value)} />
        </label>
        <Segmented<string>
          label={t("audit.fact_status_code")}
          value={statusClass}
          onChange={(value) => {
            setStatusClass(value);
            setOffset(0);
          }}
          options={[
            { value: "0", label: t("audit.statusAll") },
            { value: "2", label: "2xx" },
            { value: "3", label: "3xx" },
            { value: "4", label: "4xx" },
            { value: "5", label: "5xx" },
          ]}
        />
        <select
          className="oc-select"
          aria-label={t("audit.fact_indexability")}
          value={indexability}
          onChange={(event) => {
            setIndexability(event.target.value);
            setOffset(0);
          }}
        >
          {INDEXABILITY.map((value) => (
            <option key={value} value={value}>
              {value ? t(`audit.indexability_${value.replace(/-/g, "_")}`) : t("audit.indexabilityAll")}
            </option>
          ))}
        </select>
        <select className="oc-select" aria-label={t("audit.sortBy")} value={sort} onChange={(event) => setSort(event.target.value)}>
          {SORTS.map((name) => (
            <option key={name} value={name}>
              {t("audit.sortByColumn", { column: t(`audit.fact_${name}`) })}
            </option>
          ))}
        </select>
        <Button small className="ghost" onClick={() => setDescending(!descending)} aria-label={descending ? t("audit.sortDescending") : t("audit.sortAscending")} title={descending ? t("audit.sortDescending") : t("audit.sortAscending")}>
          {descending ? <ArrowDownWideNarrow size={14} /> : <ArrowUpNarrowWide size={14} />}
        </Button>
      </div>
      {pages.error && <p className="warn">{explain(pages.error, t).message}</p>}
      {!data && pages.isLoading && <Loading label={t("common.loading")} />}
      {data === null && <p className="faint">{t("audit.noCrawl")}</p>}
      {data && (
        <>
          <div className="table-scroll">
            <table className="oc-table compact explorer-table">
              <thead>
                <tr>
                  {SHOWN.map((name) => (
                    <th key={name} className={["status_code", "crawl_depth", "unique_inlinks", "word_count", "response_time_ms"].includes(name) ? "numeric" : undefined}>
                      {t(`audit.fact_${name}`)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr key={row[0]}>
                    {row.map((cell, index) =>
                      index === 0 ? (
                        <td key={index} className="cell-url">
                          <button type="button" className="link-button" onClick={() => setOpen(cell)} title={t("audit.openPage")}>
                            {cell.replace(/^https?:\/\/(www\.)?/, "")}
                          </button>
                        </td>
                      ) : (
                        <td key={index} className={index === 3 ? "cell-title" : index === 1 || index >= 4 ? "numeric" : undefined}>
                          {index >= 5 && cell ? formatNumber(Number(cell), locale) : cell}
                        </td>
                      ),
                    )}
                  </tr>
                ))}
                {rows.length === 0 && (
                  <tr>
                    <td colSpan={SHOWN.length} className="faint">
                      {t("overview.emptyRows")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          <div className="pager">
            <Button small disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - ROWS))} aria-label={t("audit.previousPage")}>
              <ChevronLeft size={14} />
            </Button>
            <span className="faint small">
              {data.total === 0 ? "0" : `${formatNumber(offset + 1, locale)}–${formatNumber(offset + rows.length, locale)}`} / {formatNumber(data.total, locale)}
            </span>
            <Button small disabled={offset + rows.length >= data.total} onClick={() => setOffset(offset + ROWS)} aria-label={t("audit.nextPage")}>
              <ChevronRight size={14} />
            </Button>
          </div>
        </>
      )}
      {open && shownAudit !== null && <PageWindow project={project} auditId={shownAudit} url={open} onClose={() => setOpen(null)} />}
    </section>
  );
}
