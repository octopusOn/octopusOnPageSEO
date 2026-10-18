import {expect,it} from "vitest";
import type {FindingComparisonRequest,FindingRuleCount,TablePage} from "../api/types";
import {findingSummaryRequest,findingChanges,findingCount,findingIdentity,findingKey,parseFindingKey,keyedFindingTable,findingReason} from "./findingComparison";
const request:FindingComparisonRequest={change:"all",issue_id:"",priority:"",search:"",internal_only:true,offset:0,limit:250};
it("identifies findings by both rule and exact URL, including quotes and delimiter characters",()=>{
  const url="https://example.com/?q='&array=]\"";
  const a=findingKey("title-missing",url),b=findingKey("description-missing",url);
  expect(a).not.toBe(b);expect(parseFindingKey(a)).toEqual({issueId:"title-missing",url});
  for(const value of ["bad json","[]",'["a"]','["a",2]','["a","b","c"]','["",""]'])expect(parseFindingKey(value)).toBeNull();
});
it("adds bounded row keys without mutating data or losing duplicate URL rows",()=>{
  const data:TablePage={columns:[{name:"address",kind:"text"},{name:"issue_id",kind:"text"}],rows:[["https://example.com/","title-missing"],["https://example.com/","description-missing"]],total:2};
  const keyed=keyedFindingTable(data);
  expect(keyed.rows[0]?.[2]).not.toBe(keyed.rows[1]?.[2]);expect(data.columns).toHaveLength(2);expect(data.rows[0]).toHaveLength(2);
});
it("keeps direction, URL scope and all filters in the query identity",()=>{
  const key=findingIdentity("before","after",request);
  for(const next of [{...request,change:"not-detected" as const},{...request,issue_id:"title-missing"},{...request,priority:"high"},{...request,search:"' OR true --"},{...request,internal_only:false},{...request,offset:250}])expect(findingIdentity("before","after",next)).not.toBe(key);
  expect(findingIdentity("after","before",request)).not.toBe(key);
});
it("maps every disjoint counter correctly and translates counterpart contexts",()=>{
  const counts:FindingRuleCount={id:"",priority:"",category:"",introduced:1,not_detected:2,persisting:3,new_url:4,missing_url:5,unverified:6,total:21};
  expect(findingChanges.map(state=>findingCount(counts,state))).toEqual([21,1,2,3,4,5,6]);
  const t=(key:Parameters<typeof findingReason>[1] extends (key:infer K)=>string?K:never)=>String(key);
  expect(findingReason("after-not-indexable",t)).toBe("compareCurrent: findingReasonIndexability");
  expect(findingReason("before-audit-baseline-unrecorded",t)).toBe("compareBaseline: findingReasonAuditBaseline");
  expect(findingReason("historical-new-context",t)).toBe("historical-new-context");
});

it("keeps indicator filters while ignoring the selected state and page",()=>{
  const filtered={...request,issue_id:"description-missing",priority:"low",search:"/page/4",change:"introduced" as const,offset:250,limit:1};
  const summary=findingSummaryRequest(filtered);
  expect(summary).toEqual({...filtered,change:"all",offset:0,limit:250});
  expect(findingSummaryRequest({...filtered,change:"persisting",offset:500,limit:1000})).toEqual(summary);
  expect(filtered.offset).toBe(250);
  for(const key of ["issue_id","priority","search"] as const)expect(findingSummaryRequest({...filtered,[key]:"different"})).not.toEqual(summary);
});

it("explains missing hreflang coverage and unsuitable HTML/PDF observations on either side",()=>{
  const t=(key:Parameters<typeof findingReason>[1] extends (key:infer K)=>string?K:never)=>String(key);
  for(const [side,label] of [["before","compareBaseline"],["after","compareCurrent"]]) {
    expect(findingReason(side+"-hreflang-unrecorded",t)).toBe(label+": findingReasonHreflangCoverage");
    expect(findingReason(side+"-not-hreflang-200",t)).toBe(label+": findingReasonHreflangResponse");
    expect(findingReason(side+"-hreflang-html-unparsed",t)).toBe(label+": findingReasonHreflangHtml");
  }
});
