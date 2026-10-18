import { useEffect, useState } from "react";
import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Search } from "lucide-react";
import { api, errorText } from "../api/client";
import type { RemediationDetail, RemediationRequest, TablePage } from "../api/types";
import type { Issue } from "../catalog";
import { number, useI18n } from "../i18n";
import { solutionGuide } from "../lib/seoRemediation";
import { columnLabel } from "./DataTable";
import { Button, Empty, ErrorState, Loading, Modal } from "./ui";
import { SavedSchemaResult } from "./SavedSchemaResult";

export function SeoRemediation({id,rule,onClose}:{id:string;rule:Issue;onClose:()=>void}) {
  const {t,lang}=useI18n();
  const [search,setSearch]=useState("");
  const [request,setRequest]=useState<RemediationRequest>({issue_id:rule.id,search:"",offset:0,limit:20,url:"",evidence_offset:0});
  useEffect(()=>{const timer=setTimeout(()=>setRequest(current=>({...current,search,offset:0,url:"",evidence_offset:0})),250);return()=>clearTimeout(timer);},[search]);
  const query=useQuery({queryKey:["score-remediation",id,request],queryFn:()=>api.remediation(id,request),placeholderData:keepPreviousData});
  return <Modal title={`${t("seoRemediationTitle")} · ${rule[lang].title}`} className="seo-remediation-modal" onClose={onClose}>
    <p className="seo-remediation-note">{t("seoRemediationScope")}</p>
    <div className="search-input seo-remediation-search"><Search size={15}/><input aria-label={t("seoRemediationSearch")} placeholder={t("seoRemediationSearch")} value={search} onChange={event=>setSearch(event.target.value)}/></div>
    {query.isPending?<Loading/>:query.error?<ErrorState message={errorText(query.error)} retry={()=>void query.refetch()}/>:query.data&&<div className="seo-remediation-layout">
      <aside className="seo-remediation-urls" aria-label={t("seoRemediationUrls")}>
        <strong>{number(query.data.urls.total,lang)} {t("seoScoreAffectedUrls")}</strong>
        <div className="seo-remediation-url-list">{query.data.urls.rows.map(row=>{const url=String(row[0]);return <button key={url} aria-pressed={url===query.data!.selected_url} aria-disabled={query.isPlaceholderData} onClick={()=>{if(!query.isPlaceholderData)setRequest(current=>({...current,url,evidence_offset:0}));}}>{url}</button>;})}</div>
        <div className="seo-remediation-pagination"><Button small className="icon" aria-label={t("previous")} disabled={query.isPlaceholderData||!request.offset} onClick={()=>setRequest(current=>({...current,offset:Math.max(0,current.offset-20),url:"",evidence_offset:0}))}><ChevronLeft size={15}/></Button><span>{query.isPlaceholderData?t("loading"):query.data.urls.total?`${number(request.offset+1,lang)}–${number(request.offset+query.data.urls.rows.length,lang)}`:"0"} / {number(query.data.urls.total,lang)}</span><Button small className="icon" aria-label={t("next")} disabled={query.isPlaceholderData||request.offset+20>=query.data.urls.total} onClick={()=>setRequest(current=>({...current,offset:current.offset+20,url:"",evidence_offset:0}))}><ChevronRight size={15}/></Button></div>
      </aside>
      <div className="seo-remediation-main" aria-busy={query.isPlaceholderData}>{query.isPlaceholderData?<Loading/>:query.data.selected_url?<>
        <SeoRemediationContent rule={rule} detail={query.data}/>
        <section className="seo-remediation-evidence"><h3>{t("seoRemediationEvidence")}</h3><p>{t("seoRemediationEvidenceNote")}</p><Witnesses table={query.data.evidence}/>
          {query.data.evidence.total>50&&<div className="seo-remediation-pagination"><Button small disabled={!request.evidence_offset} onClick={()=>setRequest(current=>({...current,url:query.data!.selected_url!,evidence_offset:Math.max(0,current.evidence_offset-50)}))}>{t("previous")}</Button><span>{number(request.evidence_offset+1,lang)}–{number(request.evidence_offset+query.data.evidence.rows.length,lang)} / {number(query.data.evidence.total,lang)}</span><Button small disabled={request.evidence_offset+50>=query.data.evidence.total} onClick={()=>setRequest(current=>({...current,url:query.data!.selected_url!,evidence_offset:current.evidence_offset+50}))}>{t("next")}</Button></div>}
        </section>
      </>:<Empty>{t("noResults")}</Empty>}</div>
    </div>}
  </Modal>;
}

