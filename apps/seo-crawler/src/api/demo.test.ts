import { it, expect, vi } from "vitest";
import { demo, exportDemoTable } from "./demo";
import type {
  CrawlMeta,
  CrawlStorage,
  Overview,
  PageRequest,
  TablePage,
  UrlDetail,
  Job,
} from "./types";
import { defaultConfig } from "./types";

it("does not invent native disk measurements for browser sample crawls", async () => {
  const crawls = await demo("list_crawls", {}) as CrawlMeta[];
  expect(await demo("crawl_storage", {}) as CrawlStorage[]).toEqual(crawls.map(crawl => ({id:crawl.id,bytes:null})));
});

it("keeps non-redirect sitemap responses distinct from unknown redirect observations",async()=>{
  const [crawl]=await demo("list_crawls",{}) as CrawlMeta[];
  const maps=await exportDemoTable(crawl!.id,"sitemaps");
  const index=(name:string)=>maps.columns.findIndex(column=>column.name===name);
  expect(maps.total).toBe(crawl!.progress.sitemaps);
  for(const row of maps.rows) {
    expect(row.length).toBe(maps.columns.length);expect(row[index("status_code")]).toBe(200);
    for(const name of ["redirect_url","redirect_location","redirect_outcome","final_url","final_status","redirect_hops"]) {
      expect(index(name)).toBeGreaterThan(-1);expect(row[index(name)]).toBeNull();
    }
  }
});

it("keeps original sitemap references separate from saved page facts and exports the full filtered set",async()=>{
  const [crawl]=(await demo("list_crawls",{}) as CrawlMeta[]);
  const rows=await exportDemoTable(crawl!.id,"sitemap-urls");
  const index=(name:string)=>rows.columns.findIndex(column=>column.name===name);
  expect(rows.rows.length).toBeGreaterThan(1);
  for(const row of rows.rows) {
    expect(row[index("url")]).toBe(`${String(row[index("crawl_url")])}#sitemap`);
    const detail=await demo("url_detail",{id:crawl!.id,url:row[index("crawl_url")]}) as UrlDetail;
    expect(detail.facts.rows[0]?.[2]).toBe(row[index("status_code")]);
  }
  const request:PageRequest={tab:"sitemap-urls",status_class:4,indexability:"client-error",search:"sitemap.xml",issue_id:"internal-client-error",sort:"crawl_url",descending:false,offset:1,limit:1};
  const page=await demo("crawl_page",{id:crawl!.id,request}) as TablePage;
  expect(page.total).toBeGreaterThan(1);expect(page.rows).toHaveLength(1);
  const exported=await exportDemoTable(crawl!.id,request.tab,{request,columns:["url","crawl_url","status_code"]});
  expect(exported.rows).toHaveLength(page.total);expect(exported.rows.every(row=>Number(row[2])===404)).toBe(true);
});

it("keeps both sample archives' saved counters and HTTP classifications consistent with their records",async()=>{
  const crawls=await demo("list_crawls",{}) as CrawlMeta[];
  for(const crawl of crawls) {
    const rows=await exportDemoTable(crawl.id,"internal");
    const overview=await demo("crawl_overview",{id:crawl.id}) as Overview;
    const column=(name:string)=>rows.columns.findIndex(c=>c.name===name);
    expect(crawl.progress.crawled).toBe(rows.total);
    expect(crawl.progress.discovered).toBe(rows.total);
    expect(crawl.progress.errors).toBe(overview.errors);
    expect(Number.isFinite(overview.average_response_ms)).toBe(true);
    expect(rows.rows.every(row=>row.length===rows.columns.length)).toBe(true);
    expect(rows.rows.every(row=>row[column("title_length")]===String(row[column("title")]).length)).toBe(true);
    expect(overview.indexable).toBe(rows.rows.filter(row=>row[column("indexability")]==="indexable").length);
    for(const [statusClass,category,rule] of [[4,"client-error","internal-client-error"],[5,"server-error","internal-server-error"]] as const) {
      const expected=rows.rows.filter(row=>Math.floor(Number(row[column("status_code")])/100)===statusClass);
      expect(expected.length).toBeGreaterThan(0);
      expect(expected.every(row=>row[column("indexability")]===category)).toBe(true);
      const request:PageRequest={tab:"response-codes",response_group:category,status_class:0,indexability:category,issue_id:rule,search:"",sort:"address",descending:false,offset:1,limit:1};
      const page=await demo("crawl_page",{id:crawl.id,request}) as TablePage;
      const exported=await exportDemoTable(crawl.id,request.tab,{request,columns:["address","status_code","indexability"]});
      expect(page.total).toBe(expected.length);
      expect(exported.rows).toHaveLength(expected.length);
      expect(exported.rows.every(row=>row[2]===category)).toBe(true);
      for(const row of expected) {
        const detail=await demo("url_detail",{id:crawl.id,url:row[0]}) as UrlDetail;
        expect(detail.findings!.rows).toEqual([[rule]]);
      }
    }
  }
  const previous=crawls.find(c=>c.id==="demo-atelier-previous")!;
  expect(previous.progress).toMatchObject({crawled:94,discovered:94,errors:7});
});

