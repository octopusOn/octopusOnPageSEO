import { useEffect, useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp, Ban, Download, RotateCcw, Search, Star, Tag } from "lucide-react";
import { api, errorText, type ExportFormat } from "../api/client";
import type { KeywordQuery, KeywordRow, KeywordSort, KeywordStatus, ProjectRow } from "../api/types";
import { Button, Empty, ErrorState, Loading } from "../components/ui";
import { number, usd, useI18n, type Key } from "../i18n";
import { difficultyBand, sparkline } from "../lib/format";

const PAGE = 500;
const ROW_HEIGHT = 38;

const COLUMNS: { id: string; label: Key; sort?: KeywordSort; numeric?: boolean }[] = [
  { id: "keyword", label: "keyword", sort: "keyword" },
  { id: "volume", label: "volume", sort: "volume", numeric: true },
  { id: "trend", label: "trend" },
  { id: "cpc", label: "cpc", sort: "cpc", numeric: true },
  { id: "competition", label: "competitionShort", sort: "competition", numeric: true },
  { id: "difficulty", label: "difficultyShort", sort: "difficulty", numeric: true },
  { id: "intent", label: "intent", sort: "intent" },
  { id: "cluster", label: "cluster", sort: "cluster", numeric: true },
  { id: "sources", label: "sources" },
];

