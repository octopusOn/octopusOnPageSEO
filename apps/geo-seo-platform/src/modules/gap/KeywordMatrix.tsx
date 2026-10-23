// Every keyword of the site and its competitors side by side in Semrush's groups: shared,
// missing, weak, strong, untapped and unique, with filters, tracking and lists.

import { useMemo, useState } from "react";
import { Download, Search } from "lucide-react";
import { Button, DataTable, Field, type Column } from "@octopusseo/ui";
import type { GapGroup, GapMatrix, MatrixKeyword, Ranking } from "../../api/types";
import { useI18n } from "../../i18n";
import { Entity } from "../../lib/Entity";
import { formatNumber, formatUsd } from "../../lib/format";
import { useGlossary } from "../../lib/glossary";
import { download, toCsv } from "../../lib/report";
import { PAGE_SIZES, tableLabels } from "../../lib/tables";
import { AddToList, Difficulty, INTENTS } from "../keywords/KeywordTable";
import { TrackButton } from "../rank/TrackButton";
import { asKeyword, filterMatrix, GAP_GROUPS, NO_MATRIX_FILTER, type MatrixFilters } from "./groups";

function Position({ ranking }: { ranking: Ranking | null | undefined }) {
  if (!ranking || ranking.position === null) return <span className="faint">—</span>;
  return (
    <span className="position" title={ranking.url ?? ""}>
      {ranking.position}
    </span>
  );
}

/** Two sites' keywords as two circles: the site's own, both, and the competitor's own. */
function TwoSites({ gap }: { gap: GapMatrix }) {
  const { t, locale } = useI18n();
  const [competitor] = gap.competitors;
  if (!competitor || gap.competitors.length !== 1) return null;
  const both = gap.counts.shared ?? 0;
  const mine = gap.counts.unique ?? 0;
  const theirs = gap.counts.missing ?? 0;
  return (
    <svg className="venn" viewBox="0 0 320 170" role="img" aria-label={t("gap.vennLabel", { own: gap.own, competitor, both: formatNumber(both, locale) })}>
      <circle cx="120" cy="85" r="74" className="venn-own" />
      <circle cx="200" cy="85" r="74" className="venn-theirs" />
      <text x="82" y="90" textAnchor="middle" className="venn-count">{formatNumber(mine, locale)}</text>
      <text x="160" y="90" textAnchor="middle" className="venn-count">{formatNumber(both, locale)}</text>
      <text x="238" y="90" textAnchor="middle" className="venn-count">{formatNumber(theirs, locale)}</text>
      <text x="82" y="164" textAnchor="middle" className="venn-label">{gap.own}</text>
      <text x="238" y="164" textAnchor="middle" className="venn-label">{competitor}</text>
    </svg>
  );
}

