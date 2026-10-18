import {useState} from "react";
import {useQuery} from "@tanstack/react-query";
import {ArrowDownToLine,FileArchive,FileCode2,ShieldCheck} from "lucide-react";
import {api,errorText,isDemo} from "../api/client";
import type {CrawlMeta,SitemapPlan} from "../api/types";
import {number,useI18n} from "../i18n";
import {Button,Empty,ErrorState,Loading} from "../components/ui";
import {canPlanSitemap,sitemapExportReady,sitemapReasonKeys} from "../lib/sitemap";
import type {JobsStatus} from "../lib/sessions";

export function SitemapBuilder({meta,active,jobsStatus,onRetryStatus,notify}:{meta:CrawlMeta;active:boolean;jobsStatus:JobsStatus;onRetryStatus:()=>void;notify:(text:string,error?:boolean)=>void}) {
  const {t}=useI18n();
  if(jobsStatus==="loading")return <Loading/>;
  if(jobsStatus==="unavailable")return <ErrorState message={t("crawlStatusRetry")} retry={onRetryStatus}/>;
  if(!canPlanSitemap(meta,active,jobsStatus))return <section className="oc-panel sitemap-empty"><Empty icon={<FileCode2 size={30}/>}><h2>{t("sitemapNeedsFinished")}</h2><p>{t("sitemapNeedsFinishedNote")}</p></Empty></section>;
  return <SitemapResults key={meta.id} id={meta.id} notify={notify}/>;
}

function SitemapResults({id,notify}:{id:string;notify:(text:string,error?:boolean)=>void}) {
  const {t,lang}=useI18n();
  const [origin,setOrigin]=useState("");
  const [exporting,setExporting]=useState(false);
  const plan=useQuery({queryKey:["sitemap-plan",id,origin],queryFn:()=>api.sitemapPlan(id,{origin})});
  const ready=sitemapExportReady(plan.data,plan.isFetching,plan.isError);
  async function save(format:"xml"|"zip") {
    if(!ready||!plan.data)return;
    const selection={origin:plan.data.origin};
    setExporting(true);
    try {const result=await api.exportSitemap(id,selection,format);if(result)notify(`${t("sitemapSaved")} · ${number(result.urls,lang)} URL · ${number(result.parts,lang)} ${t("sitemapParts")}`);}catch(error){notify(errorText(error),true);}finally{setExporting(false);}
  }
  return <div className="sitemap-page">
    <section className="oc-panel sitemap-intro"><span className="oc-tile large teal"><FileCode2 size={25}/></span><div><h2>{t("sitemapIntroTitle")}</h2><p>{t("sitemapCriteria")}</p></div><ShieldCheck size={22} className="good"/></section>
    {plan.isPending?<Loading/>:plan.error?<ErrorState message={errorText(plan.error)} retry={()=>void plan.refetch()}/>:<>
      <section className="oc-panel sitemap-origin"><label htmlFor="sitemap-origin">{t("sitemapOrigin")}</label><select id="sitemap-origin" disabled={exporting||plan.isFetching||!plan.data.origins.length} value={plan.data.origin} onChange={e=>setOrigin(e.target.value)}>{!plan.data.origins.length&&<option value="">{t("sitemapNoOrigin")}</option>}{plan.data.origins.map(item=><option key={item.origin} value={item.origin}>{item.origin} · {number(item.count,lang)} URL</option>)}</select><p>{t("sitemapOriginNote")}</p></section>
      <SitemapReport plan={plan.data}/>
      <section className="oc-panel sitemap-save"><div><h2>{t("sitemapSaveTitle")}</h2><p>{t("sitemapLimits")}</p><small>{t("sitemapDates")}</small></div><div className="sitemap-save-actions"><Button primary busy={exporting} disabled={!ready||plan.data.parts>1} onClick={()=>void save("xml")}><FileCode2 size={16}/>{t("sitemapSaveXml")}</Button><Button busy={exporting} disabled={!ready||isDemo} onClick={()=>void save("zip")}><FileArchive size={16}/>{t("sitemapSaveZip")}</Button></div>{plan.data.parts>1&&<p className="sitemap-split-note">{t("sitemapSplitRequired")}</p>}</section>
      <div className="oc-banner sitemap-publish"><ArrowDownToLine size={18}/><div><b>{t("sitemapPublishTitle")}</b><p>{t("sitemapPublishNote")}</p>{!!plan.data.origin&&<code>{plan.data.origin}/sitemap.xml</code>}</div></div>
    </>}
  </div>;
}

export function SitemapReport({plan}:{plan:SitemapPlan}) {
  const {t,lang}=useI18n();
  return <>
    <div className="sitemap-stats">{([{label:"sitemapIncluded",count:plan.included,tone:"good"},{label:"sitemapExcluded",count:plan.scanned-plan.included,tone:"warning"},{label:"sitemapDocuments",count:plan.parts,tone:""}] as const).map(item=><div className="oc-panel" key={item.label}><span>{t(item.label)}</span><strong className={item.tone}>{number(item.count,lang)}</strong></div>)}</div>
    {!!plan.warnings.length&&<div className="comparison-warnings" role="status">{plan.warnings.map(code=><p key={code}>{t(code==="partial"?"sitemapPartial":code==="list-mode"?"sitemapList":"sitemapUnknownWarning")}</p>)}</div>}
    <div className="sitemap-details-grid"><section className="oc-panel sitemap-preview"><header className="row between"><h2>{t("sitemapPreview")}</h2><small>{number(plan.sample.length,lang)} / {number(plan.included,lang)} URL</small></header><p>{t("sitemapPreviewNote")}</p>{plan.sample.length?<ol aria-label={t("sitemapPreview")}>{plan.sample.map(url=><li key={url} title={url}>{url}</li>)}</ol>:<Empty>{t("sitemapNone")}</Empty>}</section>
    <section className="oc-panel sitemap-exclusions"><h2>{t("sitemapExclusionTitle")}</h2><p>{t("sitemapExclusionNote")}</p>{plan.excluded.length?<dl>{plan.excluded.map(item=><div key={item.reason}><dt>{t(sitemapReasonKeys[item.reason]??"sitemapUnknownWarning")}</dt><dd>{number(item.count,lang)}</dd></div>)}</dl>:<Empty>{t("sitemapNoExclusions")}</Empty>}<footer>{number(plan.scanned,lang)} {t("sitemapRecordsExamined")} · {(plan.xml_bytes/1024).toLocaleString(lang==="tr"?"tr-TR":"en-US",{maximumFractionDigits:1})} KiB XML</footer></section></div>
  </>;
}
