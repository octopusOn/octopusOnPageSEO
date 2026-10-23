// The toxic-link audit: every linking site judged by plain rules, the user's decision on each
// (keep, watch, disavow) and the disavow file Google takes.

import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, ShieldAlert } from "lucide-react";
import { Button, DataTable, Segmented, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { LinkDecision, LinkDecisionKind, LinkRisk, LinkRiskLevel } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { formatNumber } from "../../lib/format";
import { useGlossary } from "../../lib/glossary";
import { PaidFrame } from "../../lib/PaidFrame";
import { download } from "../../lib/report";
import { tableLabels } from "../../lib/tables";
import { Authority } from "./Backlinks";

const RISKS = ["all", "high", "medium", "low"] as const;
type RiskFilter = (typeof RISKS)[number];

const DECISIONS = ["all", "none", "keep", "watch", "disavow"] as const;
type DecisionFilter = (typeof DECISIONS)[number];

const FLAGS = ["spam", "spam_some", "network", "tld", "sitewide", "link_farm", "spam_anchor"] as const;

function AuditView({ projectId, risks }: { projectId: number; risks: LinkRisk[] }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { fail, notify } = useShell();
  const queryClient = useQueryClient();
  const [risk, setRisk] = useState<RiskFilter>("high");
  const [decided, setDecided] = useState<DecisionFilter>("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const decisions = useQuery({ queryKey: ["link-decisions", projectId], queryFn: () => api.linkDecisions(projectId) });
  const byDomain = useMemo(() => new Map((decisions.data ?? []).map((decision) => [decision.domain, decision])), [decisions.data]);
  const decide = useMutation({
    mutationFn: ({ domains, decision }: { domains: string[]; decision: LinkDecisionKind | null }) => api.decideLinks(projectId, domains, decision),
    onSuccess: (next) => {
      queryClient.setQueryData<LinkDecision[]>(["link-decisions", projectId], next);
      setSelected(new Set());
    },
    onError: (thrown) => fail(thrown),
  });
  const disavow = useMutation({
    mutationFn: () => api.disavowFile(projectId),
    onSuccess: (file) => {
      download(file.file_name, file.text, "text/plain;charset=utf-8");
      notify(t("backlinks.disavowSaved", { n: file.domains }));
    },
    onError: (thrown) => fail(thrown),
  });
  const decisionOf = (row: LinkRisk) => byDomain.get(row.domain.replace(/^www\./, ""))?.decision ?? null;
  const counts = useMemo(() => {
    const result: Record<LinkRiskLevel, number> = { high: 0, medium: 0, low: 0 };
    for (const row of risks) result[row.risk] += 1;
    return result;
  }, [risks]);
  const shown = risks.filter((row) => (risk === "all" || row.risk === risk) && (decided === "all" || (decided === "none" ? decisionOf(row) === null : decisionOf(row) === decided)));
  const disavowed = (decisions.data ?? []).filter((decision) => decision.decision === "disavow").length;
  const toggle = (domain: string) =>
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(domain)) next.delete(domain);
      else next.add(domain);
      return next;
    });
  const allShown = shown.length > 0 && shown.every((row) => selected.has(row.domain));
  const columns: Column<LinkRisk>[] = [
    {
      key: "select",
      label: "",
      width: "36px",
      render: (row) => <input type="checkbox" checked={selected.has(row.domain)} onChange={() => toggle(row.domain)} aria-label={t("backlinks.selectSite", { domain: row.domain })} />,
    },
    {
      key: "domain",
      label: t("overview.colDomain"),
      render: (row) => (
        <span className="link-cell">
          <Entity kind="domain" value={row.domain}>
            <strong className="cell-main">{row.domain}</strong>
          </Entity>
          <Entity kind="url" value={row.url_from}>
            <span className="cell-url">{row.url_from.replace(/^https?:\/\/(www\.)?/, "")}</span>
          </Entity>
        </span>
      ),
      sort: (row) => row.domain,
    },
    { key: "risk", label: t("backlinks.colRisk"), render: (row) => <span className={`risk-badge risk-${row.risk}`}>{t(`backlinks.risk_${row.risk}`)} · {row.score}</span>, sort: (row) => row.score, numeric: true },
    {
      key: "flags",
      label: t("backlinks.colFlags"),
      render: (row) => (
        <span className="flag-chips">
          {row.flags.map((flag) => (
            <span key={flag} className="oc-chip small" title={t(`backlinks.flagHint_${flag}`)}>
              {t(`backlinks.flag_${flag}`)}
            </span>
          ))}
        </span>
      ),
      sort: (row) => row.flags.length,
    },
    { key: "spam", label: t("backlinks.colSpam"), help: gloss("spam_score"), render: (row) => row.spam_score ?? "—", sort: (row) => row.spam_score, numeric: true },
    { key: "rank", label: "OA", help: gloss("oa"), optional: true, render: (row) => <Authority rank={row.domain_rank} />, sort: (row) => row.domain_rank, numeric: true },
    { key: "anchor", label: t("backlinks.colAnchor"), optional: true, render: (row) => <span className="small">{row.anchor ?? "—"}{row.dofollow ? "" : " · nofollow"}</span>, sort: (row) => row.anchor },
    { key: "network", label: t("backlinks.colNetwork"), optional: true, defaultHidden: true, render: (row) => row.network ?? "—", sort: (row) => row.network },
    {
      key: "decision",
      label: t("backlinks.colDecision"),
      render: (row) => (
        <select
          className={`oc-select decision-select decision-${decisionOf(row) ?? "none"}`}
          value={decisionOf(row) ?? ""}
          aria-label={t("backlinks.decisionFor", { domain: row.domain })}
          onChange={(event) => decide.mutate({ domains: [row.domain], decision: (event.target.value || null) as LinkDecisionKind | null })}
        >
          <option value="">{t("backlinks.decisionNone")}</option>
          <option value="keep">{t("backlinks.decision_keep")}</option>
          <option value="watch">{t("backlinks.decision_watch")}</option>
          <option value="disavow">{t("backlinks.decision_disavow")}</option>
        </select>
      ),
      sort: (row) => decisionOf(row) ?? "",
    },
  ];
  return (
    <div className="page-stack">
      <div className="oc-stats">
        {(["high", "medium", "low"] as const).map((level) => (
          <button key={level} type="button" className={`oc-stat stat-button risk-stat risk-${level}${risk === level ? " on" : ""}`} onClick={() => setRisk(level)}>
            <span className="oc-stat-label">{t(`backlinks.riskSites_${level}`)}</span>
            <strong className="oc-stat-value">{formatNumber(counts[level], locale)}</strong>
          </button>
        ))}
        <div className="oc-stat">
          <span className="oc-stat-label">{t("backlinks.disavowCount")}</span>
          <strong className="oc-stat-value">{formatNumber(disavowed, locale)}</strong>
          <Button small disabled={disavowed === 0} busy={disavow.isPending} onClick={() => disavow.mutate()}>
            <Download size={14} />
            {t("backlinks.disavowDownload")}
          </Button>
        </div>
      </div>
      <div className="filters">
        <Segmented<RiskFilter> label={t("backlinks.colRisk")} value={risk} onChange={setRisk} options={RISKS.map((value) => ({ value, label: t(`backlinks.riskFilter_${value}`) }))} />
        <Segmented<DecisionFilter> label={t("backlinks.colDecision")} value={decided} onChange={setDecided} options={DECISIONS.map((value) => ({ value, label: t(`backlinks.decisionFilter_${value}`) }))} />
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
                  if (allShown) next.delete(row.domain);
                  else next.add(row.domain);
                }
                return next;
              })
            }
          />
          <span>{t("backlinks.selectShown", { n: formatNumber(shown.length, locale) })}</span>
        </label>
        {(["keep", "watch", "disavow"] as const).map((decision) => (
          <Button key={decision} small disabled={selected.size === 0} busy={decide.isPending} onClick={() => decide.mutate({ domains: [...selected], decision })}>
            {t(`backlinks.decideSelected_${decision}`, { n: selected.size })}
          </Button>
        ))}
      </div>
      <DataTable rows={shown} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "risk", direction: "desc" }} tableId="link-audit" empty={t("backlinks.auditEmpty")} labels={tableLabels(t, locale)} />
      <details className="flag-legend">
        <summary>{t("backlinks.flagsLegend")}</summary>
        <dl>
          {FLAGS.map((flag) => (
            <div key={flag}>
              <dt>{t(`backlinks.flag_${flag}`)}</dt>
              <dd>{t(`backlinks.flagHint_${flag}`)}</dd>
            </div>
          ))}
        </dl>
      </details>
    </div>
  );
}

/** Every linking site of the target judged, the riskiest first (the first thousand). */
export function LinkAuditTab({ projectId, target }: { projectId: number; target: string | undefined }) {
  const { t } = useI18n();
  return (
    <PaidFrame<LinkRisk[]> module="backlinks" ask={{ project_id: projectId, report: "audit", ...(target ? { target } : {}) }} title={t("backlinks.auditTitle")} hint={t("backlinks.auditHint")}>
      {(risks) =>
        risks.length === 0 ? (
          <p className="faint">
            <ShieldAlert size={14} aria-hidden="true" /> {t("backlinks.auditEmpty")}
          </p>
        ) : (
          <AuditView projectId={projectId} risks={risks} />
        )
      }
    </PaidFrame>
  );
}
