import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, ListPlus, Search, Sparkles } from "lucide-react";
import { Button, DataTable, Field, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { KeywordData } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatNumber, formatUsd } from "../../lib/format";
import { download, fold, toCsv } from "../../lib/report";
import { featureName, tableLabels } from "../../lib/tables";
import { TrackButton } from "../rank/TrackButton";

export const INTENTS = ["informational", "navigational", "commercial", "transactional"] as const;

export interface KeywordFilters {
  query: string;
  intent: string;
  minVolume: number;
  maxDifficulty: number;
}

export const ALL_KEYWORDS: KeywordFilters = { query: "", intent: "", minVolume: 0, maxDifficulty: 100 };

/** The keywords a filter keeps; text is folded (any case, accents, Turkish ı). */
export function filterKeywordData(rows: readonly KeywordData[], filters: KeywordFilters): KeywordData[] {
  const query = fold(filters.query.trim());
  return rows.filter(
    (row) =>
      (!query || fold(row.keyword).includes(query)) &&
      (!filters.intent || row.intent === filters.intent) &&
      (row.volume ?? 0) >= filters.minVolume &&
      (filters.maxDifficulty >= 100 || (row.difficulty ?? 0) <= filters.maxDifficulty),
  );
}

/** A difficulty in a coloured chip. */
export function Difficulty({ value }: { value: number | null }) {
  if (value === null) return <span className="faint">—</span>;
  return <span className={`kd kd-${Math.min(4, Math.floor(value / 25))}`}>{value}</span>;
}

