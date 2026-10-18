import { useCallback, useMemo, useState } from "react";
import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDownToLine, ArrowLeftRight, ChevronLeft, ChevronRight, GitCompareArrows, FilterX, Search, RefreshCw } from "lucide-react";
import { api, errorText, isDemo } from "../api/client";
import type { ComparisonFilter, ComparisonRequest, CrawlMeta, Job } from "../api/types";
import { useI18n, number, type Key } from "../i18n";
import { Button, Empty, ErrorState, Loading, Modal } from "../components/ui";
import { DataTable, columnLabel } from "../components/DataTable";
import { comparisonCandidates, comparisonIdentity, comparisonSummaryRequest, comparisonWarningKeys, prepareComparisonArchives, ComparisonPreparationError } from "../lib/comparison";
import { ComparisonDetails } from "../components/ComparisonDetails";
export { ComparisonDetails } from "../components/ComparisonDetails";
import { FindingComparison } from "./FindingComparison";
import type { JobsStatus } from "../lib/sessions";

const filters:ComparisonFilter[]=["all","changed","new","missing","unchanged"];
export const comparisonFilterKeys:Record<ComparisonFilter,Key>={all:"compareAll",changed:"compareChanged",new:"compareNew",missing:"compareMissing",unchanged:"compareUnchanged"};

export function Comparison({crawls,jobs,jobsStatus,onSessions,onRetryStatus,notify}:{crawls:CrawlMeta[];jobs:Job[];jobsStatus:JobsStatus;onSessions:()=>void;onRetryStatus:()=>void;notify:(text:string,error?:boolean)=>void}) {
  const {t,lang}=useI18n();
  const queryClient=useQueryClient();
  const [preparing,setPreparing]=useState(false);
  const [baselineId,setBaseline]=useState("");
  const [currentId,setCurrent]=useState("");
  const [internalOnly,setInternal]=useState(true);
  const eligible=useMemo(()=>comparisonCandidates(crawls,jobs),[crawls,jobs]);
  const current=eligible.find(c=>c.id===currentId)??eligible[0];
  const baseline=eligible.find(c=>c.id===baselineId&&c.id!==current?.id)??eligible.find(c=>c.id!==current?.id);
  async function prepare() {
    if(preparing||!baseline||!current)return;
    setPreparing(true);
    try {
      await prepareComparisonArchives(baseline,current,api.jobs,api.overview);
      notify(t("comparePrepared"));
    } catch(error) {
      notify(error instanceof ComparisonPreparationError?t("comparePrepareUnavailable"):`${t("comparePrepareFailed")} ${errorText(error)}`,true);
    } finally {
      // Either archive may have changed even if the second update failed. Drop stale result data.
      await queryClient.invalidateQueries({refetchType:"none"});
      setPreparing(false);
    }
  }
  const label=(c:CrawlMeta)=>`${c.name} · ${new Intl.DateTimeFormat(lang==="tr"?"tr-TR":"en-US",{dateStyle:"short",timeStyle:"short"}).format(c.created_at_ms)} · ${t(c.status)} · ${c.id}`;
  if(jobsStatus==="loading") return <Loading/>;
  if(jobsStatus==="unavailable") return <ErrorState message={t("crawlStatusRetry")} retry={onRetryStatus}/>;
  if(!baseline||!current) return <section className="oc-panel comparison-empty"><Empty icon={<GitCompareArrows size={30}/>}><h2>{t("compareNeedTwo")}</h2><p>{t("compareNeedTwoNote")}</p><Button primary onClick={onSessions}>{t("sessions")}</Button></Empty></section>;
  return <div className="comparison-page">
    <section className="oc-panel comparison-setup">
      <div className="comparison-pair">
        <label>{t("compareBaseline")}<select disabled={preparing} value={baseline.id} onChange={e=>setBaseline(e.target.value)}>{eligible.map(c=><option key={c.id} value={c.id} disabled={c.id===current.id}>{label(c)}</option>)}</select></label>
        <Button className="icon" disabled={preparing} aria-label={t("compareSwap")} onClick={()=>{setBaseline(current.id);setCurrent(baseline.id);}}><ArrowLeftRight size={17}/></Button>
        <label>{t("compareCurrent")}<select disabled={preparing} value={current.id} onChange={e=>setCurrent(e.target.value)}>{eligible.map(c=><option key={c.id} value={c.id} disabled={c.id===baseline.id}>{label(c)}</option>)}</select></label>
      </div>
      <div className="row between comparison-scope"><p>{t("compareExact")}</p><label className="row"><input type="checkbox" disabled={preparing} checked={internalOnly} onChange={e=>setInternal(e.target.checked)}/>{t("compareInternal")}</label></div>
      <div className="row between comparison-prepare"><p>{t("comparePrepareNote")}</p><Button small busy={preparing} onClick={()=>void prepare()}><RefreshCw size={15}/>{t("comparePrepare")}</Button></div>
    </section>
    <ComparisonViews baseline={baseline.id} current={current.id} internalOnly={internalOnly} preparing={preparing} notify={notify}/>
  </div>;
}

