import {expect,it} from "vitest";
import {demo} from "./demo";
import type {CrawlMeta,Overview,RemediationDetail,RemediationRequest} from "./types";
it("keeps browser remediation URL totals aligned with scored deductions and searches literally",async()=>{
 const crawls=await demo("list_crawls",{}) as CrawlMeta[];
 const id=crawls[0]!.id;
 const overview=await demo("crawl_overview",{id}) as Overview;
 for(const deduction of overview.seo_score!.deductions.slice(0,4)) {
  const request:RemediationRequest={issue_id:deduction.id,search:"",offset:0,limit:2,url:"",evidence_offset:0};
  const detail=await demo("score_remediation",{id,request}) as RemediationDetail;
  expect(detail.urls.total).toBe(deduction.count);expect(detail.urls.rows.length).toBeLessThanOrEqual(2);
  expect(detail.selected_url).toBe(detail.urls.rows[0]?.[0]??null);
  const empty=await demo("score_remediation",{id,request:{...request,search:"' OR true --"}}) as RemediationDetail;
  expect(empty.urls.total).toBe(0);expect(empty.selected_url).toBeNull();
 }
});