/** A small line of monthly searches. */
export function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return null;
  const most = Math.max(1, ...values);
  const points = values.map((value, index) => `${(index / (values.length - 1)) * 60},${18 - (value / most) * 16}`).join(" ");
  return (
    <svg className="sparkline" viewBox="0 0 60 20" width="60" height="20" aria-hidden="true">
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

/** Picks a list (or makes one) and adds the given keywords to it. */
export function AddToList({ projectId, keywords, onDone }: { projectId: number; keywords: KeywordData[]; onDone?: () => void }) {
  const { t } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const lists = useQuery({ queryKey: ["keyword-lists", projectId], queryFn: () => api.keywordLists(projectId) });
  const [choice, setChoice] = useState("");
  const add = useMutation({
    mutationFn: async () => {
      let listId = Number(choice);
      if (!choice || choice === "new") {
        const name = window.prompt(t("keywords.newListName"), t("keywords.defaultListName"));
        if (!name) return null;
        listId = (await api.createKeywordList(projectId, name)).id;
      }
      return api.addToKeywordList(listId, keywords);
    },
    onSuccess: (held) => {
      if (held === null) return;
      void queryClient.invalidateQueries({ queryKey: ["keyword-lists", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["list-keywords"] });
      notify(t("keywords.addedToList", { n: keywords.length }));
      onDone?.();
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  return (
    <span className="add-to-list">
      <select className="oc-select" value={choice} onChange={(event) => setChoice(event.target.value)} aria-label={t("keywords.list")}>
        <option value="">{t("keywords.chooseList")}</option>
        {lists.data?.map((list) => (
          <option key={list.id} value={list.id}>
            {list.name} ({list.keywords})
          </option>
        ))}
        <option value="new">{t("keywords.newList")}</option>
      </select>
      <Button small disabled={keywords.length === 0} busy={add.isPending} onClick={() => add.mutate()}>
        <ListPlus size={14} />
        {t("keywords.addSelected", { n: keywords.length })}
      </Button>
    </span>
  );
}

/** Keywords with their numbers: filters, selection, adding to a list, CSV, analysing one. */
export function KeywordTable({
  rows,
  projectId,
  fileName,
  onAnalyze,
  extraFilter,
}: {
  rows: KeywordData[];
  projectId: number;
  fileName: string;
  onAnalyze: (keyword: string) => void;
  /** Narrows the rows further (questions only). */
  extraFilter?: (row: KeywordData) => boolean;
}) {
  const { t, locale } = useI18n();
  const [filters, setFilters] = useState<KeywordFilters>(ALL_KEYWORDS);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const shown = useMemo(() => filterKeywordData(extraFilter ? rows.filter(extraFilter) : rows, filters), [rows, filters, extraFilter]);
  const toggle = (keyword: string) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(keyword)) next.delete(keyword);
      else next.add(keyword);
      return next;
    });
  const allShown = shown.length > 0 && shown.every((row) => selected.has(row.keyword));
  const columns: Column<KeywordData>[] = [
    {
      key: "select",
      label: "",
      width: "36px",
      render: (row) => (
        <input type="checkbox" checked={selected.has(row.keyword)} onChange={() => toggle(row.keyword)} aria-label={t("keywords.select", { keyword: row.keyword })} />
      ),
    },
    {
      key: "keyword",
      label: t("overview.colKeyword"),
      render: (row) => (
        <button type="button" className="link-button cell-main" onClick={() => onAnalyze(row.keyword)} title={t("keywords.analyzeThis")}>
          {row.keyword}
        </button>
      ),
      sort: (row) => row.keyword,
    },
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    { key: "trend", label: t("keywords.colTrend"), render: (row) => <Sparkline values={row.monthly.slice(-12).map((month) => month.volume)} /> },
    { key: "difficulty", label: t("overview.colKd"), render: (row) => <Difficulty value={row.difficulty} />, sort: (row) => row.difficulty, numeric: true },
    { key: "cpc", label: t("overview.colCpc"), render: (row) => (row.cpc === null ? "—" : formatUsd(row.cpc, locale)), sort: (row) => row.cpc, numeric: true },
    { key: "intent", label: t("overview.colIntent"), render: (row) => (row.intent ? t(`overview.intent_${row.intent}`) : ""), sort: (row) => row.intent },
    {
      key: "features",
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
  const chosen = rows.filter((row) => selected.has(row.keyword));
  return (
    <div className="page-stack">
      <div className="filters">
        <label className="filter-search">
          <Search size={15} aria-hidden="true" />
          <span className="sr-only">{t("keywords.filterSearch")}</span>
          <input className="oc-input" value={filters.query} placeholder={t("keywords.filterSearch")} onChange={(event) => setFilters({ ...filters, query: event.target.value })} />
        </label>
        <Field label={t("overview.filterIntent")}>
          <select className="oc-select" value={filters.intent} onChange={(event) => setFilters({ ...filters, intent: event.target.value })}>
            <option value="">{t("overview.intentAll")}</option>
            {INTENTS.map((intent) => (
              <option key={intent} value={intent}>
                {t(`overview.intent_${intent}`)}
              </option>
            ))}
          </select>
        </Field>
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
      </div>
      <div className="table-toolbar">
        <label className="oc-check">
          <input
            type="checkbox"
            checked={allShown}
            onChange={() =>
              setSelected((previous) => {
                const next = new Set(previous);
                for (const row of shown) {
                  if (allShown) next.delete(row.keyword);
                  else next.add(row.keyword);
                }
                return next;
              })
            }
          />
          <span>{t("keywords.selectShown", { n: formatNumber(shown.length, locale) })}</span>
        </label>
        <AddToList projectId={projectId} keywords={chosen} onDone={() => setSelected(new Set())} />
        <TrackButton projectId={projectId} keywords={chosen} onDone={() => setSelected(new Set())} />
        <Button
          small
          disabled={shown.length === 0}
          onClick={() =>
            download(
              `${fileName}.csv`,
              toCsv(
                [t("overview.colKeyword"), t("overview.colVolume"), t("overview.colKd"), t("overview.colCpc"), t("overview.colIntent"), t("overview.colFeatures")],
                shown.map((row) => [row.keyword, row.volume, row.difficulty, row.cpc, row.intent, row.serp_features.join(" ")]),
              ),
            )
          }
        >
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable rows={shown} columns={columns} rowKey={(row) => row.keyword} initialSort={{ key: "volume", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}
