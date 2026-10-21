import { useMemo, useState, type FormEvent } from "react";
import { Download, GitCompareArrows, Search } from "lucide-react";
import { Button, DataTable, Empty, Field, type Column } from "@octopusseo/ui";
import type { BulkRow, GapKeyword, KeywordData, KeywordGap, LinkGapRow, ProjectRow, Ranking } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { download, fold, toCsv } from "../../lib/report";
import { tableLabels } from "../../lib/tables";
import { Authority } from "../backlinks/Backlinks";
import { AddToList, Difficulty } from "../keywords/KeywordTable";

const TABS = ["missing", "shared", "links", "bulk"] as const;
type Tab = (typeof TABS)[number];

/** Most competitors one gap compares. */
const MAX_COMPETITORS = 5;

export interface GapFilters {
  query: string;
  minCompetitors: number;
  minVolume: number;
  maxDifficulty: number;
  weakOnly: boolean;
}

export const ALL_GAPS: GapFilters = { query: "", minCompetitors: 1, minVolume: 0, maxDifficulty: 100, weakOnly: false };

/** The gap keywords a filter keeps. */
export function filterGap(rows: readonly GapKeyword[], filters: GapFilters): GapKeyword[] {
  const query = fold(filters.query.trim());
  return rows.filter(
    (row) =>
      (!query || fold(row.keyword).includes(query)) &&
      row.competitors.length >= filters.minCompetitors &&
      (row.volume ?? 0) >= filters.minVolume &&
      (filters.maxDifficulty >= 100 || (row.difficulty ?? 0) <= filters.maxDifficulty) &&
      (!filters.weakOnly || row.weak),
  );
}

/** Each site's share of the traffic of all, in percent. */
export function trafficShares(rows: readonly BulkRow[]): Map<string, number> {
  const total = rows.reduce((sum, row) => sum + row.organic_traffic, 0);
  return new Map(rows.map((row) => [row.target, total > 0 ? (row.organic_traffic / total) * 100 : 0]));
}

/** A gap keyword as a list keeps it. */
function asKeyword(row: GapKeyword): KeywordData {
  return {
    keyword: row.keyword,
    volume: row.volume,
    cpc: row.cpc,
    competition: null,
    competition_level: null,
    difficulty: row.difficulty,
    intent: row.intent,
    other_intents: [],
    monthly: [],
    trend: { monthly: null, quarterly: null, yearly: null },
    serp_features: [],
    results: null,
    top_backlinks: null,
    top_referring_domains: null,
  };
}

function Position({ ranking }: { ranking: Ranking | undefined | null }) {
  if (!ranking || ranking.position === null) return <span className="faint">—</span>;
  return (
    <span className="position" title={ranking.url ?? ""}>
      {ranking.position}
    </span>
  );
}