it("updates demo HTTP error counters during crawling and preserves them in the saved result",async()=>{
  vi.useFakeTimers();
  vi.stubGlobal("window",{setInterval:globalThis.setInterval,dispatchEvent:vi.fn()});
  let id="";
  try {
    id=await demo("start_crawl",{config:structuredClone(defaultConfig)}) as string;
    await vi.advanceTimersByTimeAsync(6000);
    const jobs=await demo("list_jobs",{}) as Job[];
    const running=jobs.find(job=>job.id===id)!;
    expect(running.status).toBe("running");
    expect(running.progress).toMatchObject({crawled:48,errors:3});
    expect(running.activity!.request_errors).toBe(running.progress.errors);
    await vi.advanceTimersByTimeAsync(6000);
    const finished=(await demo("list_jobs",{}) as Job[]).find(job=>job.id===id)!;
    const overview=await demo("crawl_overview",{id}) as Overview;
    expect(finished.status).toBe("completed");
    expect(finished.progress.errors).toBe(6);
    expect(finished.progress.errors).toBe(overview.errors);
    expect(finished.meta!.progress).toEqual(finished.progress);
  } finally {
    if(id)await demo("delete_crawl",{id});
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  }
});

it("returns only the exact selected URL's recorded demo findings",async()=>{
  const [crawl]=await demo("list_crawls",{}) as CrawlMeta[];
  const id=crawl!.id, url="https://atelier.example/products/arc-chair";
  const detail=await demo("url_detail",{id,url}) as UrlDetail;
  const all=await exportDemoTable(id,"issues");
  const expected=all.rows.filter((row)=>row[1]===url).map((row)=>[row[0]]).sort();
  expect(detail.findings!.rows).toEqual(expected);
  expect(detail.findings!.total).toBe(expected.length);
  const unknown=await demo("url_detail",{id,url:url+"/unrecorded"}) as UrlDetail;
  expect(unknown.findings!.total).toBe(0);
  expect(unknown.facts.total).toBe(0);
});

it("keeps overview response groups consistent with table filters and complete exports", async () => {
  const [crawl] = await demo("list_crawls", {}) as CrawlMeta[];
  const id=crawl!.id;
  const overview=await demo("crawl_overview",{id}) as Overview;
  expect(overview.responses!.reduce((sum,row)=>sum+row.count,0)).toBe(overview.internal+overview.external);
  for (const {group,count} of overview.responses!) {
    const request:PageRequest={tab:"response-codes",response_group:group,search:"",status_class:0,indexability:"",issue_id:"",offset:1,limit:1,sort:"address",descending:true};
    const page=await demo("crawl_page",{id,request}) as TablePage;
    expect(page.total).toBe(count);
    const exported=await exportDemoTable(id,"response-codes",{request,columns:["status_code","address"]});
    expect(exported.rows).toHaveLength(count);
    if(count>1) expect(exported.rows[1]![1]).toBe(page.rows[0]![0]);
  }
  const request:PageRequest={tab:"internal",response_group:"no-response",search:"",status_class:0,indexability:"",issue_id:"",offset:0,limit:1,sort:"",descending:false};
  await expect(demo("crawl_page",{id,request})).rejects.toThrow("response group");
});

