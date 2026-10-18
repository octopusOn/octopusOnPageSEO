import type {Cell,FindingChange,FindingComparisonRequest,FindingRuleCount,TablePage} from "../api/types";
import type {Key} from "../i18n";

export const findingChanges:FindingChange[]=["all","introduced","not-detected","persisting","new-url","missing-url","unverified"];
export const findingChangeKeys:Record<FindingChange,Key>={all:"findingAll",introduced:"findingIntroduced","not-detected":"findingNotDetected",persisting:"findingPersisting","new-url":"findingNewUrl","missing-url":"findingMissingUrl",unverified:"findingUnverified"};
const countKeys:Record<Exclude<FindingChange,"all">,keyof FindingRuleCount>={introduced:"introduced","not-detected":"not_detected",persisting:"persisting","new-url":"new_url","missing-url":"missing_url",unverified:"unverified"};
export function findingCount(counts:FindingRuleCount,change:FindingChange):number {
  return change==="all"?counts.total:Number(counts[countKeys[change]]);
}
const reasonKeys:Record<string,Key>={"schema-check-unobserved":"findingReasonSchema","url-only-current":"findingReasonNewUrl","url-only-baseline":"findingReasonMissingUrl","no-url-observation":"findingReasonNoUrl","classification-changed":"findingReasonClassification","unknown-rule":"findingReasonUnknownRule","scope-not-comparable":"findingReasonScope"};
const observationKeys:Record<string,Key>={"audit-baseline-unrecorded":"findingReasonAuditBaseline","hreflang-unrecorded":"findingReasonHreflangCoverage","not-hreflang-200":"findingReasonHreflangResponse","hreflang-html-unparsed":"findingReasonHreflangHtml","http-unobserved":"findingReasonHttp","not-html-200":"findingReasonHtml","not-indexable":"findingReasonIndexability","incomplete-content":"findingReasonIncomplete","rendering-unverified":"findingReasonRendering"};
export function findingReason(reason:string,t:(key:Key)=>string):string {
  if(!reason)return t("findingComparable");
  if(reasonKeys[reason])return t(reasonKeys[reason]);
  for(const side of ["before","after"] as const) {
    const key=observationKeys[reason.slice(side.length+1)];
    if(reason.startsWith(side+"-")&&key)return `${t(side==="before"?"compareBaseline":"compareCurrent")}: ${t(key)}`;
  }
  return reason;
}
export function findingIdentity(baseline:string,current:string,request:FindingComparisonRequest) {return JSON.stringify([baseline,current,request]);}
export type SelectedFinding={url:string;issueId:string};
export function findingKey(issueId:string,url:string) {return JSON.stringify([issueId,url]);}
export function parseFindingKey(value:string):SelectedFinding|null {
  try {const pair:unknown=JSON.parse(value);return Array.isArray(pair)&&pair.length===2&&pair.every(v=>typeof v==="string"&&v.length>0)?{issueId:pair[0] as string,url:pair[1] as string}:null;}catch{return null;}
}
export function keyedFindingTable(data:TablePage):TablePage {
  const issue=data.columns.findIndex(c=>c.name==="issue_id"),url=data.columns.findIndex(c=>c.name==="address");
  if(issue<0||url<0)return {...data,rows:[]};
  return {...data,columns:[...data.columns,{name:"finding_key",kind:"text"}],rows:data.rows.map(row=>[...row,findingKey(String(row[issue]??""),String(row[url]??""))])};
}
export function findingCell(data:TablePage,row:Cell[],name:string):Cell {return row[data.columns.findIndex(c=>c.name===name)]??null;}

// Indicators span all states for the same non-state filters as the result table.
export function findingSummaryRequest(request:FindingComparisonRequest):FindingComparisonRequest {return {...request,change:"all",offset:0,limit:250};}
