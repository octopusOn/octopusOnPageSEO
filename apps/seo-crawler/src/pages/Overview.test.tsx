import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { defaultConfig, type CrawlMeta, type Overview as OverviewData } from "../api/types";
import { Overview } from "./Overview";
import { demoSeoScore } from "../lib/demoSeoScore";

const meta:CrawlMeta={id:"saved",name:"Example",start_urls:["https://example.com/"],created_at_ms:0,finished_at_ms:1,status:"stopped",progress:{crawled:7,queued:3,errors:0,elapsed_ms:1000,discovered:10,in_flight:0,sitemaps:0,paused:false},limit_reached:false,error:null,config:defaultConfig};
const data:OverviewData={internal:5,external:2,indexable:1,html:1,blocked:1,errors:3,redirects:1,average_response_ms:10,issues:[],responses:[{group:"success",count:1},{group:"redirects",count:1},{group:"client-error",count:1},{group:"server-error",count:0},{group:"other-http",count:0},{group:"robots-blocked",count:1},{group:"request-error",count:1},{group:"no-response",count:2}]};
const render=(data:OverviewData)=>renderToStaticMarkup(<Overview meta={meta} data={data} excluded={[]} onIssue={()=>{}} onExplore={()=>{}} onIssues={()=>{}} onResponse={()=>{}}/>);

it("shows exact disjoint counts, including unknown responses, with disabled empty groups",()=>{
  const html=render(data);
  expect(html).toContain('aria-label="HTTP 2xx: 1 URL"');
  expect(html).toContain('aria-label="Yanıt kaydı yok: 2 URL"');
  expect(html).toMatch(/disabled="" aria-label="Sunucu hatası \(5xx\): 0 URL"/);
  expect(html).toContain("JavaScript hataları");
  expect(html).not.toContain("Hata / engelli");
});

it("does not infer successful responses when connected to an older native backend",()=>{
  const html=render({...data,responses:undefined});
  expect(html).toContain("Yanıt dağılımı alınamadı");
  expect(html).not.toContain('aria-label="HTTP 2xx');
});

it("renders empty crawls without non-finite chart values",()=>{
  const html=render({...data,internal:0,external:0,html:0,indexable:0,responses:data.responses!.map((row)=>({...row,count:0}))});
  expect(html).not.toMatch(/NaN|Infinity/);
  expect(html).toContain('aria-label="HTTP 2xx: 0 URL"');
});

it("keeps rare positive response groups visibly above zero percent",()=>{
  expect(render({...data,internal:864,external:0})).toContain("0,1%");
  expect(render({...data,internal:10000,external:0})).toContain("&lt;0,1%");
});

it("shows the saved JavaScript layout separately from HTTP outcomes", () => {
  const html = renderToStaticMarkup(<Overview meta={{...meta,config:{...meta.config,rendering:{...meta.config.rendering,mode:"javascript",viewport:{width:390,height:844,mobile:true}}}}} data={data} excluded={[]} onIssue={()=>{}} onExplore={()=>{}} onIssues={()=>{}} onResponse={()=>{}}/>);
  expect(html).toContain("Rendering görünüm alanı");
  expect(html).toContain("390 × 844 · Mobil");
  expect(html).toContain('aria-label="HTTP 2xx: 1 URL"');
});

it("separates raw scoring exceptions from actionable recommendations", () => {
  const score=demoSeoScore([{rules:["anchor-empty"],analyzed:true,limited:false}]);
  score.excluded_rules=[{id:"links-to-broken",count:260}];
  const html=render({...data,seo_score:score,issues:[{id:"links-to-broken",count:260},{id:"anchor-empty",count:1}]});
  const plan=html.split('aria-label="Öncelikli düzeltmeler"')[1]!.split("</section>")[0]!;
  expect(plan).toContain("Açıklayıcı etiketi olmayan bağlantılar");
  expect(plan).not.toContain("Kırık iç URL");
  expect(html).toContain("Kayıtlı tarama bulguları");
  expect(html).toContain("Doğrulanan canonical bağlamıyla");
  expect(html).toContain("Kırık iç URL");
  expect(html).not.toContain("Önce bunları çözün");
});