export function SeoRemediationContent({rule,detail}:{rule:Issue;detail:RemediationDetail}) {
  const {t,lang}=useI18n();
  const guide=solutionGuide(rule,detail,lang);
  if(detail.schema_report)return <>
    <div className="seo-remediation-selected"><span>{t("seoRemediationSelected")}</span><strong>{detail.selected_url}</strong></div>
    <SavedSchemaResult url={detail.selected_url??""} facts={detail.facts} report={detail.schema_report}/>
    <section><h3>{t("seoRemediationSteps")}</h3><p>{rule[lang].fix}</p></section>
    <section><h3>{t("seoRemediationValidation")}</h3><p>{lang==="tr"?"Düzelttikten sonra Schema alan kontrolünü işaretleyerek yeniden tarayın; aynı URL’de zorunlu alan hatalarının ve puan kesintisinin giderildiğini doğrulayın.":"After correction, recrawl with Schema property checks enabled and verify that required-property errors and their score deduction disappear on this URL."}</p></section>
  </>;
  return <>
    <div className="seo-remediation-selected"><span>{t("seoRemediationSelected")}</span><strong>{detail.selected_url}</strong></div>
    <section><h3>{t("seoRemediationCurrent")}</h3><p className="seo-remediation-diagnosis">{rule[lang].description}</p><dl className="seo-remediation-facts">{guide.observations.map((observation,index)=><div key={index}><dt>{observation.label}</dt><dd>{observation.value}</dd></div>)}</dl></section>
    {guide.warning&&<p className="seo-remediation-warning">{guide.warning}</p>}
    <section><h3>{t("seoRemediationSteps")}</h3><ol className="seo-remediation-steps">{guide.steps.map((step,index)=><li key={index}>{step}</li>)}</ol></section>
    <section><h3>{t("seoRemediationExamples")}</h3><p className="seo-remediation-note">{t("seoRemediationExampleNote")}</p><div className="seo-remediation-examples">
      {guide.before&&<div><h4>{t("seoRemediationBefore")}</h4><pre><code>{guide.before}</code></pre></div>}
      {guide.after?<div><h4>{t("seoRemediationAfter")}</h4><pre><code>{guide.after}</code></pre></div>:<p>{t("seoRemediationNoExample")}</p>}
    </div></section>
    <section><h3>{t("seoRemediationValidation")}</h3><ul className="seo-remediation-steps">{guide.validation.map((step,index)=><li key={index}>{step}</li>)}</ul></section>
    {guide.sources.length>0&&<p className="seo-remediation-sources">{t("seoRemediationSources")}: {guide.sources.map(source=><a href={source.url} target="_blank" rel="noreferrer" key={source.url}>{source.title}</a>)}</p>}
  </>;
}

function Witnesses({table}:{table:TablePage}) {
  const {t,lang}=useI18n();
  const labels:Record<string,string>={json_syntax_valid:lang==="tr"?"JSON sözdizimi geçerli":"Valid JSON syntax",return_link:t("seoEvidenceReturn"),self_reference:t("seoEvidenceSelf"),target_indexability:t("indexability"),target_lang:t("seoEvidenceTargetLang"),previous_level:t("seoEvidencePreviousLevel"),previous_text:t("seoEvidencePreviousText"),matched_value:t("seoEvidenceMatched"),position:t("seoEvidencePosition")};
  if(!table.rows.length)return <p className="seo-remediation-note">{t("seoRemediationNoEvidence")}</p>;
  return <><p>{number(table.total,lang)} {t("seoRemediationRecords")}</p><div className="seo-witness-scroll"><table className="seo-witness-table"><thead><tr>{table.columns.map(column=><th key={column.name}>{labels[column.name]??columnLabel(column.name,t)}</th>)}</tr></thead><tbody>{table.rows.map((row,index)=><tr key={index}>{row.map((value,column)=><td key={column}>{value===null?t("seoRemediationMissing"):typeof value==="boolean"?(["destination_internal","source_internal"].includes(table.columns[column]!.name)?t(value?"linkInternal":"linkExternal"):value?t("yes"):t("no")):String(value)}</td>)}</tr>)}</tbody></table></div></>;
}
