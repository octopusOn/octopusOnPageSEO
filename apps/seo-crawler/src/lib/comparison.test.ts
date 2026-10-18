import {expect,it,vi} from "vitest";
import {defaultConfig,type CrawlMeta,type Job,type ComparisonRequest,type TablePage} from "../api/types";
import {comparisonCandidates,comparisonDetailRows,comparisonIdentity,prepareComparisonArchives,ComparisonPreparationError} from "./comparison";
const meta:CrawlMeta={id:"one",name:"Example",start_urls:[],created_at_ms:0,finished_at_ms:1,status:"completed",progress:{crawled:0,queued:0,errors:0,elapsed_ms:0,discovered:0,in_flight:0,sitemaps:0,paused:false},limit_reached:false,error:null,config:defaultConfig};
it("excludes unfinished and finalizing archives even when metadata already says completed",()=>{
  const jobs:Job[]=[{id:"active",crawl_id:"one",url:"",status:"running",phase:"finalizing",progress:meta.progress}];
  const crawls=[meta,{...meta,id:"two",status:"stopped" as const},{...meta,id:"three",status:"failed" as const},{...meta,id:"unfinished",status:"running" as const}];
  expect(comparisonCandidates(crawls,jobs).map(c=>c.id)).toEqual(["two","three"]);
  expect(crawls[0]?.status).toBe("completed");
});
it("keeps both direction and all filters in the result identity",()=>{
  const request:ComparisonRequest={filter:"all",field:"",search:"",internal_only:true,offset:0,limit:250};
  const key=comparisonIdentity("a","b",request);
  for(const changed of [{...request,filter:"changed" as const},{...request,field:"title"},{...request,search:"' OR true"},{...request,internal_only:false},{...request,offset:250},{...request,limit:1}]) expect(comparisonIdentity("a","b",changed)).not.toBe(key);
  expect(comparisonIdentity("b","a",request)).not.toBe(key);
});
it("preserves NULL, empty strings, booleans and exact fingerprint text in before/after details",()=>{
  const data:TablePage={columns:["field","before_value","after_value","changed"].map(name=>({name,kind:"text"})),rows:[["title",null,"",true],["text_hash","18446744073709551614","18446744073709551615",true],["noindex",false,false,false]],total:3};
  expect(comparisonDetailRows(data,true)).toEqual([{field:"title",before:null,after:"",changed:true},{field:"text_hash",before:"18446744073709551614",after:"18446744073709551615",changed:true}]);
  expect(comparisonDetailRows(data,false)[2]).toEqual({field:"noindex",before:false,after:false,changed:false});
});

it("keeps URL indicator search and field filters across membership and page changes",async()=>{
  const {comparisonSummaryRequest}=await import("./comparison");
  const request={filter:"missing" as const,field:"title",search:"EDITED",internal_only:false,offset:250,limit:1};
  expect(comparisonSummaryRequest(request)).toEqual({...request,filter:"all",offset:0,limit:250});
  expect(comparisonSummaryRequest({...request,filter:"changed",offset:500})).toEqual(comparisonSummaryRequest(request));
  expect(comparisonSummaryRequest({...request,field:"canonical"})).not.toEqual(comparisonSummaryRequest(request));
  expect(comparisonSummaryRequest({...request,search:"new"})).not.toEqual(comparisonSummaryRequest(request));
});

it("prepares exactly the selected finished archives sequentially after a fresh jobs snapshot",async()=>{
  const baseline=structuredClone(meta),current={...structuredClone(meta),id:"two",status:"stopped" as const};
  let release!:()=>void;
  const gate=new Promise<void>(resolve=>{release=resolve;});
  const readJobs=vi.fn(async()=>[] as Job[]);
  const open=vi.fn(async(_id:string)=>{}).mockImplementationOnce(()=>gate);
  const before=JSON.stringify([baseline,current]);
  const prepared=prepareComparisonArchives(baseline,current,readJobs,open);
  await vi.waitFor(()=>expect(open).toHaveBeenCalledTimes(1));
  expect(readJobs).toHaveBeenCalledOnce();expect(open.mock.calls[0]).toEqual(["one"]);
  release();await prepared;
  expect(open.mock.calls).toEqual([["one"],["two"]]);
  expect(JSON.stringify([baseline,current])).toBe(before);
});
it("refuses duplicate or unfinished selections before opening any archive",async()=>{
  const readJobs=vi.fn(async()=>[] as Job[]),open=vi.fn(async(_id:string)=>{});
  for(const current of [meta,{...meta,id:"two",status:"running" as const},{...meta,id:""}])
    await expect(prepareComparisonArchives(meta,current,readJobs,open)).rejects.toBeInstanceOf(ComparisonPreparationError);
  expect(readJobs).not.toHaveBeenCalled();expect(open).not.toHaveBeenCalled();
});
it("refuses active and finalizing jobs even when saved metadata already says completed",async()=>{
  const current={...meta,id:"two"},open=vi.fn(async(_id:string)=>{});
  for(const crawl_id of [meta.id,current.id])for(const status of ["running","paused","stopping"] as const) {
    const job:Job={id:"job",crawl_id,url:"",status,phase:"finalizing",progress:meta.progress};
    await expect(prepareComparisonArchives(meta,current,async()=>[job],open)).rejects.toBeInstanceOf(ComparisonPreparationError);
  }
  expect(open).not.toHaveBeenCalled();
  await prepareComparisonArchives(meta,current,async()=>[{id:"other",crawl_id:"unrelated",url:"",status:"running",progress:meta.progress}],open);
  expect(open.mock.calls).toEqual([["one"],["two"]]);
});
it("does not open an archive when live job status is unavailable",async()=>{
  const open=vi.fn(async(_id:string)=>{});
  await expect(prepareComparisonArchives(meta,{...meta,id:"two"},async()=>{throw new Error("offline");},open)).rejects.toThrow("offline");
  expect(open).not.toHaveBeenCalled();
});
it("stops preparation on the first failed archive and propagates partial failures for retry",async()=>{
  const open=vi.fn(async(_id:string)=>{}).mockRejectedValueOnce(new Error("cannot upgrade"));
  await expect(prepareComparisonArchives(meta,{...meta,id:"two"},async()=>[],open)).rejects.toThrow("cannot upgrade");
  expect(open.mock.calls).toEqual([["one"]]);
  open.mockReset().mockResolvedValueOnce().mockRejectedValueOnce(new Error("second archive failed"));
  await expect(prepareComparisonArchives(meta,{...meta,id:"two"},async()=>[],open)).rejects.toThrow("second archive failed");
  expect(open.mock.calls).toEqual([["one"],["two"]]);
});