export function KeywordMatrix({ gap, projectId }: { gap: GapMatrix; projectId: number }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const [group, setGroup] = useState<GapGroup | "all">("missing");
  const [filters, setFilters] = useState<MatrixFilters>(NO_MATRIX_FILTER);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const rows = useMemo(() => filterMatrix(gap.keywords, group, filters), [gap.keywords, group, filters]);
  const domains = [gap.own, ...gap.competitors];
  const toggle = (keyword: string) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(keyword)) next.delete(keyword);
      else next.add(keyword);
      return next;
    });
  const allShown = rows.length > 0 && rows.every((row) => selected.has(row.keyword));
  const columns: Column<MatrixKeyword>[] = [
    { key: "select", label: "", width: "36px", render: (row) => <input type="checkbox" checked={selected.has(row.keyword)} onChange={() => toggle(row.keyword)} aria-label={t("keywords.select", { keyword: row.keyword })} /> },
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
    { key: "volume", label: t("overview.colVolume"), help: gloss("volume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true, total: (shown) => formatNumber(shown.reduce((sum, row) => sum + (row.volume ?? 0), 0), locale) },
    { key: "difficulty", label: t("overview.colKd"), help: gloss("kd"), render: (row) => <Difficulty value={row.difficulty} />, sort: (row) => row.difficulty, numeric: true },
    { key: "cpc", label: t("overview.colCpc"), help: gloss("cpc"), optional: true, render: (row) => (row.cpc === null ? "—" : formatUsd(row.cpc, locale)), sort: (row) => row.cpc, numeric: true },
    { key: "intent", label: t("overview.colIntent"), help: gloss("intent"), optional: true, defaultHidden: true, render: (row) => (row.intent ? t(`overview.intent_${row.intent}`) : ""), sort: (row) => row.intent },
    ...domains.map(
      (domain, index) =>
        ({
          key: `d-${index}`,
          label: domain,
          render: (row: MatrixKeyword) => <Position ranking={row.rankings[index]} />,
          sort: (row: MatrixKeyword) => row.rankings[index]?.position ?? null,
          numeric: true,
        }) satisfies Column<MatrixKeyword>,
    ),
    {
      key: "groups",
      label: t("gap.colGroups"),
      optional: true,
      render: (row) => (
        <span className="flag-chips">
          {row.groups.map((entry) => (
            <span key={entry} className={`oc-chip small group-${entry}`}>
              {t(`gap.group_${entry}`)}
            </span>
          ))}
        </span>
      ),
    },
  ];
  const chosen = gap.keywords.filter((row) => selected.has(row.keyword)).map(asKeyword);
  return (
    <div className="page-stack">
      <div className="gap-overview">
        <div className="gap-groups" role="radiogroup" aria-label={t("gap.groupsLabel")}>
          {(["all", ...GAP_GROUPS] as const).map((entry) => (
            <button
              key={entry}
              type="button"
              role="radio"
              aria-checked={group === entry}
              className={`oc-stat stat-button gap-group${group === entry ? " on" : ""}`}
              onClick={() => setGroup(entry)}
              title={entry === "all" ? undefined : t(`gap.groupHint_${entry}`)}
            >
              <span className="oc-stat-label">{t(`gap.group_${entry}`)}</span>
              <strong className="oc-stat-value">{formatNumber(entry === "all" ? gap.keywords.length : (gap.counts[entry] ?? 0), locale)}</strong>
            </button>
          ))}
        </div>
        <TwoSites gap={gap} />
      </div>
      <p className="faint small">
        {group === "all" ? t("gap.matrixHint", { n: formatNumber(gap.rows_per_domain, locale) }) : t(`gap.groupHint_${group}`)}{" "}
        {domains.map((domain, index) => `${domain}: ${formatNumber(gap.keywords_per_domain[index] ?? 0, locale)}`).join(" · ")}
      </p>
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
          <input className="oc-input narrow" inputMode="numeric" value={filters.maxDifficulty >= 100 ? "" : filters.maxDifficulty} onChange={(event) => setFilters({ ...filters, maxDifficulty: Math.min(100, Number(event.target.value.replace(/\D/g, "")) || 100) })} />
        </Field>
        <Field label={t("gap.minCpc")}>
          <input className="oc-input narrow" inputMode="decimal" value={filters.minCpc || ""} onChange={(event) => setFilters({ ...filters, minCpc: Number(event.target.value.replace(",", ".")) || 0 })} />
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
                for (const row of rows) {
                  if (allShown) next.delete(row.keyword);
                  else next.add(row.keyword);
                }
                return next;
              })
            }
          />
          <span>{t("keywords.selectShown", { n: formatNumber(rows.length, locale) })}</span>
        </label>
        <AddToList projectId={projectId} keywords={chosen} onDone={() => setSelected(new Set())} />
        <TrackButton projectId={projectId} keywords={chosen} onDone={() => setSelected(new Set())} />
        <Button
          small
          disabled={rows.length === 0}
          onClick={() =>
            download(
              `${gap.own}-${group}.csv`,
              toCsv(
                [t("overview.colKeyword"), t("overview.colVolume"), t("overview.colKd"), t("overview.colCpc"), t("overview.colIntent"), ...domains, t("gap.colGroups")],
                rows.map((row) => [row.keyword, row.volume, row.difficulty, row.cpc, row.intent, ...row.rankings.map((ranking) => ranking?.position ?? null), row.groups.join(" ")]),
              ),
            )
          }
        >
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable rows={rows} columns={columns} rowKey={(row) => row.keyword} initialSort={{ key: "volume", direction: "desc" }} tableId="gap-matrix" pageSizes={PAGE_SIZES} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
    </div>
  );
}