function ComparisonResults({baseline,current,internalOnly,notify}:{baseline:string;current:string;internalOnly:boolean;notify:(text:string,error?:boolean)=>void}) {
  const {t,lang}=useI18n();
  const [filter,setFilter]=useState<ComparisonFilter>("all");
  const [field,setField]=useState("");
  const [search,setSearch]=useState("");
  const [offset,setOffset]=useState(0);
  const [selected,setSelected]=useState("");
  const closeDetail=useCallback(()=>setSelected(""),[]);
  const [exporting,setExporting]=useState(false);
  const request:ComparisonRequest={filter,field,search,internal_only:internalOnly,offset,limit:250};
  const identity=comparisonIdentity(baseline,current,request);
  const summaryRequest=comparisonSummaryRequest(request);
  const summary=useQuery({queryKey:["comparison-summary",comparisonIdentity(baseline,current,summaryRequest)],queryFn:()=>api.comparisonSummary(baseline,current,internalOnly,summaryRequest),placeholderData:keepPreviousData});
  const page=useQuery({queryKey:["comparison-page",identity],queryFn:()=>api.comparisonPage(baseline,current,request),enabled:summary.isSuccess&&!summary.isFetching});
  const detail=useQuery({queryKey:["comparison-detail",baseline,current,selected,internalOnly],queryFn:()=>api.comparisonDetail(baseline,current,selected,internalOnly),enabled:!!selected});
  const ready=page.isSuccess&&!page.isFetching&&summary.isSuccess&&!summary.isFetching;
  async function exportRows(format:"csv"|"xlsx"|"json") {
    if(!ready) return;
    setExporting(true);
    try {const count=await api.exportComparison(baseline,current,request,format);if(count!==null)notify(`${t("exportDone")} · ${number(count,lang)} ${t("rows")}`);}catch(error){notify(errorText(error),true);}finally{setExporting(false);}
  }
  return <>
    <div className="oc-banner comparison-note"><GitCompareArrows size={18}/><p>{t("compareMembershipNote")}</p></div>
    {summary.isPending?<Loading/>:summary.error?<ErrorState message={errorText(summary.error)} retry={()=>void summary.refetch()}/>:<>
      {!!summary.data.warnings.length&&<div className="comparison-warnings" role="status">{summary.data.warnings.map(w=><p key={w}>{comparisonWarningKeys[w]?t(comparisonWarningKeys[w]):w}</p>)}</div>}
      <p className="comparison-counter-note">{t("compareSummaryScope")}</p>
      <div className="comparison-stats" aria-busy={summary.isFetching}>{filters.slice(1).map(kind=><button key={kind} className={`oc-panel compare-stat ${filter===kind?"active":""}`} aria-pressed={filter===kind} disabled={summary.isFetching} onClick={()=>{setFilter(kind);setOffset(0);}}><span>{t(comparisonFilterKeys[kind])}</span><strong>{summary.isFetching?"…":number(summary.data[kind as "new"|"missing"|"changed"|"unchanged"],lang)}</strong></button>)}</div>
      <section className="oc-panel comparison-table">
        <div className="comparison-toolbar">
          <label className="compare-search"><Search size={15}/><input aria-label={t("search")} placeholder={t("search")} value={search} onChange={e=>{setSearch(e.target.value);setOffset(0);}}/></label>
          <select aria-label={t("compareResultFilter")} value={filter} onChange={e=>{setFilter(e.target.value as ComparisonFilter);setOffset(0);}}>{filters.map(f=><option value={f} key={f}>{t(comparisonFilterKeys[f])}</option>)}</select>
          <select aria-label={t("compareFieldFilter")} value={field} onChange={e=>{setField(e.target.value);setFilter("changed");setOffset(0);}}><option value="">{t("compareAllFields")}</option>{summary.data.fields.map(f=><option value={f.field} key={f.field}>{columnLabel(f.field,t)}</option>)}</select>
          <Button className="icon" aria-label={t("findingClearFilters")} onClick={()=>{setFilter("all");setField("");setSearch("");setOffset(0);}}><FilterX size={16}/></Button>
        </div>
        <div className="row between comparison-export"><small>{t("compareExportNote")}</small><div className="row">{(["csv","xlsx","json"] as const).map(format=><Button key={format} small busy={exporting} disabled={!ready||(isDemo&&format==="xlsx")} onClick={()=>void exportRows(format)}><ArrowDownToLine size={13}/>{format.toUpperCase()}</Button>)}</div></div>
        {summary.isFetching||page.isPending?<Loading/>:page.error?<ErrorState message={errorText(page.error)} retry={()=>void page.refetch()}/>:<DataTable data={page.data} onSelect={setSelected} selected={selected}/>}
        <div className="row between comparison-pagination"><span>{page.data?`${number(page.data.total,lang)} ${t("rows")} · ${number(Math.min(offset+1,page.data.total),lang)}–${number(Math.min(offset+250,page.data.total),lang)}`:t("loading")}</span><div className="row"><Button small aria-label={t("previous")} disabled={!ready||offset===0} onClick={()=>setOffset(Math.max(0,offset-250))}><ChevronLeft size={16}/></Button><Button small aria-label={t("next")} disabled={!ready||offset+250>=(page.data?.total??0)} onClick={()=>setOffset(offset+250)}><ChevronRight size={16}/></Button></div></div>
      </section>
    </>}
    {!!selected&&<Modal title={t("compareDetail")} className="comparison-modal" onClose={closeDetail}><p className="comparison-url">{selected}</p>{detail.isPending?<Loading/>:detail.error?<ErrorState message={errorText(detail.error)} retry={()=>void detail.refetch()}/>:<ComparisonDetails key={selected} data={detail.data}/>}</Modal>}
  </>;
}


function ComparisonViews({preparing,...props}:{baseline:string;current:string;internalOnly:boolean;preparing:boolean;notify:(text:string,error?:boolean)=>void}) {
  const {t}=useI18n();
  const [view,setView]=useState<"urls"|"findings">("urls");
  // Keep the selected view across direction/scope changes; reset only its result context.
  const context=JSON.stringify([props.baseline,props.current,props.internalOnly]);
  return <><div className="oc-tabs comparison-views" role="group" aria-label={t("findingCompareView")}><button disabled={preparing} className={`oc-tab ${view==="urls"?"active":""}`} aria-pressed={view==="urls"} onClick={()=>setView("urls")}>{t("findingUrlView")}</button><button disabled={preparing} className={`oc-tab ${view==="findings"?"active":""}`} aria-pressed={view==="findings"} onClick={()=>setView("findings")}>{t("findingView")}</button></div>{preparing?<div className="oc-banner info" role="status">{t("comparePreparing")}</div>:view==="urls"?<ComparisonResults key={context} {...props}/>:<FindingComparison key={context} {...props}/>}</>;
}
