import { useEffect, useMemo, useState, type ReactNode } from "react";
import { ArrowDownRight, ArrowUpRight, Download, FileText, Search, Sparkles, X } from "lucide-react";
import { Button, DataTable, type Column, type SortDirection } from "@octopusseo/ui";
import type { KeywordSet, RankedKeyword, RankedKeywords } from "../../api/types";
import { useI18n } from "../../i18n";
import { formatNumber, formatUsd } from "../../lib/format";
import { download, featureCounts, filterKeywords, NO_FILTER, toCsv } from "../../lib/report";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { FilterBar, type FilterPreset } from "../../lib/FilterBar";
import { useGlossary } from "../../lib/glossary";
import { filterRows, RANKED_KEYWORD_ACCESSORS, RANKED_KEYWORD_FILTERS, type QueryFilter, type QueryOrder } from "../../lib/query";
import { featureName, PAGE_SIZES, shortUrl, tableLabels } from "../../lib/tables";
import { useListQuery } from "../../lib/useListQuery";
import { ReportFrame } from "./ReportFrame";

/** No fixed filters. */
const NO_FILTERS: QueryFilter[] = [];

/** The table column a sorting field reads. */
const SORT_COLUMNS: Record<string, string> = { kd: "difficulty", previous: "position" };

function tableSort(order: readonly QueryOrder[]): { key: string; direction: SortDirection } {
  const first = order[0];
  return first ? { key: SORT_COLUMNS[first.field] ?? first.field, direction: first.direction } : { key: "traffic", direction: "desc" };
}

/** Ready filters of a domain's keywords. */
export function keywordPresets(t: (key: string) => string): FilterPreset[] {
  const position = (op: QueryFilter["op"], value: number): QueryFilter => ({ field: "position", op, value });
  return [
    { id: "top3", label: t("overview.posTop3"), filters: [position("le", 3)] },
    { id: "top10", label: t("query.preset4to10"), filters: [position("ge", 4), position("le", 10)] },
    { id: "striking", label: t("query.presetStriking"), filters: [position("ge", 11), position("le", 20)] },
    { id: "new", label: t("overview.changeNew"), filters: [{ field: "new", op: "eq", value: true }] },
    { id: "up", label: t("query.fieldUp"), filters: [{ field: "up", op: "eq", value: true }] },
    { id: "down", label: t("query.fieldDown"), filters: [{ field: "down", op: "eq", value: true }] },
    { id: "ai", label: t("query.presetAi"), filters: [{ field: "features", op: "has", value: "ai_overview" }] },
    { id: "commercial", label: t("query.presetBuying"), filters: [{ field: "intent", op: "in", value: ["commercial", "transactional"] }] },
  ];
}

function Position({ row }: { row: RankedKeyword }) {
  const { t } = useI18n();
  if (row.position === null) return <span className="faint">—</span>;
  const moved = row.previous_position !== null && row.absolute_position !== null ? row.previous_position - row.absolute_position : 0;
  return (
    <span className="position">
      {row.position}
      {row.change === "new" && <span className="oc-badge success small-badge">{t("overview.changeNew")}</span>}
      {row.change === "up" && moved > 0 && (
        <span className="good">
          <ArrowUpRight size={12} aria-hidden="true" />
          {moved}
        </span>
      )}
      {row.change === "down" && moved < 0 && (
        <span className="warn">
          <ArrowDownRight size={12} aria-hidden="true" />
          {-moved}
        </span>
      )}
    </span>
  );
}

