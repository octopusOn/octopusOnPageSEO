import {expect,it} from "vitest";
import {defaultConfig,type CrawlMeta,type SitemapPlan} from "../api/types";
import {canPlanSitemap,sitemapExportReady,sitemapXml} from "./sitemap";
const meta:CrawlMeta={id:"test",name:"example.com",start_urls:[],created_at_ms:0,finished_at_ms:1,status:"completed",progress:{crawled:0,queued:0,errors:0,elapsed_ms:0,discovered:0,in_flight:0,sitemaps:0,paused:false},limit_reached:false,error:null,config:defaultConfig};
const plan:SitemapPlan={origin:"https://example.com",origins:[],scanned:1,included:1,excluded:[],sample:[],parts:1,xml_bytes:100,warnings:[]};
it("requires verified finished metadata and excludes finalizing active jobs",()=>{
  expect(canPlanSitemap(meta,false,"ready")).toBe(true);
  for(const status of ["loading","unavailable"] as const) expect(canPlanSitemap(meta,false,status)).toBe(false);
  expect(canPlanSitemap(meta,true,"ready")).toBe(false);
  expect(canPlanSitemap({...meta,status:"running"},false,"ready")).toBe(false);
  expect(canPlanSitemap({...meta,status:"stopped"},false,"ready")).toBe(true);
});
it("disables export for stale, empty, erroneous or unscoped plans",()=>{
  expect(sitemapExportReady(plan,false,false)).toBe(true);
  expect(sitemapExportReady(plan,true,false)).toBe(false);
  expect(sitemapExportReady(plan,false,true)).toBe(false);
  expect(sitemapExportReady(undefined,false,false)).toBe(false);
  expect(sitemapExportReady({...plan,included:0},false,false)).toBe(false);
  expect(sitemapExportReady({...plan,origin:""},false,false)).toBe(false);
});
it("escapes XML entities without changing URL identity or fabricating optional dates",()=>{
  const xml=sitemapXml(['https://example.com/?q=&<>"\'']);
  expect(xml).toContain('?q=&amp;&lt;&gt;&quot;&apos;');
  expect(xml).toContain('xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"');
  expect(xml).not.toMatch(/lastmod|priority|changefreq/);
});
