import {useCallback,useMemo,useState} from "react";
import {keepPreviousData,useQuery} from "@tanstack/react-query";
import {ArrowDownToLine,ChevronLeft,ChevronRight,CircleAlert,FilterX,Search} from "lucide-react";
import {api,errorText,isDemo} from "../api/client";
import type {Cell,FindingChange,FindingComparisonRequest,TablePage} from "../api/types";
import {issueById} from "../catalog";
import {useI18n,number} from "../i18n";
import {Button,Empty,ErrorState,Loading,Modal} from "../components/ui";
import {ComparisonDetails} from "../components/ComparisonDetails";
import {DataTable} from "../components/DataTable";
import {findingSummaryRequest,findingCell,findingChangeKeys,findingChanges,findingCount,findingIdentity,findingReason,keyedFindingTable,parseFindingKey} from "../lib/findingComparison";

import {comparisonWarningKeys} from "../lib/comparison";

const visible=["address","issue_id","finding_change","priority","comparison_reason","before_status_code","after_status_code"];
export function FindingComparison({baseline,current,internalOnly,notify}:{baseline:string;current:string;internalOnly:boolean;notify:(text:string,error?:boolean)=>void}) {
  const {t,lang}=useI18n();
  const [change,setChange]=useState<FindingChange>("all");
  const [issueId,setIssue]=useState("");
  const [priority,setPriority]=useState("");
  const [search,setSearch]=useState("");
  const [offset,setOffset]=useState(0);
  const [selected,setSelected]=useState("");
  const [exporting,setExporting]=useState(false);
  const close=useCallback(()=>setSelected(""),[]);
  const request:FindingComparisonRequest={change,issue_id:issueId,priority,search,internal_only:internalOnly,offset,limit:250};
  const summaryRequest=findingSummaryRequest(request);
  const summary=useQuery({queryKey:["finding-summary",findingIdentity(baseline,current,summaryRequest)],queryFn:()=>api.findingComparisonSummary(baseline,current,internalOnly,summaryRequest),placeholderData:keepPreviousData});
  const page=useQuery({queryKey:["finding-page",findingIdentity(baseline,current,request)],queryFn:()=>api.findingComparisonPage(baseline,current,request),enabled:summary.isSuccess&&!summary.isFetching});
  const keyed=useMemo(()=>page.data?keyedFindingTable(page.data):undefined,[page.data]);
  const selection=parseFindingKey(selected);
  const detail=useQuery({queryKey:["comparison-detail",baseline,current,selection?.url??"",internalOnly],queryFn:()=>api.comparisonDetail(baseline,current,selection!.url,internalOnly),enabled:!!selection});
  const ready=page.isSuccess&&!page.isFetching&&summary.isSuccess&&!summary.isFetching;
  const selectedRow=keyed?.rows.find(row=>row.at(-1)===selected);
  const reset=()=>{setChange("all");setIssue("");setPriority("");setSearch("");setOffset(0);};
  async function exportRows(format:"csv"|"xlsx"|"json") {
    if(!ready)return;
    setExporting(true);
    try {const rows=await api.exportFindingComparison(baseline,current,request,format);if(rows!==null)notify(`${t("exportDone")} · ${number(rows,lang)} ${t("findingPairs")}`);}catch(error){notify(errorText(error),true);}finally{setExporting(false);}
  }
  return <div className="finding-comparison">
    <div className="oc-banner comparison-note"><CircleAlert size={18}/><p>{t("findingObservationNote")}</p></div>
    {summary.isPending?<Loading/>:summary.error?<ErrorState message={errorText(summary.error)} retry={()=>void summary.refetch()}/>:<>
      {summary.data.warnings.some(w=>w!=="finding-observations")&&<div className="comparison-warnings" role="status">{summary.data.warnings.filter(w=>w!=="finding-observations").map(w=><p key={w}>{comparisonWarningKeys[w]?t(comparisonWarningKeys[w]):w}</p>)}</div>}
      <div className="row between finding-totals"><span>{t("compareBaseline")}: <strong>{summary.isFetching?"…":number(summary.data.baseline,lang)}</strong> · {t("compareCurrent")}: <strong>{summary.isFetching?"…":number(summary.data.current,lang)}</strong> {t("findingPairs")}</span><small>{t("findingSummaryScope")}</small></div>
      <div className="finding-stats" aria-busy={summary.isFetching}>{findingChanges.filter(c=>c!=="all").map(kind=><button className={`oc-panel compare-stat ${change===kind?"active":""}`} key={kind} aria-pressed={change===kind} disabled={summary.isFetching} onClick={()=>{setChange(kind);setOffset(0);}}><span>{t(findingChangeKeys[kind])}</span><strong>{summary.isFetching?"…":number(findingCount(summary.data.counts,kind),lang)}</strong></button>)}</div>
      <section className="oc-panel comparison-table">
        <div className="finding-toolbar">
          <label className="compare-search"><Search size={15}/><input aria-label={t("search")} placeholder={t("search")} value={search} onChange={e=>{setSearch(e.target.value);setOffset(0);}}/></label>
          <select aria-label={t("findingStateFilter")} value={change} onChange={e=>{setChange(e.target.value as FindingChange);setOffset(0);}}>{findingChanges.map(c=><option key={c} value={c}>{t(findingChangeKeys[c])}</option>)}</select>
          <select aria-label={t("findingPriority")} value={priority} onChange={e=>{setPriority(e.target.value);setOffset(0);}}><option value="">{t("findingAllPriorities")}</option>{(["high","medium","low"] as const).map(p=><option key={p} value={p}>{t(p)}</option>)}<option value="unknown">{t("findingUnknownPriority")}</option></select>
          <select aria-label={t("findingCheck")} value={issueId} onChange={e=>{setIssue(e.target.value);setOffset(0);}}><option value="">{t("findingAllChecks")}</option>{issueId&&!summary.data.rules.some(rule=>rule.id===issueId)&&<option value={issueId}>{issueById.get(issueId)?.[lang].title??issueId}</option>}{summary.data.rules.map(rule=><option key={rule.id} value={rule.id}>{issueById.get(rule.id)?.[lang].title??rule.id}</option>)}</select>
          <Button className="icon" aria-label={t("findingClearFilters")} onClick={reset}><FilterX size={16}/></Button>
        </div>
        {summary.data.rule_total>summary.data.rules.length&&<p className="finding-rule-limit">{t("findingRuleLimit")} {number(summary.data.rules.length,lang)} / {number(summary.data.rule_total,lang)}</p>}
        <div className="row between comparison-export"><small>{t("findingExportNote")}</small><div className="row">{(["csv","xlsx","json"] as const).map(format=><Button key={format} small busy={exporting} disabled={!ready||(isDemo&&format==="xlsx")} onClick={()=>void exportRows(format)}><ArrowDownToLine size={13}/>{format.toUpperCase()}</Button>)}</div></div>
        {summary.isFetching||page.isPending?<Loading/>:page.error?<ErrorState message={errorText(page.error)} retry={()=>void page.refetch()}/>:keyed?<DataTable data={keyed} visible={visible} selectColumn="finding_key" selected={selected} onSelect={setSelected}/>:<Empty>{t("noResults")}</Empty>}
        <div className="row between comparison-pagination"><span>{page.data?`${number(page.data.total,lang)} ${t("findingPairs")} · ${number(Math.min(offset+1,page.data.total),lang)}–${number(Math.min(offset+250,page.data.total),lang)}`:t("loading")}</span><div className="row"><Button small aria-label={t("previous")} disabled={!ready||offset===0} onClick={()=>setOffset(Math.max(0,offset-250))}><ChevronLeft size={16}/></Button><Button small aria-label={t("next")} disabled={!ready||offset+250>=(page.data?.total??0)} onClick={()=>setOffset(offset+250)}><ChevronRight size={16}/></Button></div></div>
      </section>
    </>}
    {selection&&<Modal title={t("findingDetail")} className="comparison-modal" onClose={close}>
      {selectedRow&&keyed&&<FindingObservation data={keyed} row={selectedRow}/>}
      <p className="comparison-url">{selection.url}</p>
      <Button small onClick={()=>{setIssue(selection.issueId);setChange("all");setPriority("");setSearch("");setOffset(0);setSelected("");}}>{t("findingSameCheck")}</Button>
      <div className="finding-url-facts">{detail.isPending?<Loading/>:detail.error?<ErrorState message={errorText(detail.error)} retry={()=>void detail.refetch()}/>:detail.data?.total?<ComparisonDetails key={selected} data={detail.data}/>:<p>{t("findingNoUrlFacts")}</p>}</div>
    </Modal>}
  </div>;
}

export function FindingObservation({data,row}:{data:TablePage;row:Cell[]}) {
  const {t,lang}=useI18n();
  const id=String(findingCell(data,row,"issue_id")??""),issue=issueById.get(id);
  const state=String(findingCell(data,row,"finding_change")??"") as FindingChange;
  return <section className="finding-observation"><h3>{issue?.[lang].title??id}</h3><div className="row finding-observation-badges"><span className={`oc-badge finding-${state}`}>{findingChangeKeys[state]?t(findingChangeKeys[state]):state}</span><span>{t("compareBaseline")}: <b>{t(findingCell(data,row,"before_finding")===true?"findingRecorded":"findingNotRecorded")}</b></span><span>{t("compareCurrent")}: <b>{t(findingCell(data,row,"after_finding")===true?"findingRecorded":"findingNotRecorded")}</b></span></div><p>{state==="persisting"?t("findingRecordedNote"):findingReason(String(findingCell(data,row,"comparison_reason")??""),t)}</p>{issue?<details><summary>{t("why")} / {t("fix")}</summary><p>{issue[lang].description}</p><p>{issue[lang].fix}</p></details>:<p>{t("urlFindingsUnknown")}</p>}</section>;
}