function KeywordGapView({ gap, projectId, shared }: { gap: KeywordGap; projectId: number; shared: boolean }) {
  const { t, locale } = useI18n();
  const [filters, setFilters] = useState<GapFilters>(ALL_GAPS);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const rows = useMemo(() => filterGap(gap.keywords, filters), [gap.keywords, filters]);
  const columns: Column<GapKeyword>[] = [
    {
      key: "select",
      label: "",
      width: "36px",
      render: (row) => (
        <input
          type="checkbox"
          checked={selected.has(row.keyword)}
          aria-label={t("keywords.select", { keyword: row.keyword })}
          onChange={() =>
            setSelected((previous) => {
              const next = new Set(previous);
              if (next.has(row.keyword)) next.delete(row.keyword);
              else next.add(row.keyword);
              return next;
            })
          }
        />
      ),
    },
    {
      key: "keyword",
      label: t("overview.colKeyword"),
      render: (row) => (
        <span className="cell-main">
          {row.keyword}
          {row.weak && <span className="oc-badge warning small-badge">{t("gap.weak")}</span>}
        </span>
      ),
      sort: (row) => row.keyword,
    },
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    { key: "difficulty", label: t("overview.colKd"), render: (row) => <Difficulty value={row.difficulty} />, sort: (row) => row.difficulty, numeric: true },
    ...(shared
      ? [{ key: "own", label: gap.own, render: (row: GapKeyword) => <Position ranking={row.own} />, sort: (row: GapKeyword) => row.own?.position ?? null, numeric: true } satisfies Column<GapKeyword>]
      : []),
    ...gap.competitors.map(
      (competitor) =>
        ({
          key: `c-${competitor}`,
          label: competitor,
          render: (row: GapKeyword) => <Position ranking={row.competitors.find((ranking) => ranking.domain === competitor)} />,
          sort: (row: GapKeyword) => row.competitors.find((ranking) => ranking.domain === competitor)?.position ?? null,
          numeric: true,
        }) satisfies Column<GapKeyword>,
    ),
    { key: "count", label: t("gap.colCompetitors"), render: (row) => row.competitors.length, sort: (row) => row.competitors.length, numeric: true },
  ];
  const chosen = gap.keywords.filter((row) => selected.has(row.keyword)).map(asKeyword);
  return (
    <div className="page-stack">
      <div className="filters">
        <label className="filter-search">
          <Search size={15} aria-hidden="true" />
          <span className="sr-only">{t("keywords.filterSearch")}</span>
          <input className="oc-input" value={filters.query} placeholder={t("keywords.filterSearch")} onChange={(event) => setFilters({ ...filters, query: event.target.value })} />
        </label>
        {gap.competitors.length > 1 && (
          <Field label={t("gap.minCompetitors")}>
            <select className="oc-select" value={filters.minCompetitors} onChange={(event) => setFilters({ ...filters, minCompetitors: Number(event.target.value) })}>
              {gap.competitors.map((_, index) => (
                <option key={index} value={index + 1}>
                  {t("gap.atLeast", { n: index + 1 })}
                </option>
              ))}
            </select>
          </Field>
        )}
        <Field label={t("overview.minVolume")}>
          <input className="oc-input narrow" inputMode="numeric" value={filters.minVolume || ""} onChange={(event) => setFilters({ ...filters, minVolume: Number(event.target.value.replace(/\D/g, "")) || 0 })} />
        </Field>
        <Field label={t("keywords.maxDifficulty")}>
          <input
            className="oc-input narrow"
            inputMode="numeric"
            value={filters.maxDifficulty >= 100 ? "" : filters.maxDifficulty}
            onChange={(event) => setFilters({ ...filters, maxDifficulty: Math.min(100, Number(event.target.value.replace(/\D/g, "")) || 100) })}
          />
        </Field>
        {shared && (
          <label className="oc-check">
            <input type="checkbox" checked={filters.weakOnly} onChange={(event) => setFilters({ ...filters, weakOnly: event.target.checked })} />
            <span>{t("gap.weakOnly")}</span>
          </label>
        )}
      </div>
      <div className="table-toolbar">
        <span className="faint small">{t("gap.shown", { n: formatNumber(rows.length, locale), total: formatNumber(gap.keywords.length, locale) })}</span>
        <AddToList projectId={projectId} keywords={chosen} onDone={() => setSelected(new Set())} />
        <Button
          small
          disabled={rows.length === 0}
          onClick={() =>
            download(
              `${gap.own}-${shared ? "shared" : "missing"}.csv`,
              toCsv(
                [t("overview.colKeyword"), t("overview.colVolume"), t("overview.colKd"), ...(shared ? [gap.own] : []), ...gap.competitors],
                rows.map((row) => [
                  row.keyword,
                  row.volume,
                  row.difficulty,
                  ...(shared ? [row.own?.position ?? null] : []),
                  ...gap.competitors.map((competitor) => row.competitors.find((ranking) => ranking.domain === competitor)?.position ?? null),
                ]),
              ),
            )
          }
        >
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable rows={rows} columns={columns} rowKey={(row) => row.keyword} initialSort={{ key: "count", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}

function LinkGapView({ rows, competitors }: { rows: LinkGapRow[]; competitors: string[] }) {
  const { t, locale } = useI18n();
  const columns: Column<LinkGapRow>[] = [
    { key: "domain", label: t("overview.colDomain"), render: (row) => <strong className="cell-main">{row.domain}</strong>, sort: (row) => row.domain },
    { key: "rank", label: "OA", render: (row) => <Authority rank={row.rank} />, sort: (row) => row.rank, numeric: true },
    ...competitors.map(
      (competitor) =>
        ({
          key: `c-${competitor}`,
          label: competitor,
          render: (row: LinkGapRow) => formatNumber(row.backlinks[competitor] ?? 0, locale),
          sort: (row: LinkGapRow) => row.backlinks[competitor] ?? 0,
          numeric: true,
        }) satisfies Column<LinkGapRow>,
    ),
  ];
  return (
    <div className="page-stack">
      <div className="table-toolbar">
        <Button
          small
          disabled={rows.length === 0}
          onClick={() =>
            download(
              "link-gap.csv",
              toCsv(
                [t("overview.colDomain"), "OA", ...competitors],
                rows.map((row) => [row.domain, row.rank, ...competitors.map((competitor) => row.backlinks[competitor] ?? 0)]),
              ),
            )
          }
        >
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable rows={rows} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "rank", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}

function BulkView({ rows }: { rows: BulkRow[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const shares = useMemo(() => trafficShares(rows), [rows]);
  const option = useMemo<ChartOption>(
    () => ({
      tooltip: { trigger: "item", valueFormatter: (value: unknown) => `${Number(value).toFixed(1)}%` },
      legend: { bottom: 0, left: 0, type: "scroll" },
      series: [
        {
          type: "pie",
          radius: ["45%", "70%"],
          top: -20,
          itemStyle: { borderColor: tokens.raised, borderWidth: 2 },
          label: { color: tokens.text },
          data: rows.map((row) => ({ name: row.target, value: Number((shares.get(row.target) ?? 0).toFixed(1)) })),
        },
      ],
    }),
    [rows, shares, tokens],
  );
  const columns: Column<BulkRow>[] = [
    { key: "target", label: t("overview.colDomain"), render: (row) => <strong className="cell-main">{row.target}</strong>, sort: (row) => row.target },
    { key: "traffic", label: t("overview.organicTraffic"), render: (row) => formatCompact(row.organic_traffic, locale), sort: (row) => row.organic_traffic, numeric: true },
    { key: "share", label: t("gap.colShare"), render: (row) => `${(shares.get(row.target) ?? 0).toFixed(1)}%`, sort: (row) => shares.get(row.target) ?? 0, numeric: true },
    { key: "keywords", label: t("overview.organicKeywords"), render: (row) => formatCompact(row.organic_keywords, locale), sort: (row) => row.organic_keywords, numeric: true },
    { key: "paid", label: t("overview.colPaidTraffic"), render: (row) => formatCompact(row.paid_traffic, locale), sort: (row) => row.paid_traffic, numeric: true },
    { key: "rank", label: "OA", render: (row) => <Authority rank={row.rank} />, sort: (row) => row.rank, numeric: true },
  ];
  return (
    <div className="page-stack">
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("gap.shareTitle")}</h4>
          <EChart option={option} height={260} />
        </section>
        <section className="sub-panel">
          <h4>{t("gap.shareHint")}</h4>
          <p className="faint small">{t("gap.shareText")}</p>
        </section>
      </div>
      <div className="table-toolbar">
        <Button
          small
          disabled={rows.length === 0}
          onClick={() =>
            download(
              "bulk-compare.csv",
              toCsv(
                [t("overview.colDomain"), t("overview.organicTraffic"), t("gap.colShare"), t("overview.organicKeywords"), t("overview.colPaidTraffic"), "OA"],
                rows.map((row) => [row.target, row.organic_traffic, (shares.get(row.target) ?? 0).toFixed(1), row.organic_keywords, row.paid_traffic, row.rank]),
              ),
            )
          }
        >
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable rows={rows} columns={columns} rowKey={(row) => row.target} initialSort={{ key: "traffic", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}

export function Gap({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go } = useShell();
  const [tab, setTab] = useState<Tab>("missing");
  const [chosen, setChosen] = useState<string[]>(project.competitors.slice(0, MAX_COMPETITORS));
  const [limit, setLimit] = useState(100);
  const [input, setInput] = useState("");
  const [targets, setTargets] = useState<string[]>([]);
  const labels: Record<Tab, string> = { missing: t("gap.tabMissing"), shared: t("gap.tabShared"), links: t("gap.tabLinks"), bulk: t("gap.tabBulk") };
  const toggle = (competitor: string) =>
    setChosen((previous) =>
      previous.includes(competitor) ? previous.filter((entry) => entry !== competitor) : previous.length >= MAX_COMPETITORS ? previous : [...previous, competitor],
    );
  const submitTargets = (event: FormEvent) => {
    event.preventDefault();
    setTargets(
      input
        .split(/[\n,;\s]+/)
        .map((line) => line.trim())
        .filter(Boolean),
    );
  };
  const needsCompetitors = tab !== "bulk" && chosen.length === 0;
  return (
    <div className="page-stack">
      <div className="oc-tabs" role="tablist" aria-label={t("common.navGap")}>
        {TABS.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => setTab(entry)}>
            {labels[entry]}
          </button>
        ))}
      </div>
      {tab !== "bulk" && (
        <div className="oc-card competitor-picker">
          <span className="faint small">{t("gap.compareWith", { own: project.domain })}</span>
          <div className="feature-chips">
            {project.competitors.map((competitor) => (
              <button key={competitor} type="button" className={`oc-chip chip-button ${chosen.includes(competitor) ? "accent" : ""}`} aria-pressed={chosen.includes(competitor)} onClick={() => toggle(competitor)}>
                {competitor}
              </button>
            ))}
            {project.competitors.length === 0 && (
              <Button small onClick={() => go({ page: "overview", project: project.id })}>
                {t("gap.findCompetitors")}
              </Button>
            )}
          </div>
          <span className="faint small">{t("gap.maxCompetitors", { n: MAX_COMPETITORS })}</span>
        </div>
      )}
      <div role="tabpanel">
        {needsCompetitors && (
          <div className="oc-card">
            <Empty icon={<GitCompareArrows size={28} />} title={t("gap.noCompetitorsTitle")}>
              <p>{t("gap.noCompetitorsText")}</p>
            </Empty>
          </div>
        )}
        {!needsCompetitors && (tab === "missing" || tab === "shared") && (
          <PaidFrame<KeywordGap>
            module="gap"
            ask={{ project_id: project.id, report: tab, competitors: chosen }}
            title={tab === "missing" ? t("gap.missingTitle") : t("gap.sharedTitle")}
            hint={tab === "missing" ? t("gap.missingHint") : t("gap.sharedHint")}
            limit={limit}
            onLimit={setLimit}
          >
            {(gap) => <KeywordGapView gap={gap} projectId={project.id} shared={tab === "shared"} />}
          </PaidFrame>
        )}
        {!needsCompetitors && tab === "links" && (
          <PaidFrame<LinkGapRow[]> module="gap" ask={{ project_id: project.id, report: "links", competitors: chosen }} title={t("gap.linksTitle")} hint={t("gap.linksHint")} limit={limit} onLimit={setLimit}>
            {(rows) => <LinkGapView rows={rows} competitors={chosen} />}
          </PaidFrame>
        )}
        {tab === "bulk" && (
          <div className="page-stack">
            <form className="oc-card list-bar" onSubmit={submitTargets}>
              <textarea className="oc-input bulk-input" rows={3} value={input} onChange={(event) => setInput(event.target.value)} placeholder={t("gap.bulkPlaceholder")} spellCheck={false} />
              <Button type="submit">{t("backlinks.bulkCompare")}</Button>
              <span className="faint small">{t("gap.bulkHint")}</span>
            </form>
            <PaidFrame<BulkRow[]> module="gap" ask={{ project_id: project.id, report: "bulk", targets }} title={t("gap.bulkTitle")} hint={targets.length ? t("backlinks.bulkCount", { n: targets.length }) : t("backlinks.bulkDefault")}>
              {(rows) => <BulkView rows={rows} />}
            </PaidFrame>
          </div>
        )}
      </div>
    </div>
  );
}
