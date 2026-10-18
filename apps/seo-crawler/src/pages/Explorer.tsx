import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { Search, ChevronLeft, ChevronRight, X, Filter, Columns3 } from "lucide-react";
import { api, errorText } from "../api/client";
import { issueById } from "../catalog";
import { useI18n, number, persist, stored, type Key } from "../i18n";
import { DataTable } from "../components/DataTable";
import { Inspector } from "../components/Inspector";
import { Button, Loading, ErrorState } from "../components/ui";
import type { ExplorerExportView } from "../lib/exports";
import type { LinkClass, LinkScope, DuplicateScope, ResponseGroup } from "../api/types";
import { isLinkEvidenceIssue } from "../lib/links";
import { duplicateKindForIssue, duplicateLabels } from "../lib/duplicates";
import { responseGroups } from "../lib/responses";
import { ColumnSettings } from "../components/ColumnSettings";
import { columnPreferencesKey, parseColumnPreferences, visibleColumns as resolveColumns } from "../lib/columns";

export const tabs: {
  id: string;
  label: Key;
  backend: string;
  columns: string[];
}[] = [
  {
    id: "internal",
    label: "internal",
    backend: "internal",
    columns: [
      "address",
      "status_code",
      "indexability",
      "title",
      "crawl_depth",
      "internal_inlinks",
      "external_inlinks",
      "internal_outlinks",
      "external_outlinks",
      "response_time_ms",
    ],
  },
  {
    id: "external",
    label: "external",
    backend: "external",
    columns: [
      "address",
      "status_code",
      "content",
      "internal_inlinks",
      "external_inlinks",
      "internal_outlinks",
      "external_outlinks",
      "response_time_ms",
    ],
  },
  {
    id: "response-codes",
    label: "responseCodes",
    backend: "response-codes",
    columns: [
      "address",
      "status_code",
      "content",
      "indexability",
      "error",
      "blocked_by_robots",
      "redirect_url",
      "response_time_ms",
    ],
  },
  {
    id: "titles",
    label: "title",
    backend: "internal",
    columns: [
      "address",
      "status_code",
      "title",
      "title_length",
      "title_count",
      "indexability",
    ],
  },
  {
    id: "descriptions",
    label: "description",
    backend: "internal",
    columns: [
      "address",
      "status_code",
      "meta_description",
      "meta_description_length",
      "meta_description_count",
    ],
  },
  {
    id: "headings",
    label: "headings",
    backend: "headings",
    columns: ["page", "level", "text", "length"],
  },
  {
    id: "canonicals",
    label: "canonical",
    backend: "internal",
    columns: [
      "address",
      "status_code",
      "canonical",
      "header_canonical",
      "indexability",
    ],
  },
  {
    id: "images",
    label: "images",
    backend: "images",
    columns: [
      "image",
      "page",
      "alt",
      "missing_alt",
      "has_dimensions",
      "status_code",
      "size_bytes",
    ],
  },
  {
    id: "links",
    label: "links",
    backend: "links",
    columns: [
      "source",
      "destination",
      "text",
      "kind",
      "nofollow",
      "destination_status",
      "destination_error",
      "source_internal",
      "destination_internal",
      "name_source",
      "has_svg",
    ],
  },
  {
    id: "hreflang",
    label: "hreflang",
    backend: "hreflang",
    columns: ["page", "lang", "target", "target_status", "source_canonical", "return_link", "canonical_context", "pagination_candidate", "canonical_content_equal"],
  },
  {
    id: "structured-data",
    label: "structuredData",
    backend: "structured-data",
    columns: ["page", "format", "types", "valid"],
  },
  {
    id: "sitemaps",
    label: "sitemaps",
    backend: "sitemaps",
    columns: ["url", "status_code", "kind", "redirect_outcome", "final_url", "final_status", "entries", "children", "error"],
  },
  {
    id: "sitemap-urls",
    label: "sitemapUrls",
    backend: "sitemap-urls",
    columns: ["url", "crawl_url", "status_code", "indexability", "sitemap", "lastmod", "crawl_depth"],
  },
  {
    id: "robots",
    label: "robots",
    backend: "robots",
    columns: ["url", "status", "http_status", "sitemaps"],
  },
  {
    id: "rendering",
    label: "renderingTab",
    backend: "rendering",
    columns: [
      "address",
      "rendered",
      "render_time_ms",
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
      "rendered_url",
      "render_error",
      "render_truncated",
    ],
  },
  {
    id: "content-matches",
    label: "contentMatch",
    backend: "content-matches",
    columns: ["address", "match_address", "hamming_distance", "word_count", "match_word_count", "title", "match_title"],
  },
];
export function Explorer({
  id,
  tabId,
  onTab,
  issueId,
  onClearIssue,
  onExportView,
  responseGroup,
  onResponseGroup,
  excluded,
  onIssue,
  onSchema,
}: {
  id: string;
  tabId: string;
  onTab: (tab: string) => void;
  issueId: string;
  onClearIssue: () => void;
  onExportView: (view: ExplorerExportView | null) => void;
  responseGroup: ResponseGroup | "";
  onResponseGroup: (group: ResponseGroup | "") => void;
  excluded: string[];
  onIssue: (id: string) => void;
  onSchema?: (url:string)=>void;
}) {
  const { t, lang } = useI18n();
  const activeTab = useRef<HTMLButtonElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  useEffect(() => { activeTab.current?.scrollIntoView({ block: "nearest", inline: "nearest" }); }, [tabId]);
  const [search, setSearch] = useState("");
  const [debounced, setDebounced] = useState("");
  const [status, setStatus] = useState(0);
  const [indexability, setIndexability] = useState("");
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState("");
  const [descending, setDescending] = useState(false);
  const [selected, setSelected] = useState("");
  const [linkScope, setLinkScope] = useState<LinkScope | null>(null);
  const [linkClass, setLinkClass] = useState<LinkClass | null>(null);
  const [duplicateScope, setDuplicateScope] = useState<DuplicateScope | null>(null);
  const [columnPreferences, setColumnPreferences] = useState(() => parseColumnPreferences(stored<unknown>(columnPreferencesKey, {}), tabs.map((tab) => tab.id)));
  const [columnsOpen, setColumnsOpen] = useState(false);
  useEffect(() => setColumnsOpen(false), [id, tabId]);
  const tab = tabs.find((t) => t.id === tabId) ?? tabs[0]!;
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(search), 220);
    return () => clearTimeout(timer);
  }, [search]);
  useEffect(() => {
    setOffset(0);
    setSelected("");
  }, [id, tabId, debounced, status, indexability, issueId, linkScope, linkClass, duplicateScope, responseGroup]);
  useEffect(() => {
    setStatus(0);
    setIndexability("");
    setSort("");
    if (tabId !== "links") { setLinkScope(null); setLinkClass(null); }
    if (tabId !== "internal") setDuplicateScope(null);
  }, [tabId]);
  const request = useMemo(() => ({
    tab: tab.backend,
    offset,
    limit: 250,
    search: debounced,
    status_class: tab.id === "response-codes" ? 0 : status,
    indexability,
    issue_id: issueId,
    sort,
    descending,
    link_scope: tab.backend === "links" ? linkScope : null,
    link_class: tab.backend === "links" ? linkClass : null,
    duplicate_scope: tab.id === "internal" ? duplicateScope : null,
    response_group: tab.id === "response-codes" ? responseGroup || null : null,
  }), [tab.backend, tab.id, offset, debounced, status, indexability, issueId, sort, descending, linkScope, linkClass, duplicateScope, responseGroup]);
  const query = useQuery({
    queryKey: ["urls", id, request],
    queryFn: () => api.page(id, request),
    placeholderData: keepPreviousData,
  });
  const schemaColumns = useMemo(() => query.data?.columns.map((column) => column.name) ?? [], [query.data?.columns]);
  const requiredColumns = useMemo(() => {
    const fields = [tab.columns[0]!];
    if (tab.backend === "links") fields.push("destination");
    if (tab.backend === "links" && isLinkEvidenceIssue(issueId)) fields.push("destination_status", "destination_error", "text");
    if (tab.backend === "images") fields.push("page");
    if (tab.backend === "content-matches") fields.push("match_address");
    if (tab.id === "internal" && duplicateScope) {
      const field = {title:"title",description:"meta_description",h1:"h1",h2:"h2",content:"word_count"}[duplicateScope.kind];
      fields.splice(1,0,field);
    }
    return [...new Set(fields)];
  }, [tab, duplicateScope, issueId]);
  const visibleColumns = useMemo(() => query.data ? resolveColumns(schemaColumns, tab.columns, columnPreferences[tab.id], requiredColumns) : tab.columns,
    [query.data, schemaColumns, tab, columnPreferences, requiredColumns]);
  useEffect(() => {
    if (query.data && !query.isPlaceholderData && sort && !visibleColumns.includes(sort)) {
      setSort("");
      setDescending(false);
      setOffset(0);
    }
  }, [query.data, query.isPlaceholderData, sort, visibleColumns]);
  function saveColumns(columns: string[] | undefined) {
    const next = {...columnPreferences};
    if (columns === undefined) delete next[tab.id];
    else next[tab.id] = [...columns];
    setColumnPreferences(next);
    persist(columnPreferencesKey, next);
    setColumnsOpen(false);
  }
  const issue = issueById.get(issueId);
  const exportView = useMemo<ExplorerExportView>(() => ({
    id, tabId,
    selection: { request, columns: visibleColumns.filter((name) => query.data?.columns.some((column) => column.name === name)) },
    ready: !!query.data && !query.isFetching && !query.isPlaceholderData && !query.error && search === debounced && (!sort || visibleColumns.includes(sort)),
  }), [id, tabId, request, visibleColumns, query.data, query.isFetching, query.isPlaceholderData, query.error, search, debounced]);
  useLayoutEffect(() => {
    onExportView(exportView);
    return () => onExportView(null);
  }, [exportView, onExportView]);
  const hasStatus = ["internal", "external", "links", "sitemaps", "sitemap-urls"].includes(
    tab.backend,
  );
  const total = query.data?.total ?? 0;
  const searchKey = linkScope ? linkScope.direction === "incoming" ? "searchLinkSources" : "searchLinkDestinations" : "search";
  const hasFilters = !!(search || debounced || status || indexability || issueId || responseGroup || linkScope || linkClass || duplicateScope);
  function clearFilters() {
    setSearch("");
    setDebounced("");
    setStatus(0);
    setIndexability("");
    onClearIssue();
    onResponseGroup("");
    setLinkScope(null);
    setLinkClass(null);
    setDuplicateScope(null);
    setOffset(0);
    setSelected("");
    searchInput.current?.focus();
  }
  function openLinks(scope: LinkScope, classification?: LinkClass) {
    onClearIssue();
    setDuplicateScope(null);
    setLinkScope(scope);
    setLinkClass(classification ?? null);
    setSearch("");
    setDebounced("");
    setStatus(0);
    setIndexability("");
    setSort("");
    setDescending(false);
    setOffset(0);
    setSelected("");
    onTab("links");
  }
  function openDuplicates(scope: DuplicateScope) {
    onClearIssue();
    setLinkScope(null);
    setLinkClass(null);
    setDuplicateScope(scope);
    setSearch("");
    setDebounced("");
    setStatus(0);
    setIndexability("");
    setSort("address");
    setDescending(false);
    setOffset(0);
    setSelected("");
    onTab("internal");
  }
  function openIssue(id: string) {
    setLinkScope(null);
    setLinkClass(null);
    setDuplicateScope(null);
    setSearch("");
    setDebounced("");
    setStatus(0);
    setIndexability("");
    setSort("");
    setDescending(false);
    setOffset(0);
    setSelected("");
    onIssue(id);
  }
  return (
    <div className="explorer-page">
      <div className="explorer-tabs oc-tabs">
        {tabs.map((item) => (
          <button
            className={`oc-tab ${tabId === item.id ? "active" : ""}`}
            key={item.id}
            ref={tabId === item.id ? activeTab : undefined}
            onClick={() => {
              onClearIssue();
              setLinkScope(null);
              setLinkClass(null);
              setDuplicateScope(null);
              onTab(item.id);
            }}
          >
            {t(item.label)}
          </button>
        ))}
      </div>
      {tab.backend === "content-matches" && <p className="content-match-note">{t("contentMatchCriteria")}</p>}
      {["internal", "external"].includes(tab.id) && <p className="content-match-note">{t("linkCountColumnsNote")}</p>}
      <section className="oc-panel explorer-table">
        <div className="filter-toolbar">
          <div className="search-input">
            <Search size={16} />
            <input
              ref={searchInput}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              aria-label={t(searchKey)}
              placeholder={t(searchKey)}
            />
            {search && (
              <button onClick={() => setSearch("")} aria-label={t("clearSearch")} title={t("clearSearch")}>
                <X size={14} />
              </button>
            )}
          </div>
          {hasStatus && (
            <select
              className="oc-select"
              value={status}
              onChange={(e) => setStatus(Number(e.target.value))}
              aria-label={t(tab.id === "links" ? "destination_status" : "status")}
            >
              <option value={0}>
                {t(tab.id === "links" ? "destination_status" : "status")}: {t("all")}
              </option>
              {[2, 3, 4, 5].map((code) => (
                <option key={code} value={code}>
                  {code}xx
                </option>
              ))}
            </select>
          )}
          {tab.id === "response-codes" && <select className="oc-select" value={responseGroup} onChange={(e) => onResponseGroup(e.target.value as ResponseGroup | "")} aria-label={t("responseGroup")}>
            <option value="">{t("responseGroup")}: {t("all")}</option>
            {responseGroups.map((group) => <option key={group.group} value={group.group}>{t(group.label)}</option>)}
          </select>}
          {["internal", "sitemap-urls"].includes(tab.backend) && (
            <select
              className="oc-select"
              value={indexability}
              onChange={(e) => setIndexability(e.target.value)}
              aria-label={t("indexability")}
            >
              <option value="">
                {t("indexability")}: {t("all")}
              </option>
              <option value="indexable">{t("indexableLabel")}</option>
              <option value="noindex">noindex</option>
              <option value="canonicalised">canonicalised</option>
              <option value="blocked-by-robots">robots.txt</option>
            </select>
          )}
          {tab.backend === "links" && <select className="oc-select" aria-label={t(linkScope?.direction === "incoming" ? "linkSourceClass" : "linkDestinationClass")} value={linkClass ?? ""} onChange={event => setLinkClass((event.target.value || null) as LinkClass | null)}>
            <option value="">{t(linkScope?.direction === "incoming" ? "linkSourceClass" : "linkDestinationClass")}: {t("all")}</option>
            <option value="internal">{t("linkInternal")}</option><option value="external">{t("linkExternal")}</option>
          </select>}
          {hasFilters && <Button small className="ghost" onClick={clearFilters}><X size={14} />{t("clear")}</Button>}
          <span className="toolbar-count">
            {number(total, lang)} {t("rows")}
            {query.isFetching && <span className="loading-dot" />}
          </span>
          <Button small disabled={!query.data || query.isFetching || query.isPlaceholderData || !!query.error} onClick={() => setColumnsOpen(true)} aria-label={t("tableColumns")}>
            <Columns3 size={14}/>{t("tableColumns")} <span className="faint">{number(visibleColumns.length,lang)}</span>
          </Button>
        </div>
        {issue && (
          <div className="active-filter">
            <Filter size={14} />
            <span>{issue[lang].title}</span>
            <button aria-label={t("clearIssueFilter")} title={t("clearIssueFilter")} onClick={onClearIssue}>
              <X size={14} />
            </button>
          </div>
        )}
        {tab.backend === "links" && !query.isPlaceholderData && query.data?.link_summary && <div className="link-evidence-summary">
          <strong>{number(query.data.link_summary.sources, lang)} {t("linkSourceCount")} · {number(query.data.link_summary.destinations, lang)} {t("linkDestinationCount")} · {number(total, lang)} {t("linkRecordCount")}</strong>
          {isLinkEvidenceIssue(issueId) && <p>{t("linkEvidenceNote")}</p>}
        </div>}
        {tab.id === "links" && linkScope && (
          <div className="active-filter link-scope">
            <Filter size={14} />
            <strong>{t(linkScope.direction === "incoming" ? "incomingTo" : "outgoingFrom")}</strong>
            <span title={linkScope.url}>{linkScope.url}</span>
            <button aria-label={t("clearLinkScope")} onClick={() => { setLinkScope(null); setLinkClass(null); }}><X size={14} /></button>
          </div>
        )}
        {tab.id === "internal" && duplicateScope && <div className="active-filter link-scope">
          <Filter size={14} /><strong>{t(duplicateLabels[duplicateScope.kind])}</strong>
          <span title={duplicateScope.url}>{t("duplicateGroupFrom")} {duplicateScope.url}</span>
          <button aria-label={t("clearDuplicateScope")} onClick={() => setDuplicateScope(null)}><X size={14} /></button>
        </div>}
        <div
          className={`table-stage ${query.isPlaceholderData ? "updating" : ""}`}
        >
          {query.isPending ? (
            <Loading />
          ) : query.error ? (
            <ErrorState
              message={errorText(query.error)}
              retry={() => void query.refetch()}
            />
          ) : (
            query.data && (
              <DataTable
                data={query.data}
                visible={visibleColumns}
                sort={sort}
                descending={descending}
                onSort={(column) => {
                  setDescending(column === sort ? !descending : false);
                  setSort(column);
                  setOffset(0);
                }}
                selected={selected}
                selectColumn={ tab.id === "sitemap-urls" ? (query.data.columns.some(column=>column.name==="crawl_url") ? "crawl_url" : "url") : tab.id === "links" && (linkScope?.direction === "outgoing" || isLinkEvidenceIssue(issueId)) ? "destination" : undefined}
                onSelect={
                  ["sitemaps", "robots"].includes(tab.id)
                    ? undefined
                    : setSelected
                }
              />
            )
          )}
        </div>
        <footer className="table-footer">
          <span>
            {total
              ? `${number(offset + 1, lang)}–${number(Math.min(offset + 250, total), lang)} / ${number(total, lang)}`
              : `0 ${t("rows")}`}
          </span>
          <div className="row">
            <Button
              small
              className="icon ghost"
              aria-label={t("previous")}
              disabled={offset === 0 || query.isPlaceholderData}
              onClick={() => setOffset(Math.max(0, offset - 250))}
            >
              <ChevronLeft size={16} />
            </Button>
            <Button
              small
              className="icon ghost"
              aria-label={t("next")}
              disabled={offset + 250 >= total || query.isPlaceholderData}
              onClick={() => setOffset(offset + 250)}
            >
              <ChevronRight size={16} />
            </Button>
          </div>
        </footer>
      </section>
      {columnsOpen && query.data && !query.isPlaceholderData && !query.error && <ColumnSettings
        key={`${id}:${tab.id}`} tabLabel={t(tab.label)} schema={schemaColumns} defaults={tab.columns}
        saved={columnPreferences[tab.id]} required={requiredColumns} onApply={saveColumns} onClose={() => setColumnsOpen(false)}
      />}
      {selected && (
        <Inspector
          key={`${id}:${selected}`}
          id={id}
          url={selected}
          onClose={() => setSelected("")}
          onLinks={openLinks}
          onInspect={setSelected}
          onDuplicates={openDuplicates}
          onIssue={openIssue}
          onSchema={onSchema}
          excluded={excluded}
          activeIssueId={issueId}
          initialTab={ duplicateScope || duplicateKindForIssue(issueId) ? "duplicates" : tab.backend === "content-matches" ? "contentMatch" : tab.backend === "rendering" ? "renderComparison" : issueId ? "urlFindings" : "facts"}
        />
      )}
      <div className="explorer-hint">{tab.backend === "robots" ? t("robotsStatusHint") : tab.backend === "hreflang" ? t("hreflangEligibilityHint") : tab.backend === "sitemaps" ? t("sitemapRedirectHint") : tab.backend === "sitemap-urls" ? t("sitemapUrlsHint") : tab.backend === "images" ? t("responsiveImagesHint") : t("selectUrl")}</div>
    </div>
  );
}