it("browser demo overview, issue filters and detail counts agree", async () => {
  const [crawl] = (await demo("list_crawls", {})) as CrawlMeta[];
  expect(crawl).toBeDefined();
  const id = crawl!.id;
  const overview = (await demo("crawl_overview", { id })) as Overview;
  const request: PageRequest = {
    tab: "internal",
    search: "",
    issue_id: "title-missing",
    status_class: 0,
    indexability: "",
    sort: "address",
    descending: false,
    offset: 0,
    limit: 250,
  };
  const result = (await demo("crawl_page", { id, request })) as TablePage;
  expect(result.total).toBe(
    overview.issues.find((i) => i.id === "title-missing")?.count,
  );
  const detail = (await demo("url_detail", {
    id,
    url: "https://atelier.example/products/arc-chair",
  })) as UrlDetail;
  expect(detail.inlinks.total).toBe(detail.inlinks.rows.length);
  expect(detail.outlinks.total).toBe(detail.outlinks.rows.length);
  expect(
    detail.inlinks.rows.every(
      (row) => row[1] === "https://atelier.example/products/arc-chair",
    ),
  ).toBe(true);
});

it("exports every filtered demo row with the displayed columns and sort, beyond the current page", async () => {
  const [crawl] = await demo("list_crawls", {}) as CrawlMeta[];
  const request: PageRequest = { tab: "internal", search: "products", issue_id: "", status_class: 2, indexability: "", sort: "address", descending: true, offset: 2, limit: 1 };
  const current = await demo("crawl_page", { id: crawl!.id, request }) as TablePage;
  const result = await exportDemoTable(crawl!.id, "internal", { request, columns: ["title", "address"] });
  expect(result.rows.length).toBeGreaterThan(current.rows.length);
  expect(result.columns.map((column) => column.name)).toEqual(["title", "address"]);
  const urls = result.rows.map((row) => String(row[1]));
  expect(urls.every((url) => url.includes("products"))).toBe(true);
  expect(urls).toEqual([...urls].sort((a, b) => b.localeCompare(a)));
  await expect(exportDemoTable(crawl!.id, "internal", { request, columns: ["not_a_column"] })).rejects.toThrow("column");
});

it("keeps exact link scopes, counterpart search and destination status consistent with export", async () => {
  const [crawl] = await demo("list_crawls", {}) as CrawlMeta[];
  const id = crawl!.id;
  const request: PageRequest = { tab: "links", search: "", issue_id: "", status_class: 0, indexability: "", sort: "destination", descending: false, offset: 0, limit: 250 };
  const all = await demo("crawl_page", { id, request }) as TablePage;
  const broken = all.rows.find((row) => row[6] === 404)!;
  expect(broken).toBeDefined();
  const incoming: PageRequest = { ...request, link_scope: { url: String(broken[1]), direction: "incoming" }, status_class: 4, search: String(broken[0]).toUpperCase() };
  const result = await demo("crawl_page", { id, request: incoming }) as TablePage;
  expect(result.rows).toEqual([broken]);
  const outgoing: PageRequest = { ...request, link_scope: { url: String(broken[0]), direction: "outgoing" }, offset: 2, limit: 1 };
  const exported = await exportDemoTable(id, "links", { request: outgoing, columns: ["destination", "destination_status"] });
  const expected = all.rows.filter((row) => row[0] === broken[0]).map((row) => [row[1], row[6]]);
  expect(exported.rows).toEqual(expected);
  expect(exported.rows.length).toBeGreaterThan(outgoing.limit);
  const destinationSearch = await demo("crawl_page", { id, request: { ...outgoing, offset: 0, search: String(broken[1]), status_class: 4 } }) as TablePage;
  expect(destinationSearch.rows).toEqual([broken]);
  const unknownScope = await demo("crawl_page", { id, request: { ...incoming, link_scope: { url: "' OR true --", direction: "incoming" } } }) as TablePage;
  expect(unknownScope.total).toBe(0);
  await expect(demo("crawl_page", { id, request: { ...incoming, tab: "internal" } })).rejects.toThrow("link scope");
});