function useDebounced<T>(value: T, delay = 250): T {
  const [current, setCurrent] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setCurrent(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return current;
}

function Trend({ row }: { row: KeywordRow }) {
  const path = sparkline(row.monthly.map((month) => month.volume));
  if (!path) return <span className="faint">–</span>;
  return (
    <svg className="sparkline" viewBox="0 0 64 18" width="64" height="18" aria-hidden="true">
      <path d={path} />
    </svg>
  );
}

export function Keywords({
  project,
  notify,
  initialCluster,
}: {
  project: ProjectRow;
  notify: (text: string, error?: boolean) => void;
  initialCluster?: number | null;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<KeywordStatus>("active");
  const [intent, setIntent] = useState("");
  const [source, setSource] = useState("");
  const [tag, setTag] = useState("");
  const [minVolume, setMinVolume] = useState("");
  const [cluster, setCluster] = useState(initialCluster != null ? String(initialCluster + 1) : "");
  const [sort, setSort] = useState<KeywordSort>("volume");
  const [descending, setDescending] = useState(true);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const debouncedSearch = useDebounced(search);
  const query: Omit<KeywordQuery, "offset" | "limit"> = useMemo(
    () => ({
      search: debouncedSearch || null,
      status,
      intent: intent || null,
      source: source || null,
      tag: tag || null,
      min_volume: minVolume ? Number(minVolume) : null,
      cluster: cluster ? Math.max(0, Number(cluster) - 1) : null,
      sort,
      descending,
    }),
    [debouncedSearch, status, intent, source, tag, minVolume, cluster, sort, descending],
  );
  const facets = useQuery({ queryKey: ["facets", project.id], queryFn: () => api.facets(project.id) });
  const pages = useInfiniteQuery({
    queryKey: ["keywords", project.id, query],
    initialPageParam: 0,
    queryFn: ({ pageParam }) => api.keywords(project.id, { ...query, offset: pageParam, limit: PAGE }),
    getNextPageParam: (last, all) => {
      const loaded = all.reduce((sum, page) => sum + page.rows.length, 0);
      return loaded < last.total ? loaded : undefined;
    },
  });
  const rows = useMemo(() => pages.data?.pages.flatMap((page) => page.rows) ?? [], [pages.data]);
  const total = pages.data?.pages[0]?.total ?? 0;
  useEffect(() => setSelected(new Set()), [query]);

  const scroller = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });
  const items = virtualizer.getVirtualItems();
  const lastIndex = items.at(-1)?.index ?? 0;
  useEffect(() => {
    if (lastIndex >= rows.length - 50 && pages.hasNextPage && !pages.isFetchingNextPage) void pages.fetchNextPage();
  }, [lastIndex, rows.length, pages]);

  const refresh = () => {
    setSelected(new Set());
    void queryClient.invalidateQueries({ queryKey: ["keywords", project.id] });
    void queryClient.invalidateQueries({ queryKey: ["facets", project.id] });
    void queryClient.invalidateQueries({ queryKey: ["projects"] });
  };
  const setStatusOf = useMutation({
    mutationFn: (next: KeywordStatus) => api.setStatus(project.id, [...selected], next),
    onSuccess: (changed, next) => {
      notify(t(next === "excluded" ? "excludedCount" : "includedCount", { count: changed }));
      refresh();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const tagSelected = useMutation({
    mutationFn: ({ add, remove }: { add: string[]; remove: string[] }) => api.tag(project.id, [...selected], add, remove),
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const exportAll = useMutation({
    mutationFn: (format: ExportFormat) =>
      api.exportKeywords(project.id, { ...query, offset: 0, limit: 10000 }, `${project.name}-keywords`, format),
    onSuccess: (written) => written !== null && notify(t("exported", { count: written })),
    onError: (error) => notify(errorText(error), true),
  });

  const sortBy = (column: KeywordSort) => {
    if (sort === column) setDescending(!descending);
    else {
      setSort(column);
      setDescending(column !== "keyword" && column !== "intent" && column !== "cluster");
    }
  };
  const toggle = (id: number) =>
    setSelected((old) => {
      const next = new Set(old);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const allShown = rows.length > 0 && rows.every((row) => selected.has(row.id));
  return (
    <div className="stack keywords-page">
      <div className="toolbar">
        <label className="oc-search">
          <Search size={15} />
          <input value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("searchKeywords")} aria-label={t("searchKeywords")} />
        </label>
        <select className="oc-select" value={status} onChange={(e) => setStatus(e.target.value as KeywordStatus)} aria-label={t("status")}>
          <option value="active">{t("statusActive")}</option>
          <option value="excluded">{t("statusExcluded")}</option>
        </select>
        <select className="oc-select" value={intent} onChange={(e) => setIntent(e.target.value)} aria-label={t("intent")}>
          <option value="">{t("anyIntent")}</option>
          {["informational", "commercial", "transactional", "navigational", "local"].map((value) => (
            <option key={value} value={value}>
              {t(`intent.${value}` as Key)}
            </option>
          ))}
        </select>
        <select className="oc-select" value={source} onChange={(e) => setSource(e.target.value)} aria-label={t("source")}>
          <option value="">{t("anySource")}</option>
          {(facets.data?.sources ?? []).map(([name, count]) => (
            <option key={name} value={name}>
              {t(`source.${name}` as Key)} ({number(count, lang, 0)})
            </option>
          ))}
        </select>
        {(facets.data?.tags.length ?? 0) > 0 && (
          <select className="oc-select" value={tag} onChange={(e) => setTag(e.target.value)} aria-label={t("tag")}>
            <option value="">{t("anyTag")}</option>
            {(facets.data?.tags ?? []).map(([name, count]) => (
              <option key={name} value={name}>
                {name} ({count})
              </option>
            ))}
          </select>
        )}
        <input className="oc-input narrow" type="number" min={0} value={minVolume} onChange={(e) => setMinVolume(e.target.value)} placeholder={t("minVolume")} aria-label={t("minVolume")} />
        <input className="oc-input narrow" type="number" min={1} value={cluster} onChange={(e) => setCluster(e.target.value)} placeholder={t("clusterNumber")} aria-label={t("clusterNumber")} />
        <span className="toolbar-spacer" />
        <span className="faint">{t("keywordCount", { count: number(total, lang, 0) })}</span>
        <select
          className="oc-select"
          value=""
          aria-label={t("export")}
          onChange={(e) => {
            if (e.target.value) exportAll.mutate(e.target.value as ExportFormat);
          }}
        >
          <option value="">{t("export")}…</option>
          <option value="xlsx">Excel</option>
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
        </select>
      </div>
      {selected.size > 0 && (
        <div className="selection-bar">
          <b>{t("selectedCount", { count: selected.size })}</b>
          {status === "active" ? (
            <Button small onClick={() => setStatusOf.mutate("excluded")}>
              <Ban size={14} />
              {t("exclude")}
            </Button>
          ) : (
            <Button small onClick={() => setStatusOf.mutate("active")}>
              <RotateCcw size={14} />
              {t("include")}
            </Button>
          )}
          <Button
            small
            onClick={() => {
              const name = window.prompt(t("tagPrompt"))?.trim();
              if (name) tagSelected.mutate({ add: [name], remove: [] });
            }}
          >
            <Tag size={14} />
            {t("addTag")}
          </Button>
          {tag && (
            <Button small onClick={() => tagSelected.mutate({ add: [], remove: [tag] })}>
              {t("removeTag", { tag })}
            </Button>
          )}
          <Button small className="ghost" onClick={() => setSelected(new Set())}>
            {t("clearSelection")}
          </Button>
        </div>
      )}
      {pages.isLoading ? (
        <Loading />
      ) : pages.error ? (
        <ErrorState message={errorText(pages.error)} retry={() => void pages.refetch()} />
      ) : rows.length === 0 ? (
        <Empty>
          <b>{t("noKeywords")}</b>
          <p>{t("noKeywordsLead")}</p>
        </Empty>
      ) : (
        <div className="grid-table" role="table" aria-rowcount={total}>
          <div className="grid-inner">
          <div className="grid-row grid-head" role="row">
            <span role="columnheader">
              <input
                type="checkbox"
                checked={allShown}
                aria-label={t("selectShown")}
                onChange={() => setSelected(allShown ? new Set() : new Set(rows.map((row) => row.id)))}
              />
            </span>
            {COLUMNS.map((column) => (
              <span key={column.id} role="columnheader" className={column.numeric ? "num" : ""}>
                {column.sort ? (
                  <button className="sort" onClick={() => sortBy(column.sort as KeywordSort)}>
                    {t(column.label)}
                    {sort === column.sort && (descending ? <ArrowDown size={12} /> : <ArrowUp size={12} />)}
                  </button>
                ) : (
                  t(column.label)
                )}
              </span>
            ))}
          </div>
          <div ref={scroller} className="grid-body">
            <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
              {items.map((item) => {
                const row = rows[item.index];
                if (!row) return null;
                const band = difficultyBand(row.difficulty);
                return (
                  <div
                    key={row.id}
                    role="row"
                    className={`grid-row ${selected.has(row.id) ? "selected" : ""}`}
                    style={{ position: "absolute", top: 0, left: 0, right: 0, height: ROW_HEIGHT, transform: `translateY(${item.start}px)` }}
                  >
                    <span role="cell">
                      <input type="checkbox" checked={selected.has(row.id)} onChange={() => toggle(row.id)} aria-label={row.keyword} />
                    </span>
                    <span role="cell" className="keyword-cell" title={row.tags.length ? row.tags.join(", ") : undefined}>
                      {row.pillar && <Star size={12} className="pillar-star" aria-label={t("pillar")} />}
                      {row.keyword}
                      {row.tags.map((name) => (
                        <span key={name} className="oc-badge tag">
                          {name}
                        </span>
                      ))}
                    </span>
                    <span role="cell" className="num">
                      {row.volume === null ? <span className="faint">–</span> : number(row.volume, lang, 0)}
                    </span>
                    <span role="cell">
                      <Trend row={row} />
                    </span>
                    <span role="cell" className="num">
                      {row.cpc_usd === null ? "" : usd(row.cpc_usd, lang)}
                    </span>
                    <span role="cell" className="num">
                      {row.competition === null ? "" : number(row.competition, lang, 2)}
                    </span>
                    <span role="cell" className="num">
                      {row.difficulty === null ? "" : <span className={`kd ${band}`}>{number(row.difficulty, lang, 0)}</span>}
                    </span>
                    <span role="cell">{row.intent && <span className={`intent ${row.intent}`}>{t(`intent.${row.intent}` as Key)}</span>}</span>
                    <span role="cell" className="num">
                      {row.cluster === null ? "" : row.cluster + 1}
                      {row.serp && <span className="serp-dot" title={t("hasSerp")} />}
                    </span>
                    <span role="cell" className="faint clip">
                      {row.sources.map((name) => t(`source.${name}` as Key)).join(", ")}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
          {pages.isFetchingNextPage && <div className="faint small grid-foot">{t("loading")}</div>}
          </div>
        </div>
      )}
      <p className="faint small">
        <Download size={12} /> {t("exportHint")}
      </p>
    </div>
  );
}
