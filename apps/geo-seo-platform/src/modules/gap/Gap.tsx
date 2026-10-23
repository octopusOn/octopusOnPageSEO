import { useState, type FormEvent } from "react";
import { Download, GitCompareArrows, Plus, X } from "lucide-react";
import { Button, DataTable, Empty, type Column } from "@octopusseo/ui";
import type { GapMatrix, LinkGapRow, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { domainOf, Entity } from "../../lib/Entity";
import { ProspectButton } from "../backlinks/Prospects";
import { useGlossary } from "../../lib/glossary";
import { useShell } from "../../lib/context";
import { formatNumber } from "../../lib/format";
import { MarketPicker } from "../../lib/MarketPicker";
import { PaidFrame } from "../../lib/PaidFrame";
import { download, toCsv } from "../../lib/report";
import { tableLabels } from "../../lib/tables";
import { Authority } from "../backlinks/Backlinks";
import { KeywordMatrix } from "./KeywordMatrix";
import { MarketTab } from "./Market";
import { PageGap } from "./PageGap";

const TABS = ["keywords", "pages", "links", "market"] as const;
type Tab = (typeof TABS)[number];

/** Most competitors one gap compares. */
const MAX_COMPETITORS = 5;

function LinkGapView({ projectId, rows, competitors }: { projectId: number; rows: LinkGapRow[]; competitors: string[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const columns: Column<LinkGapRow>[] = [
    { key: "domain", label: t("overview.colDomain"), render: (row) => (
        <Entity kind="domain" value={row.domain}>
          <strong className="cell-main">{row.domain}</strong>
        </Entity>
      ), sort: (row) => row.domain },
    { key: "rank", label: "OA", help: gloss("oa"), render: (row) => <Authority rank={row.rank} />, sort: (row) => row.rank, numeric: true },
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
    { key: "prospect", label: "", render: (row) => <ProspectButton projectId={projectId} domain={row.domain} source="gap" /> },
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

export function Gap({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go, params } = useShell();
  const inMarket = params.market ? { market: params.market } : {};
  // The tab lives in the address, so links and the back button reach it.
  const tab: Tab = (TABS as readonly string[]).includes(params.tab ?? "") ? (params.tab as Tab) : "keywords";
  const setTab = (next: Tab) => go({ params: { ...params, tab: next } });
  const [chosen, setChosen] = useState<string[]>(project.competitors.slice(0, MAX_COMPETITORS));
  // Domains typed in for this look, beside the project's competitors.
  const [typed, setTyped] = useState<string[]>([]);
  const [domain, setDomain] = useState("");
  const [limit, setLimit] = useState(500);
  const labels: Record<Tab, string> = { keywords: t("gap.tabKeywords"), pages: t("gap.tabPages"), links: t("gap.tabLinks"), market: t("gap.tabMarket") };
  const toggle = (competitor: string) =>
    setChosen((previous) =>
      previous.includes(competitor) ? previous.filter((entry) => entry !== competitor) : previous.length >= MAX_COMPETITORS ? previous : [...previous, competitor],
    );
  const addTyped = (event: FormEvent) => {
    event.preventDefault();
    const site = domainOf(domain.trim());
    setDomain("");
    if (!site.includes(".") || site === project.domain || typed.includes(site) || project.competitors.includes(site)) return;
    setTyped([...typed, site]);
    setChosen((previous) => (previous.length >= MAX_COMPETITORS ? previous : [...previous, site]));
  };
  const candidates = [...project.competitors, ...typed];
  const needsCompetitors = (tab === "keywords" || tab === "links") && chosen.length === 0;
  return (
    <div className="page-stack">
      <div className="tabs-with-tools">
        <div className="oc-tabs" role="tablist" aria-label={t("common.navGap")}>
          {TABS.map((entry) => (
            <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => setTab(entry)}>
              {labels[entry]}
            </button>
          ))}
        </div>
        {tab !== "links" && <MarketPicker project={project} />}
      </div>
      {tab !== "pages" && (
        <div className="oc-card competitor-picker">
          <span className="faint small">{t("gap.compareWith", { own: project.domain })}</span>
          <div className="feature-chips">
            {candidates.map((competitor) => (
              <button key={competitor} type="button" className={`oc-chip chip-button ${chosen.includes(competitor) ? "accent" : ""}`} aria-pressed={chosen.includes(competitor)} onClick={() => toggle(competitor)}>
                {competitor}
                {typed.includes(competitor) && (
                  <X
                    size={12}
                    aria-label={t("overview.compareRemove", { domain: competitor })}
                    onClick={(event) => {
                      event.stopPropagation();
                      setTyped(typed.filter((entry) => entry !== competitor));
                      setChosen(chosen.filter((entry) => entry !== competitor));
                    }}
                  />
                )}
              </button>
            ))}
            <form className="inline-add" onSubmit={addTyped}>
              <input className="oc-input" value={domain} onChange={(event) => setDomain(event.target.value)} placeholder={t("gap.addCompetitor")} aria-label={t("gap.addCompetitor")} spellCheck={false} />
              <Button type="submit" small disabled={!domain.trim()}>
                <Plus size={13} aria-hidden="true" />
              </Button>
            </form>
          </div>
          <span className="faint small">{t("gap.maxCompetitors", { n: MAX_COMPETITORS })}</span>
        </div>
      )}
      <div role="tabpanel">
        {needsCompetitors && (
          <div className="oc-card">
            <Empty icon={<GitCompareArrows size={28} />} title={t("gap.noCompetitorsTitle")}>
              <p>{t("gap.noCompetitorsText")}</p>
              {project.competitors.length === 0 && (
                <Button small onClick={() => go({ page: "overview", project: project.id, params: { tab: "competitors" } })}>
                  {t("gap.findCompetitors")}
                </Button>
              )}
            </Empty>
          </div>
        )}
        {!needsCompetitors && tab === "keywords" && (
          <PaidFrame<GapMatrix>
            module="gap"
            ask={{ project_id: project.id, report: "keywords", competitors: chosen, ...inMarket }}
            title={t("gap.matrixTitle")}
            hint={t("gap.matrixHintShort")}
            limit={limit}
            onLimit={setLimit}
          >
            {(gap) => <KeywordMatrix gap={gap} projectId={project.id} />}
          </PaidFrame>
        )}
        {tab === "pages" && <PageGap project={project} market={inMarket} />}
        {!needsCompetitors && tab === "links" && (
          <PaidFrame<LinkGapRow[]> module="gap" ask={{ project_id: project.id, report: "links", competitors: chosen }} title={t("gap.linksTitle")} hint={t("gap.linksHint")} limit={limit} onLimit={setLimit}>
            {(rows) => <LinkGapView projectId={project.id} rows={rows} competitors={chosen} />}
          </PaidFrame>
        )}
        {tab === "market" && <MarketTab project={project} competitors={chosen} market={inMarket} />}
      </div>
    </div>
  );
}