it("filters broken target evidence and counterpart classes consistently across pagination and export", async () => {
  const [crawl] = await demo("list_crawls", {}) as CrawlMeta[];
  const id = crawl!.id;
  const request: PageRequest = { tab: "links", search: "", issue_id: "links-to-broken", link_class: "internal", status_class: 0, indexability: "", sort: "source", descending: false, offset: 0, limit: 1 };
  const page = await demo("crawl_page", { id, request }) as TablePage;
  expect(page.total).toBeGreaterThan(1);
  expect(page.link_summary!.sources).toBeGreaterThan(0);
  const exported = await exportDemoTable(id, "links", { request, columns: ["source", "destination", "destination_status"] });
  expect(exported.total).toBe(page.total);
  expect(exported.rows.every(row => Number(row[2]) >= 400)).toBe(true);
  expect(page.link_summary!.sources).toBe(new Set(exported.rows.map(row => row[0])).size);
  expect(page.link_summary!.destinations).toBe(new Set(exported.rows.map(row => row[1])).size);
  const external = await demo("crawl_page", { id, request: { ...request, issue_id: "", link_class: "external", link_scope: { url: String(exported.rows[0]![1]), direction: "incoming" } } }) as TablePage;
  expect(external.total).toBe(0);
  await expect(demo("crawl_page", { id, request: { ...request, tab: "internal" } })).rejects.toThrow("classification");
});

it("returns stored demo redirect steps and leaves unknown URLs inconclusive", async () => {
  const [crawl] = await demo("list_crawls", {}) as CrawlMeta[];
  const trace = await demo("url_redirects", { id: crawl!.id, url: "https://atelier.example/old-collection" }) as import("./types").RedirectTrace;
  expect(trace.outcome).toBe("complete");
  expect(trace.hops.map((hop) => hop.status)).toEqual([301,200]);
  expect(trace.hops[0]!.location).toBe(trace.hops[1]!.url);
  const unknown = await demo("url_redirects", { id: crawl!.id, url: "https://outside.example/" }) as import("./types").RedirectTrace;
  expect(unknown.outcome).toBe("unchecked");
  expect(unknown.hops[0]!.recorded).toBe(false);
});
it("keeps demo duplicate summaries, exact URL scopes and complete exports consistent", async () => {
  const [crawl] = await demo("list_crawls",{}) as CrawlMeta[];
  const id=crawl!.id, url="https://atelier.example/collections/lighting";
  const groups=await demo("url_duplicate_groups",{id,url}) as import("./types").DuplicateGroup[];
  const title=groups.find((group)=>group.kind==="title")!;
  expect(title.count).toBeGreaterThan(1);
  const request: PageRequest={tab:"internal",search:"",status_class:2,indexability:"",issue_id:"",sort:"address",descending:true,offset:1,limit:1,duplicate_scope:{kind:"title",url}};
  const page=await demo("crawl_page",{id,request}) as TablePage;
  expect(page.total).toBe(title.count);
  const all=await exportDemoTable(id,"internal",{request,columns:["address","title"]});
  expect(all.rows).toHaveLength(title.count);
  expect(all.rows.every((row)=>row[1]===title.value)).toBe(true);
  const unknown=await demo("crawl_page",{id,request:{...request,duplicate_scope:{kind:"title",url:"' OR true --"}}}) as TablePage;
  expect(unknown.total).toBe(0);
  await expect(demo("crawl_page",{id,request:{...request,tab:"links"}})).rejects.toThrow("duplicate scope");
});

it("compares demo snapshots with reversible membership and literal filters",async()=>{
  const records=await demo("list_crawls",{}) as CrawlMeta[];
  const current=records.find(c=>c.id==="demo-atelier")!,baseline=records.find(c=>c.id==="demo-atelier-previous")!;
  const summary=await demo("comparison_summary",{baseline:baseline.id,current:current.id,internalOnly:true}) as {new:number;missing:number;changed:number;unchanged:number};
  expect(summary).toMatchObject({new:3,missing:1,changed:3,unchanged:90});
  const reverse=await demo("comparison_summary",{baseline:current.id,current:baseline.id,internalOnly:true}) as typeof summary;
  expect(reverse.new).toBe(summary.missing);expect(reverse.missing).toBe(summary.new);
  const request={filter:"changed",field:"title",search:"",internal_only:true,offset:0,limit:250};
  const page=await demo("comparison_page",{baseline:baseline.id,current:current.id,request}) as TablePage;
  expect(page.total).toBe(1);expect(page.rows[0]?.[7]).toBe("Atelier — Previous page title");
  expect((await demo("comparison_page",{baseline:baseline.id,current:current.id,request:{...request,search:"' OR true --"}}) as TablePage).total).toBe(0);
});

