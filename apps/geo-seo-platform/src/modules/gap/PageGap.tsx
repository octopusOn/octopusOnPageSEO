// The keywords a competitor's page ranks for and the site's page does not: what to add to the
// page that should win them.

import { useState, type FormEvent } from "react";
import { Download } from "lucide-react";
import { Button, DataTable, type Column } from "@octopusseo/ui";
import type { PageGapRow, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { Entity } from "../../lib/Entity";
import { formatNumber, formatUsd } from "../../lib/format";
import { useGlossary } from "../../lib/glossary";
import { PaidFrame } from "../../lib/PaidFrame";
import { download, toCsv } from "../../lib/report";
import { tableLabels } from "../../lib/tables";
import { AddToList, Difficulty } from "../keywords/KeywordTable";
import { asKeyword } from "./groups";

function absolute(url: string): string {
  const trimmed = url.trim();
  return /^https?:\/\//.test(trimmed) ? trimmed : `https://${trimmed}`;
}

export function PageGap({ project, market }: { project: ProjectRow; market: Record<string, unknown> }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const [own, setOwn] = useState(`https://${project.domain}/`);
  const [theirs, setTheirs] = useState("");
  const [asked, setAsked] = useState<{ own: string; theirs: string } | null>(null);
  const [limit, setLimit] = useState(100);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (own.trim() && theirs.trim()) setAsked({ own: absolute(own), theirs: absolute(theirs) });
  };
  const columns: Column<PageGapRow>[] = [
    { key: "keyword", label: t("overview.colKeyword"), render: (row) => (
        <Entity kind="keyword" value={row.keyword}>
          <strong className="cell-main">{row.keyword}</strong>
        </Entity>
      ), sort: (row) => row.keyword },
    { key: "volume", label: t("overview.colVolume"), help: gloss("volume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true },
    { key: "difficulty", label: t("overview.colKd"), help: gloss("kd"), render: (row) => <Difficulty value={row.difficulty} />, sort: (row) => row.difficulty, numeric: true },
    { key: "cpc", label: t("overview.colCpc"), help: gloss("cpc"), render: (row) => (row.cpc === null ? "—" : formatUsd(row.cpc, locale)), sort: (row) => row.cpc, numeric: true },
    { key: "position", label: t("gap.colTheirPosition"), render: (row) => row.position ?? "—", sort: (row) => row.position, numeric: true },
    { key: "traffic", label: t("overview.colTraffic"), help: gloss("traffic"), render: (row) => formatNumber(row.traffic, locale), sort: (row) => row.traffic, numeric: true },
  ];
  return (
    <div className="page-stack">
      <form className="oc-card page-gap-form" onSubmit={submit}>
        <label className="field-inline">
          <span className="faint small">{t("gap.ownPage")}</span>
          <input className="oc-input" value={own} onChange={(event) => setOwn(event.target.value)} spellCheck={false} />
        </label>
        <label className="field-inline">
          <span className="faint small">{t("gap.competitorPage")}</span>
          <input className="oc-input" value={theirs} onChange={(event) => setTheirs(event.target.value)} placeholder="https://www.rakip.com/sayfa" spellCheck={false} />
        </label>
        <Button type="submit" primary disabled={!own.trim() || !theirs.trim()}>
          {t("gap.comparePages")}
        </Button>
      </form>
      {asked && (
        <PaidFrame<PageGapRow[]> module="gap" ask={{ project_id: project.id, report: "pages", own_page: asked.own, competitor_page: asked.theirs, ...market }} title={t("gap.pageGapTitle")} hint={t("gap.pageGapHint")} limit={limit} onLimit={setLimit}>
          {(rows) => (
            <div className="page-stack">
              <div className="table-toolbar">
                <AddToList projectId={project.id} keywords={rows.map(asKeyword)} />
                <Button small disabled={rows.length === 0} onClick={() => download("page-gap.csv", toCsv([t("overview.colKeyword"), t("overview.colVolume"), t("overview.colKd"), t("overview.colCpc"), t("gap.colTheirPosition"), t("overview.colTraffic")], rows.map((row) => [row.keyword, row.volume, row.difficulty, row.cpc, row.position, Math.round(row.traffic)])))}>
                  <Download size={14} />
                  {t("overview.export")}
                </Button>
              </div>
              <DataTable rows={rows} columns={columns} rowKey={(row) => row.keyword} initialSort={{ key: "volume", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
            </div>
          )}
        </PaidFrame>
      )}
    </div>
  );
}
