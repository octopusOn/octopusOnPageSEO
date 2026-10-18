import { expect, it } from "vitest";
import { defaultConfig, type CrawlMeta, type Job } from "../api/types";
import { prepareRepeat, savedCrawlState } from "./sessions";

export const crawl: CrawlMeta={id:"saved",name:"example.com",start_urls:["https://example.com/a","https://example.com/b"],created_at_ms:0,finished_at_ms:null,status:"running",progress:{crawled:7,queued:3,errors:0,elapsed_ms:1000,discovered:10,in_flight:0,sitemaps:0,paused:false},limit_reached:false,error:null,config:{...defaultConfig,mode:"list",start_urls:["https://example.com/a","https://example.com/b"],speed:{threads:3,max_requests_per_second:2},limits:{max_urls:50,max_depth:4,timeout_seconds:15},urls:{...defaultConfig.urls,include:["/allowed"],exclude:["/private"],strip_params:["tracking"]},storage:{headers:false,html:true},rendering:{mode:"javascript",wait_ms:1500,timeout_seconds:25,concurrency:1,browser_path:"/browser"}}};
const active=(status:Job["status"]):Job=>({id:"job",crawl_id:crawl.id,url:crawl.start_urls[0]!,status,phase:"finalizing",progress:crawl.progress});

it("requires a successful jobs snapshot before diagnosing an interruption or enabling replay",()=>{
  for(const status of ["loading","unavailable"] as const) {
    const state=savedCrawlState(crawl,[],status);
    expect(state.inactive).toBe(false); expect(state.canRepeat).toBe(false);
  }
  expect(savedCrawlState(crawl,[],"ready")).toMatchObject({label:"interrupted",inactive:true,canRepeat:true});
});
it("preserves running, paused, stopping and finalizing jobs even if metadata already finished",()=>{
  for(const status of ["running","paused","stopping"] as const) {
    const job=active(status);
    const state=savedCrawlState({...crawl,status:"completed"},[job],"ready");
    expect(state.active).toBe(job); expect(state.label).toBe("finalizing"); expect(state.canRepeat).toBe(false);
  }
  expect(savedCrawlState(crawl,[{...active("running"),crawl_id:"other"}],"ready").inactive).toBe(true);
  expect(savedCrawlState(crawl,[active("completed")],"ready").inactive).toBe(true);
});
it("copies the complete plan including URL order and limits without mutating the source",()=>{
  const source=structuredClone(crawl);
  source.config.rendering.viewport={width:768,height:1024,mobile:true};
  source.config.rendering.cache_resources=false;
  const before=structuredClone(source);
  const plan=prepareRepeat(source);
  expect(plan).toEqual(source.config);
  plan.start_urls.reverse(); plan.urls.include.push("changed"); plan.limits.max_urls=0; plan.rendering.wait_ms=2000; plan.storage.html=false;
  plan.rendering.viewport!.width=800;
  expect(source).toEqual(before);
});
it("retains automatic layout when a historical rendering object has no viewport",()=>{
  expect(prepareRepeat(crawl).rendering.viewport).toBeNull();
  expect(prepareRepeat(crawl).rendering.cache_resources).toBe(true);
});
it("provides default rendering settings for historical HTML plans",()=>{
  const historical=structuredClone(crawl);
  delete (historical.config as Partial<typeof historical.config>).rendering;
  expect(prepareRepeat(historical).rendering).toEqual(defaultConfig.rendering);
});
