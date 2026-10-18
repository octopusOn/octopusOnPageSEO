import { useState } from "react";
import { ArrowUpRight, ChevronRight, CircleAlert } from "lucide-react";
import type { TablePage } from "../api/types";
import { categories, issueById, priorityOrder } from "../catalog";
import { useI18n, number } from "../i18n";
import { Button, Empty } from "./ui";

export function UrlFindings({ findings, recorded, excluded, activeIssueId, onIssue }: {
  findings: TablePage | undefined;
  recorded: boolean;
  excluded: string[];
  activeIssueId: string;
  onIssue: (id: string) => void;
}) {
  const { t, lang } = useI18n();
  const [showExcluded, setShowExcluded] = useState(() => excluded.includes(activeIssueId));
  const column = findings?.columns.findIndex((column) => column.name === "issue_id") ?? -1;
  if (!findings || column < 0) return <Empty>{t("urlFindingsUnavailable")}</Empty>;
  const items = [...new Set(findings.rows.flatMap((row) => typeof row[column] === "string" && row[column] ? [row[column]] : []))]
    .map((id) => ({ id, issue: issueById.get(id), excluded: excluded.includes(id) }))
    .sort((a, b) => (priorityOrder[a.issue?.priority ?? "unknown"] ?? 3) - (priorityOrder[b.issue?.priority ?? "unknown"] ?? 3) || a.id.localeCompare(b.id));
  const visible = items.filter((item) => showExcluded || !item.excluded);
  return <div className="url-findings">
    <div className="row between url-findings-toolbar">
      <strong>{number(findings.total, lang)} {t("urlFindingsRecorded")}</strong>
      {items.some((item) => item.excluded) && <label className="oc-check"><input type="checkbox" checked={showExcluded} onChange={(e) => setShowExcluded(e.target.checked)} />{t("showExcluded")}</label>}
    </div>
    <p className="content-match-note">{t("urlFindingsNote")}</p>
    {findings.total > findings.rows.length && <p className="render-comparison-warning">{t("urlFindingsPreview")} {number(findings.rows.length, lang)} / {number(findings.total, lang)}</p>}
    {!visible.length ? <Empty>{t(findings.total ? "urlFindingsAllExcluded" : recorded ? "urlFindingsNone" : "urlFindingsNotRecorded")}</Empty> : <div className="url-findings-list">
      {visible.map(({id, issue, excluded}, index) => <details key={id} open={id === activeIssueId || (!activeIssueId && index === 0)} className={excluded ? "finding-excluded" : ""}>
        <summary>
          <span className={`priority-mark ${issue?.priority ?? "low"}`}><CircleAlert size={15}/></span>
          <span className="url-finding-title"><strong>{issue?.[lang].title ?? id}</strong><small>{issue ? categories.find((category) => category.id === issue.category)?.[lang] : t("urlFindingsUnknown")}</small></span>
          {excluded && <span className="oc-badge">{t("excluded")}</span>}
          {issue && <span className={`oc-badge ${issue.priority === "high" ? "critical" : issue.priority === "medium" ? "warning" : "info"}`}>{t(issue.priority as "high" | "medium" | "low")}</span>}
          <ChevronRight size={14} className="finding-chevron"/>
        </summary>
        <div className="url-finding-body">
          {issue ? <>
            <h3>{t("why")}</h3><p>{issue[lang].description}</p>
            <h3>{t("fix")}</h3><p>{issue[lang].fix}</p>
            <Button small onClick={() => onIssue(id)}>{t("urlFindingsAffected")}<ArrowUpRight size={14}/></Button>
          </> : <p>{t("urlFindingsUnknown")}</p>}
        </div>
      </details>)}
    </div>}
  </div>;
}