it("previews sitemap eligibility with disjoint exclusions and bounded sample records",async()=>{
  const plan=await demo("sitemap_plan",{id:"demo-atelier",selection:{origin:""}}) as import("./types").SitemapPlan;
  expect(plan.origin).toBe("https://atelier.example");
  expect(plan.scanned).toBe(96);expect(plan.included).toBeGreaterThan(0);
  expect(plan.included+plan.excluded.reduce((sum,row)=>sum+row.count,0)).toBe(plan.scanned);
  expect(plan.sample.length).toBe(Math.min(100,plan.included));
  expect(plan.sample.every(url=>url.startsWith(plan.origin+"/"))).toBe(true);
  await expect(demo("sitemap_plan",{id:"demo-atelier",selection:{origin:"https://atelier.example/path"}})).rejects.toThrow();
  const other=await demo("sitemap_plan",{id:"demo-atelier",selection:{origin:"https://other.example"}}) as import("./types").SitemapPlan;
  expect(other.included).toBe(0);expect(other.excluded.find(row=>row.reason==="other-origin")?.count).toBe(plan.included);
});

it("compares stored demo finding pairs independently of URL-field changes and preserves scope membership",async()=>{
  const args={baseline:"demo-atelier-previous",current:"demo-atelier",internalOnly:true};
  const summary=await demo("finding_comparison_summary",args) as import("./types").FindingComparisonSummary;
  const c=summary.counts;
  expect(c.total).toBe(c.introduced+c.not_detected+c.persisting+c.new_url+c.missing_url+c.unverified);
  const reverse=await demo("finding_comparison_summary",{...args,baseline:args.current,current:args.baseline}) as typeof summary;
  expect(reverse.counts.new_url).toBe(c.missing_url);expect(reverse.counts.missing_url).toBe(c.new_url);
  const request={change:"not-detected",issue_id:"internal-client-error",priority:"high",search:"collections",internal_only:true,offset:0,limit:250};
  const page=await demo("finding_comparison_page",{...args,request}) as TablePage;
  expect(page.total).toBe(1);expect(page.rows[0]?.[6]).toBe(true);expect(page.rows[0]?.[7]).toBe(false);
  expect((await demo("finding_comparison_page",{...args,request:{...request,search:"' OR true --"}}) as TablePage).total).toBe(0);
});

it("makes comparison indicators agree with all state pages after search, rule and priority filters",async()=>{
  const args={baseline:"demo-atelier-previous",current:"demo-atelier",internalOnly:true};
  for(const filters of [{issue_id:"internal-client-error",priority:"high",search:"collections"},{issue_id:"description-missing",priority:"low",search:""},{issue_id:"title-missing",priority:"medium",search:""},{issue_id:"",priority:"",search:"' OR true --"}]) {
    const request={change:"persisting" as const,...filters,internal_only:true,offset:9999,limit:1};
    const summary=await demo("finding_comparison_summary",{...args,request}) as import("./types").FindingComparisonSummary;
    for(const [change,key] of [["introduced","introduced"],["not-detected","not_detected"],["persisting","persisting"],["new-url","new_url"],["missing-url","missing_url"],["unverified","unverified"]] as const) {
      const page=await demo("finding_comparison_page",{...args,request:{...request,change,offset:0,limit:250}}) as TablePage;
      expect(page.total).toBe(summary.counts[key]);
    }
    const all=await demo("finding_comparison_page",{...args,request:{...request,change:"all",offset:0,limit:1000}}) as TablePage;
    expect(all.total).toBe(summary.counts.total);expect(all.rows.filter(r=>r[6]===true).length).toBe(summary.baseline);expect(all.rows.filter(r=>r[7]===true).length).toBe(summary.current);
  }
  for(const filter of [{field:"title",search:"products"},{field:"",search:"retired"},{field:"title",search:"retired"},{field:"",search:"' OR true --"}]) {
    const request={...filter,filter:"unchanged",internal_only:true,offset:500,limit:1};
    const summary=await demo("comparison_summary",{...args,request}) as import("./types").ComparisonSummary;
    for(const state of ["changed","unchanged","new","missing"] as const)expect((await demo("comparison_page",{...args,request:{...request,filter:state,offset:0,limit:1000}}) as TablePage).total).toBe(summary[state]);
  }
});
