import { useMemo, useState } from "react";
import { ArrowDownRight, ArrowUpRight, Download, Search, Sparkles } from "lucide-react";
import { Button, DataTable, Field, type Column } from "@octopusseo/ui";
import type { RankedKeyword, RankedKeywords } from "../../api/types";
import { useI18n } from "../../i18n";
import { formatNumber, formatUsd } from "../../lib/format";
import { download, featureCounts, filterKeywords, NO_FILTER, toCsv, type KeywordFilter, type PositionBand } from "../../lib/report";
import { featureName, shortUrl, tableLabels } from "../../lib/tables";
import { ReportFrame } from "./ReportFrame";

const INTENTS = ["informational", "navigational", "commercial", "transactional"] as const;

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

function KeywordsView({ data, domain }: { data: RankedKeywords; domain: string }) {
  const { t, locale } = useI18n();
  const [filter, setFilter] = useState<KeywordFilter>(NO_FILTER);
  const rows = useMemo(() => filterKeywords(data.keywords, filter), [data.keywords, filter]);
  const features = useMemo(() => featureCounts(data.keywords).slice(0, 8), [data.keywords]);
  const intentName = (intent: string | null) => (intent ? t(`overview.intent_${intent}`) : "");
  const columns: Column<RankedKeyword>[] = [
    { key: "keyword", label: t("overview.colKeyword"), render: (row) => <strong className="cell-main">{row.keyword}</strong>, sort: (row) => row.keyword },
    { key: "position", label: t("overview.colPosition"), render: (row) => <Position row={row} />, sort: (row) => row.position, numeric: true },
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    { key: "difficulty", label: t("overview.colKd"), render: (row) => (row.difficulty === null ? "—" : <span className={`kd kd-${Math.min(4, Math.floor(row.difficulty / 25))}`}>{row.difficulty}</span>), sort: (row) => row.difficulty, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), render: (row) => formatNumber(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
    { key: "cpc", label: t("overview.colCpc"), render: (row) => (row.cpc === null ? "—" : formatUsd(row.cpc, locale)), sort: (row) => row.cpc, numeric: true },
    { key: "intent", label: t("overview.colIntent"), render: (row) => intentName(row.intent), sort: (row) => row.intent },
    { key: "url", label: t("overview.colUrl"), render: (row) => <span className="cell-url" title={row.url ?? ""}>{shortUrl(row.url)}</span>, sort: (row) => row.url },
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
          <span className="sr-only">{t("overview.filterSearch")}</span>
          <input className="oc-input" value={filter.query} placeholder={t("overview.filterSearch")} onChange={(event) => setFilter({ ...filter, query: event.target.value })} />
        </label>
        <Field label={t("overview.filterPosition")}>
          <select className="oc-select" value={filter.position} onChange={(event) => setFilter({ ...filter, position: event.target.value as PositionBand })}>
            <option value="all">{t("overview.posAll")}</option>
            <option value="top3">{t("overview.posTop3")}</option>
            <option value="top10">{t("overview.posTop10")}</option>
            <option value="top20">{t("overview.posTop20")}</option>
            <option value="beyond">{t("overview.posBeyond")}</option>
          </select>
        </Field>
        <Field label={t("overview.filterIntent")}>
          <select className="oc-select" value={filter.intent} onChange={(event) => setFilter({ ...filter, intent: event.target.value })}>
            <option value="">{t("overview.intentAll")}</option>
            {INTENTS.map((intent) => (
              <option key={intent} value={intent}>
                {intentName(intent)}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t("overview.minVolume")}>
          <input className="oc-input narrow" inputMode="numeric" value={filter.minVolume || ""} onChange={(event) => setFilter({ ...filter, minVolume: Number(event.target.value.replace(/\D/g, "")) || 0 })} />
        </Field>
        <Button small onClick={exportCsv} disabled={rows.length === 0}>
          <Download size={14} />
          {t("overview.export")}
        </Button>
      </div>
      <DataTable
        rows={rows}
        columns={columns}
        rowKey={(row) => `${row.keyword}|${row.url ?? ""}`}
        initialSort={{ key: "traffic", direction: "desc" }}
        empty={t("overview.emptyRows")}
        labels={tableLabels(t, locale)}
      />
    </div>
  );
}

export function KeywordsReport({ projectId, domain, shownDomain }: { projectId: number; domain: string | undefined; shownDomain: string }) {
  const { t } = useI18n();
  const [limit, setLimit] = useState(100);
  return (
    <ReportFrame report="keywords" projectId={projectId} domain={domain} title={t("overview.keywordsTitle")} hint={t("overview.keywordsHint")} limit={limit} onLimit={setLimit}>
      {(data) => <KeywordsView data={data} domain={shownDomain} />}
    </ReportFrame>
  );
}
