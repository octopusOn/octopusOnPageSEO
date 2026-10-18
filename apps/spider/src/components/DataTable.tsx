import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-react";
import { type Key, useI18n, number } from "../i18n";
import type { Cell, TablePage } from "../api/types";
import { Empty } from "./ui";
import { issueById } from "../catalog";
import { findingChangeKeys, findingReason } from "../lib/findingComparison";
import type { FindingChange } from "../api/types";

const labels: Record<string, Key> = {
  url: "address",
  crawl_url: "crawl_url",
  sitemap: "sitemapSource",
  sitemaps: "sitemaps",
  status: "status",
  lastmod: "sitemapLastmod",
  redirect_outcome: "sitemapRedirectOutcome",
  redirect_location: "sitemapRedirectLocation",
  final_url: "sitemapFinalUrl",
  final_status: "sitemapFinalStatus",
  redirect_hops: "sitemapRedirectHops",
  match_address: "match_address",
  match_title: "match_title",
  match_word_count: "match_word_count",
  hamming_distance: "hamming_distance",
  ...Object.fromEntries(
    [
      "rendered",
      "rendered_url",
      "render_error",
      "render_time_ms",
      "render_truncated",
      "raw_title",
      "rendered_title",
      "raw_meta_description",
      "rendered_meta_description",
      "raw_canonical",
      "rendered_canonical",
      "raw_noindex",
      "rendered_noindex",
      "raw_nofollow",
      "rendered_nofollow",
      "raw_word_count",
      "rendered_word_count",
      "raw_link_count",
      "rendered_link_count",
    ].map((v) => [v, v as Key]),
  ),
  status_code: "status",
  error: "error",
  destination_error: "linkDestinationError",
  source_internal: "linkSourceClass",
  destination_internal: "linkDestinationClass",
  source_status: "linkSourceStatus",
  internal_inlinks: "internalInlinks",
  external_inlinks: "externalInlinks",
  internal_outlinks: "internalOutlinks",
  external_outlinks: "externalOutlinks",
  blocked_by_robots: "blocked_by_robots",
  redirect_url: "redirect_url",
  ...Object.fromEntries([
    "h1_length", "h2", "h2_count", "header_canonical", "meta_robots", "x_robots_tag",
    "unique_inlinks", "unique_outlinks", "rel_next", "rel_prev",
    "last_modified", "robots_rule", "crawled_at",
  ].map((name) => [name, name as Key])),
  indexability: "indexability",
  canonical: "canonical",
  title: "title",
  ...Object.fromEntries(
    [
      "address",
      "content",
      "crawl_depth",
      "inlinks",
      "outlinks",
      "word_count",
      "size_bytes",
      "response_time_ms",
      "title_length",
      "title_count",
      "meta_description",
      "meta_description_length",
      "meta_description_count",
      "h1",
      "h1_count",
      "page",
      "image",
      "alt",
      "missing_alt",
      "has_dimensions",
      "lazy",
      "source",
      "destination",
      "text",
      "kind",
      "destination_status",
      "lang",
      "target",
      "target_status",
      "format",
      "types",
      "valid",
      "entries",
      "children",
      "http_status",
    ].map((v) => [v, v as Key]),
  ),
};
export function columnLabel(name: string, t: (key: Key) => string): string {
  const hreflangLabels:Record<string,Key>={source_canonical:"sourceCanonical",return_link:"seoEvidenceReturn",canonical_context:"hreflangCanonicalContext",pagination_candidate:"paginationCandidate",canonical_content_equal:"canonicalContentEqual"};
  if(hreflangLabels[name])return t(hreflangLabels[name]);
  const nameLabels:Record<string,Key>={name_source:"linkNameSource",visible_text:"linkVisibleText",recorded_text:"linkRecordedText",has_svg:"linkHasSvg",has_image:"linkHasImage",name_observation:"linkNameObservation"};
  if(nameLabels[name])return t(nameLabels[name]);
  const findingLabels:Record<string,Key>={issue_id:"findingCheck",finding_change:"findingStateFilter",comparison_reason:"findingReason",priority:"findingPriority"};
  if(findingLabels[name])return t(findingLabels[name]);
  if(name.startsWith("before_")) return `${t("compareBeforeShort")} · ${columnLabel(name.slice(7),t)}`;
  if(name.startsWith("after_")) return `${t("compareAfterShort")} · ${columnLabel(name.slice(6),t)}`;
  const comparison:Record<string,Key>={change:"compareResultFilter",changed_fields:"compareChangedFields",text_hash:"compareTextHash",noindex:"compareNoindex",nofollow:"compareNofollow",truncated:"compareTruncated"};
  if(comparison[name]) return t(comparison[name]);
  return labels[name] ? t(labels[name]) : name.replaceAll("_", " ");
}
export function DataTable({
  data,
  visible,
  sort,
  descending,
  onSort,
  onSelect,
  selected,
  selectColumn,
}: {
  data: TablePage;
  visible?: string[];
  sort?: string;
  descending?: boolean;
  onSort?: (column: string) => void;
  onSelect?: (url: string) => void;
  selected?: string;
  selectColumn?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const { t, lang } = useI18n();
  const columns = visible
    ? visible.flatMap((name) => {
        const index = data.columns.findIndex((c) => c.name === name);
        return index < 0 ? [] : [{ name, index }];
      })
    : data.columns.map((c, index) => ({ name: c.name, index }));
  const virtual = useVirtualizer({
    count: data.rows.length,
    getScrollElement: () => ref.current,
    estimateSize: () => 43,
    overscan: 8,
  });
  const template = columns
    .map(({ name }) =>
      ["address", "url", "crawl_url", "sitemap", "final_url", "redirect_url", "match_address", "page", "image", "source", "destination", "target"].includes(
        name,
      )
        ? "minmax(310px, 2.4fr)"
        : ["title", "match_title", "meta_description", "canonical", "text", "value", "issue_id", "comparison_reason"].includes(
              name,
            )
          ? "minmax(230px, 1.5fr)"
          : "minmax(112px, 1fr)",
    )
    .join(" ");
  const minWidth = columns.reduce(
    (sum, { name }) =>
      sum +
      (["address", "url", "crawl_url", "sitemap", "final_url", "redirect_url", "match_address", "page", "image", "source", "destination", "target"].includes(
        name,
      )
        ? 310
        : ["title", "match_title", "meta_description", "canonical", "text", "value", "issue_id", "comparison_reason"].includes(
              name,
            )
          ? 230
          : 112),
    0,
  );
  function display(name: string, value: Cell | undefined) {
    if (value === null || value === undefined || value === "")
      return <span className="faint">—</span>;
    if(name==="redirect_outcome") {
      const states:Record<string,Key>={complete:"sitemapRedirectComplete",error:"sitemapRedirectError",loop:"sitemapRedirectLoop",unchecked:"sitemapRedirectUnchecked"};
      return <span className={value==="complete"?"good":"muted"}>{states[String(value)]?t(states[String(value)]!):String(value)}</span>;
    }
    if(name==="status") {
      const states:Record<string,Key>={found:"robotsFound",missing:"robotsMissing",unreachable:"robotsUnreachable"};
      if(states[String(value)]) return <span className={value==="found"?"good":"muted"} title={String(value)}>{t(states[String(value)]!)}</span>;
    }
    if(name==="name_observation")return t(value==="saved-html"?"linkNameSavedHtml":"linkNameRecorded");
    if(name==="name_source"&&value==="missing")return t("linkNameMissing");
    if(name==="issue_id")return issueById.get(String(value))?.[lang].title??String(value);
    if(name==="finding_change")return <span className={`finding-${String(value)}`}>{findingChangeKeys[value as FindingChange]?t(findingChangeKeys[value as FindingChange]):String(value)}</span>;
    if(name==="comparison_reason")return findingReason(String(value),t);
    if(name==="priority")return ["high","medium","low"].includes(String(value))?t(value as "high"|"medium"|"low"):t("findingUnknownPriority");
    if(name==="change") {
      const keys:Record<string,Key>={changed:"compareChanged",new:"compareNew",missing:"compareMissing",unchanged:"compareUnchanged"};
      return <span className={`compare-kind compare-${String(value)}`}>{keys[String(value)]?t(keys[String(value)]!):String(value)}</span>;
    }
    if(name==="changed_fields") return String(value).split(", ").map(field=>columnLabel(field,t)).join(", ");
    const baseName = name.replace(/^(before|after)_/, "");
    if (
      baseName.endsWith("status") ||
      baseName === "status_code" ||
      baseName === "http_status"
    ) {
      const status = Number(value);
      if (status >= 100)
        return (
          <span className={`status-code s${Math.floor(status / 100)}`}>
            {status}
          </span>
        );
    }
    if (baseName === "indexability")
      return (
        <span
          className={`index-state ${value === "indexable" ? "good" : "muted"}`}
        >
          <i />
          {value === "indexable" ? t("indexableLabel") : value === "other-http" ? t("responseOtherHttp") : String(value)}
        </span>
      );
    if (typeof value === "boolean")
      return (
        <span className={value ? "good" : "muted"}>
          {t(["source_internal", "destination_internal"].includes(baseName) ? value ? "linkInternal" : "linkExternal" : value ? "yes" : "no")}
        </span>
      );
    if (name === "size_bytes" && typeof value === "number")
      return `${number(value / 1024, lang)} KB`;
    if (name === "response_time_ms" || name === "render_time_ms")
      return `${number(Number(value), lang)} ms`;
    if (typeof value === "number") return number(value, lang);
    return String(value);
  }
  if (!data.rows.length) return <Empty>{t("noResults")}</Empty>;
  return (
    <div
      ref={ref}
      className="data-scroll"
      role="table"
      aria-rowcount={data.total + 1}
      aria-colcount={columns.length}
    >
      <div
        className="data-head"
        role="row"
        style={{ gridTemplateColumns: template, minWidth }}
      >
        {columns.map(({ name }) => (
          <div
            key={name}
            role="columnheader"
            aria-sort={
              sort === name ? (descending ? "descending" : "ascending") : "none"
            }
          >
            <button disabled={!onSort} onClick={() => onSort?.(name)}>
              {columnLabel(name, t)}
              {onSort &&
                (sort === name ? (
                  descending ? (
                    <ArrowDown size={12} />
                  ) : (
                    <ArrowUp size={12} />
                  )
                ) : (
                  <ArrowUpDown size={11} className="sort-hint" />
                ))}
            </button>
          </div>
        ))}
      </div>
      <div
        style={{
          height: virtual.getTotalSize(),
          minWidth,
          position: "relative",
        }}
      >
        {virtual.getVirtualItems().map((item) => {
          const row = data.rows[item.index];
          if (!row) return null;
          const urlIndex = data.columns.findIndex((c) => selectColumn ? c.name === selectColumn :
            ["address", "page", "source"].includes(c.name));
          const url = String(row[urlIndex] ?? "");
          const selectable = Boolean(onSelect && url);
          const isSelected = Boolean(url && selected === url);
          return (
            <div
              key={item.key}
              role="row"
              aria-rowindex={item.index + 2}
              aria-selected={isSelected}
              tabIndex={selectable ? 0 : undefined}
              className={`data-row ${selectable ? "selectable" : ""} ${isSelected ? "selected" : ""}`}
              style={{
                gridTemplateColumns: template,
                height: item.size,
                transform: `translateY(${item.start}px)`,
              }}
              onClick={() => { if(selectable) onSelect?.(url); }}
              onKeyDown={(e) => {
                if (selectable && e.key === "Enter") onSelect?.(url);
              }}
            >
              {columns.map(({ name, index }) => (
                <div
                  role="cell"
                  key={name}
                  className={
                    [
                      "address",
                      "page",
                      "image",
                      "source",
                      "destination",
                      "target",
                      "canonical",
                    ].includes(name)
                      ? "url-cell"
                      : ""
                  }
                  title={name==="comparison_reason"?(row[data.columns.findIndex(column=>column.name==="finding_change")]==="persisting"?t("findingRecordedNote"):findingReason(String(row[index]??""),t)):name==="issue_id"?issueById.get(String(row[index]))?.[lang].title??String(row[index]??""):String(row[index]??"")}
                >
                  {display(name, row[index])}
                </div>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
