import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { ArrowRight, ChevronDown, ScanLine, Search, ShieldCheck } from "lucide-react";
import type { SeoScore } from "../api/types";
import { categories as catalogCategories, issues, priorityOrder } from "../catalog";
import { number, useI18n } from "../i18n";
import { categoryTone, seoSystems, systemPoints, scoreLossText } from "../lib/seoAnatomy";
import { Button } from "./ui";
import { seoActionPlan } from "../lib/seoActionPlan";
import { SeoRemediation } from "./SeoRemediation";
import { issueById } from "../catalog";
import { SeoActionPlan } from "./SeoActionPlan";
import { SchemaCrawlSummary } from "./SchemaCrawlSummary";

export function SeoAnatomy({score,onIssue,crawlId}:{score?:SeoScore;onIssue:(id:string)=>void;crawlId?:string}) {
  const {t,lang}=useI18n();
  const [system,setSystem]=useState("all"),[search,setSearch]=useState(""),[onlyLoss,setOnlyLoss]=useState(false),[open,setOpen]=useState<Set<string>>(()=>new Set());
  const [remediationId,setRemediationId]=useState("");
  const remediationRule=issueById.get(remediationId);
  const branches=score?.categories??[];
  const actions=useMemo(()=>seoActionPlan(score),[score]);
  const actionById=new Map(actions.map(action=>[action.rule.id,action]));
  const active=seoSystems.find(row=>row.id===system);
  const shown=useMemo(()=>branches.filter(branch=>(!active||active.categories.includes(branch.id))&&(!onlyLoss||branch.points>0)&&(!search.trim()||[catalogCategories.find(row=>row.id===branch.id)?.[lang]??branch.id,...issues.filter(rule=>rule.category===branch.id).map(rule=>rule[lang].title)].some(text=>text.toLocaleLowerCase(lang).includes(search.trim().toLocaleLowerCase(lang))))),[branches,active,onlyLoss,search,lang]);
  if(!score||![2,3,4,5].includes(score.version)||!branches.length)return null;
  function choose(id:string) {
    setSystem(id);setSearch("");setOnlyLoss(false);
    const categories=seoSystems.find(row=>row.id===id)?.categories;
    setOpen(new Set(categories??[]));
  }
  function toggle(id:string) {setOpen(current=>{const next=new Set(current);if(next.has(id))next.delete(id);else next.add(id);return next;});}
  const allOpen=shown.length>0&&shown.every(branch=>open.has(branch.id));
  return <><section className="oc-panel seo-anatomy" aria-label={t("seoAnatomyTitle")}>
    <header className="seo-anatomy-header"><div><span className="eyebrow"><ScanLine size={14}/>{t("seoAnatomyEyebrow")}</span><h2>{t("seoAnatomyTitle")}</h2><p>{t("seoAnatomyDescription")}</p></div><div className="seo-anatomy-total"><strong>{scoreLossText(branches.reduce((sum,row)=>sum+row.points,0),lang)}</strong><span>{t("seoAnatomyTotalLoss")}</span></div></header>
    {!!(score.excluded_findings||score.excluded_links||score.excluded_urls)&&<div className="seo-exclusion-note"><ShieldCheck size={18}/><div><strong>{t("seoScoreCloudflareExcluded")}</strong><p>{number(score.excluded_urls??0,lang)} {t("seoExcludedUrls")} · {number(score.excluded_links??0,lang)} {t("seoExcludedReferences")} · {number(score.excluded_findings??0,lang)} {t("seoExcludedFindings")}</p>{!!score.canonical_context_findings&&<p>{number(score.canonical_context_findings,lang)} {t("seoCanonicalContextExcluded")}</p>}{!!score.anchor_context_findings&&<p>{number(score.anchor_context_findings,lang)} {t("seoAnchorContextExcluded")}</p>}{!!score.advisory_findings&&<p>{number(score.advisory_findings,lang)} {t("seoAdvisoryNoPenalty")}</p>}{!!score.contact_redirect_findings&&<p>{number(score.contact_redirect_findings,lang)} {t("seoContactRedirectExcluded")}</p>}{!!score.unverified_anchor_findings&&<p>{number(score.unverified_anchor_findings,lang)} {t("seoAnchorEvidenceMissing")}</p>}<p>{t("seoAnatomyArchiveNote")}</p></div></div>}
    <SeoActionPlan actions={actions} available={score.value!==null&&score.value!==undefined} onIssue={onIssue} onDetails={crawlId?setRemediationId:undefined}/>
    <SchemaCrawlSummary score={score} onIssue={onIssue}/>
    <div className="seo-anatomy-layout">
      <aside className="seo-anatomy-systems">
        <button className={`seo-system-button ${system==="all"?"active":""}`} onClick={()=>choose("all")}><span>{t("seoAnatomyAll")}</span><b>{branches.length} {t("seoAnatomyBranches")}</b></button>
        {seoSystems.map((row,index)=>{const points=systemPoints(row,branches);return <button className={`seo-system-button ${system===row.id?"active":""}`} key={row.id} onClick={()=>choose(row.id)} aria-pressed={system===row.id}><i>{index+1}</i><span>{t(row.label)}</span><b className={points>0?"has-loss":""}>{points>0?scoreLossText(points,lang):"—"}</b></button>;})}
      </aside>
      <div className="seo-anatomy-ledger">
        <div className="seo-anatomy-toolbar"><div className="search-input"><Search size={15}/><input aria-label={t("seoAnatomySearch")} placeholder={t("seoAnatomySearch")} value={search} onChange={event=>setSearch(event.target.value)}/></div><label><input type="checkbox" checked={onlyLoss} onChange={event=>setOnlyLoss(event.target.checked)}/>{t("seoAnatomyLossOnly")}</label><Button small onClick={()=>setOpen(allOpen?new Set():new Set(shown.map(row=>row.id)))}>{t(allOpen?"seoAnatomyCollapse":"seoAnatomyExpand")}</Button></div>
        <p className="seo-anatomy-ledger-note">{t("seoAnatomyScoresNote")}</p>
        <div className="seo-branch-grid">{shown.map(branch=>{
          const deductions=new Map(branch.deductions.map(row=>[row.id,row]));
          const excluded=new Map((score.excluded_rules??[]).map(row=>[row.id,row.count]));
          const checks=issues.filter(rule=>rule.category===branch.id).sort((a,b)=>(actionById.get(a.id)?.rank??Infinity)-(actionById.get(b.id)?.rank??Infinity)||priorityOrder[a.priority]!-priorityOrder[b.priority]!);
          const title=catalogCategories.find(row=>row.id===branch.id)?.[lang]??branch.id;
          return <article className={`seo-branch score-${categoryTone(branch.value)} ${open.has(branch.id)?"expanded":""}`} key={branch.id}>
            <button className="seo-branch-heading" aria-expanded={open.has(branch.id)} aria-label={title} onClick={()=>toggle(branch.id)}><span><strong>{title}</strong><small>{number(branch.checks,lang)} {t("seoAnatomyDefinedChecks")} · {number(branch.affected_urls,lang)} / {number(branch.urls,lang)} {t("seoScoreAffectedUrls")}</small></span><b>{branch.value===null?"—":number(branch.value,lang)}<small>{branch.value===null?t("seoAnatomyNoData"):"/ 100"}</small></b><ChevronDown size={15}/></button>
            <div className="seo-branch-meter" role={branch.value===null?undefined:"meter"} aria-label={`${title} · ${t("seoAnatomyBranchScore")}`} aria-valuemin={branch.value===null?undefined:0} aria-valuemax={branch.value===null?undefined:100} aria-valuenow={branch.value??undefined}><i style={{width:`${branch.value??0}%`}}/></div>
            <div className="seo-branch-impact"><span>{t("seoAnatomyGlobalImpact")}</span><strong>{scoreLossText(branch.points,lang)} {t("seoScorePoints")}</strong></div>
            {open.has(branch.id)&&<div className="seo-check-list">{checks.map(rule=>{
              const optionalUnavailable=rule.id.startsWith("schema-")&&!score.schema_validation;
              const deduction=deductions.get(rule.id),ignored=excluded.get(rule.id)??0,action=actionById.get(rule.id);
              return <div className="seo-check" key={rule.id}>
                <button className="seo-check-row" onClick={()=>onIssue(rule.id)}><span className={`seo-check-dot ${optionalUnavailable?"unverified":deduction?rule.priority:"clear"}`}/><span><strong>{rule[lang].title}</strong><small>{deduction?`${number(deduction.count,lang)} ${t("seoScoreAffectedUrls")} · ${t(rule.priority as "high"|"medium"|"low")}`:t(optionalUnavailable||branch.value===null?"seoAnatomyNoData":"seoAnatomyNoRecordedFinding")}{ignored>0&&` · ${number(ignored,lang)} ${t("seoAnatomyIgnored")}`}</small></span><b>{deduction?scoreLossText(deduction.points,lang):"0"}</b><ArrowRight size={12}/></button>
                {!!deduction&&deduction.points>0&&<div className="seo-check-advice"><div><strong>{t("seoActionFix")}</strong>{action&&<span>{t("seoActionRank")} #{action.rank}</span>}</div><p>{rule[lang].fix}</p>{crawlId&&<Button small onClick={()=>setRemediationId(rule.id)}>{t("seoRemediationTitle")}</Button>}</div>}
              </div>;
            })}</div>}
          </article>;
        })}</div>
        {!shown.length&&<p className="muted padded">{t("noResults")}</p>}
      </div>
    </div>
  </section>{crawlId&&remediationRule&&createPortal(<SeoRemediation key={`${crawlId}:${remediationRule.id}`} id={crawlId} rule={remediationRule} onClose={()=>setRemediationId("")}/>,document.querySelector(".spider-shell")??document.body)}</>;
}