function KeywordsView({ data, domain, set, filters, order, onSeen }: { data: RankedKeywords; domain: string; set: KeywordSet; filters: QueryFilter[]; order: QueryOrder[]; onSeen: (rows: number, total: number) => void }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const [search, setSearch] = useState("");
  useEffect(() => onSeen(data.keywords.length, data.total), [data, onSeen]);
  const rows = useMemo(() => filterKeywords(filterRows(data.keywords, filters, RANKED_KEYWORD_ACCESSORS), { ...NO_FILTER, query: search }), [data.keywords, filters, search]);
  const features = useMemo(() => featureCounts(data.keywords).slice(0, 8), [data.keywords]);
  const intentName = (intent: string | null) => (intent ? t(`overview.intent_${intent}`) : "");
  const columns: Column<RankedKeyword>[] = [
    {
      key: "keyword",
      label: t("overview.colKeyword"),
      render: (row) => (
        <Entity kind="keyword" value={row.keyword}>
          <strong className="cell-main">{row.keyword}</strong>
        </Entity>
      ),
      sort: (row) => row.keyword,
    },
    set === "lost"
      ? { key: "position", label: t("query.fieldPrevious"), help: gloss("position"), render: (row) => (row.previous_position === null ? "—" : row.previous_position), sort: (row) => row.previous_position, numeric: true }
      : { key: "position", label: t("overview.colPosition"), help: gloss("position"), render: (row) => <Position row={row} />, sort: (row) => row.position, numeric: true },
    ...(set === "paid"
      ? [
          {
            key: "ad",
            label: t("overview.colAd"),
            render: (row: RankedKeyword) =>
              row.ad ? (
                <span className="ad-cell">
                  <strong>{row.ad.title}</strong>
                  {row.ad.description && <span className="faint small">{row.ad.description}</span>}
                </span>
              ) : (
                ""
              ),
          },
        ]
      : []),
    { key: "volume", label: t("overview.colVolume"), help: gloss("volume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true, total: (shown) => formatNumber(shown.reduce((sum, row) => sum + (row.volume ?? 0), 0), locale) },
    { key: "difficulty", label: t("overview.colKd"), help: gloss("kd"), optional: true, render: (row) => (row.difficulty === null ? "—" : <span className={`kd kd-${Math.min(4, Math.floor(row.difficulty / 25))}`}>{row.difficulty}</span>), sort: (row) => row.difficulty, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.traffic, locale), sort: (row) => row.traffic, numeric: true, total: (shown) => formatNumber(shown.reduce((sum, row) => sum + row.traffic, 0), locale) },
    { key: "value", label: t("overview.colValue"), help: gloss("traffic_value"), optional: true, defaultHidden: true, render: (row) => formatUsd(row.traffic_value_usd, locale), sort: (row) => row.traffic_value_usd, numeric: true, total: (shown) => formatUsd(shown.reduce((sum, row) => sum + row.traffic_value_usd, 0), locale) },
    { key: "cpc", label: t("overview.colCpc"), help: gloss("cpc"), optional: true, render: (row) => (row.cpc === null ? "—" : formatUsd(row.cpc, locale)), sort: (row) => row.cpc, numeric: true },
    { key: "competition", label: t("overview.colCompetition"), help: gloss("competition"), optional: true, defaultHidden: true, render: (row) => (row.competition === null ? "—" : row.competition.toFixed(2)), sort: (row) => row.competition, numeric: true },
    { key: "intent", label: t("overview.colIntent"), help: gloss("intent"), optional: true, render: (row) => intentName(row.intent), sort: (row) => row.intent },
    {
      key: "url",
      label: t("overview.colUrl"),
      render: (row) =>
        row.url ? (
          <Entity kind="url" value={row.url}>
            <span className="cell-url">{shortUrl(row.url)}</span>
          </Entity>
        ) : (
          ""
        ),
      sort: (row) => row.url,
    },
    {
      key: "features",
      help: gloss("serp_features"),
      optional: true,
      label: t("overview.colFeatures"),
      render: (row) =>
        row.serp_features.includes("ai_overview") ? (
          <span className="feature-ai" title={row.serp_features.map((feature) => featureName(feature, t)).join(", ")}>
            <Sparkles size={13} aria-hidden="true" /> AI
          </span>
        ) : (
          <span className="faint" title={row.serp_features.map((feature) => featureName(feature, t)).join(", ")}>
            {row.serp_features.length || ""}
          </span>
        ),
      sort: (row) => row.serp_features.length,
      numeric: true,
    },
  ];
  const exportCsv = () =>
    download(
      `${domain}-keywords.csv`,
      toCsv(
        [t("overview.colKeyword"), t("overview.colPosition"), t("overview.colVolume"), t("overview.colKd"), t("overview.colTraffic"), t("overview.colCpc"), t("overview.colIntent"), t("overview.colUrl"), t("overview.colFeatures")],
        rows.map((row) => [row.keyword, row.position, row.volume, row.difficulty, Math.round(row.traffic), row.cpc, row.intent, row.url, row.serp_features.join(" ")]),
      ),
    );
  return (
    <div className="page-stack">
      <p className="faint small">{t("overview.keywordsTotal", { shown: formatNumber(data.keywords.length, locale), total: formatNumber(data.total, locale) })}</p>
      {features.length > 0 && (
        <div className="feature-chips" aria-label={t("overview.features")}>
          {features.map(([feature, count]) => (
            <span key={feature} className="oc-chip">
              {featureName(feature, t)} <strong>{formatNumber(count, locale)}</strong>
            </span>
          ))}
        </div>
      )}
      <div className="filters">
        <label className="filter-search">
          <Search size={15} aria-hidden="true" />
          <span className="sr-only">{t("query.searchLoaded")}</span>
          <input className="oc-input" value={search} placeholder={t("query.searchLoaded")} onChange={(event) => setSearch(event.target.value)} />
        </label>
        <Button small onClick={exportCsv} disabled={rows.length === 0}>
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable
        key={JSON.stringify(order)}
        rows={rows}
        columns={columns}
        rowKey={(row) => `${row.keyword}|${row.url ?? ""}`}
        initialSort={tableSort(order)}
        tableId={`overview-keywords-${set}`}
        pageSizes={PAGE_SIZES}
        empty={t("overview.emptyRows")}
        labels={tableLabels(t, locale)}
      />
    </div>
  );
}

/**
 * A list of a domain's keywords with its filter bar: organic ones now (the page's own when the
 * address names one), lost ones, or paid ones with their ads. `base` filters are part of the
 * question (a change tab's `up` …), not the bar's.
 */
export function KeywordsReport({
  projectId,
  domain,
  shownDomain,
  set = "organic",
  base = NO_FILTERS,
  title,
  hint,
  extra,
}: {
  projectId: number;
  domain: string | undefined;
  shownDomain: string;
  set?: KeywordSet;
  base?: QueryFilter[];
  title?: string;
  hint?: string;
  extra?: ReactNode;
}) {
  const { t } = useI18n();
  const { params, go } = useShell();
  const market = params.market;
  // A page's keywords: the address names the page.
  const page = set === "organic" && base.length === 0 ? params.url : undefined;
  const fixed = useMemo<QueryFilter[]>(() => (page ? [...base, { field: "url", op: "eq", value: page }] : base), [base, page]);
  const [limit, setLimit] = useState(100);
  const list = useListQuery(JSON.stringify([projectId, domain, market, limit, set, fixed]));
  const presets = useMemo(() => (set === "organic" && base.length === 0 ? keywordPresets(t) : []), [t, set, base.length]);
  const asked = fixed.length > 0 ? { ...list.asked, filters: [...fixed, ...list.asked.filters] } : list.asked;
  return (
    <ReportFrame
      report="keywords"
      projectId={projectId}
      domain={domain}
      market={market}
      more={set === "organic" ? undefined : { set }}
      title={title ?? t("overview.keywordsTitle")}
      hint={hint ?? t("overview.keywordsHint")}
      limit={limit}
      onLimit={setLimit}
      extra={extra}
      query={asked}
      onOffset={list.setOffset}
      toolbar={
        <>
          {page && (
            <div className="oc-banner" role="status">
              <FileText size={15} aria-hidden="true" />
              <span>{t("overview.pageKeywords", { page: shortUrl(page) })}</span>
              <Button small className="ghost" onClick={() => go({ params: { ...params, url: undefined } })}>
                <X size={14} aria-hidden="true" />
                {t("overview.allPages")}
              </Button>
            </div>
          )}
          <FilterBar fields={RANKED_KEYWORD_FILTERS} query={list.applied} onChange={list.change} scope={list.scope} presets={presets} />
        </>
      }
    >
      {(data) => <KeywordsView data={data} domain={shownDomain} set={set} filters={list.local} order={list.applied.order} onSeen={list.seen} />}
    </ReportFrame>
  );
}
