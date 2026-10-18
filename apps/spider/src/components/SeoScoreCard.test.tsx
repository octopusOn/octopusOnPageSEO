import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { defaultConfig, type CrawlMeta, type SeoScore } from "../api/types";
import { SeoScoreCard } from "./SeoScoreCard";

const meta:CrawlMeta={id:"test",name:"Example",start_urls:[],created_at_ms:0,finished_at_ms:1,status:"completed",progress:{crawled:2,queued:0,errors:0,elapsed_ms:1,discovered:2,in_flight:0,sitemaps:0,paused:false},limit_reached:false,error:null,config:defaultConfig};
const score:SeoScore={version:2,value:85.5,urls:2,affected_urls:1,analyzed_html:2,limited_urls:0,unknown_findings:0,deductions:[{id:"title-missing",count:1,points:14.5}]};
const render=(value:SeoScore|undefined,source=meta)=>renderToStaticMarkup(<SeoScoreCard score={value} meta={source} onIssue={()=>{}} onIssues={()=>{}}/>);

it("shows a real bounded meter, score drivers and a transparent model",()=>{
  const html=render(score);
  expect(html).toContain('role="meter"');expect(html).toContain('aria-valuenow="85.5"');expect(html).toContain("İyi");
  expect(html).toContain("Puanı en çok düşürenler");expect(html).toContain("20, orta 7, düşük 2");expect(html).toContain("Google sıralama puanı değildir");
  expect(html).toContain("bir bulguyu gizlemek bu puanı değiştirmez");
});

it("supports canonical-aware v3 and explains its excluded finding count",()=>{
 const html=render({...score,version:3,excluded_findings:12,canonical_context_findings:12});
 expect(html).toContain('aria-valuenow="85.5"');expect(html).toContain("canonical kümesinde çözüldü");
});

it("never shows an unavailable, unknown-version or empty sample as a perfect score",()=>{
  for(const value of [undefined,{...score,value:null,analyzed_html:0},{...score,version:99}]) {
    const html=render(value);expect(html).toContain("Hesaplanmadı");expect(html).not.toContain('role="meter"');
    expect(html).not.toMatch(/NaN|Infinity/);
  }
  expect(render({...score,value:null,unknown_findings:1})).toContain("tanımadığı kontroller");
});

it("marks stopped, limited, list-mode and incomplete-observation crawls without changing their recorded score",()=>{
  for(const source of [{...meta,status:"stopped" as const},{...meta,limit_reached:true},{...meta,config:{...defaultConfig,mode:"list" as const}}]) {
    const html=render(score,source);expect(html).toContain("Sınırlı değerlendirme");expect(html).toContain('aria-valuenow="85.5"');
  }
  expect(render({...score,limited_urls:1})).toContain("URL’de eksik gözlem");
  expect(render({...score,value:100,affected_urls:0,deductions:[]})).toContain("Puanlanan örneklemde bulgu yok");
});


it("explains observed-label and expected-contact exceptions in v4",()=>{
 const html=render({...score,version:4,excluded_findings:20,anchor_context_findings:12,contact_redirect_findings:8});
 expect(html).toContain('aria-valuenow="85.5"');expect(html).toContain("kayıtlı açıklayıcı etiket");expect(html).toContain("WhatsApp’ın beklenen");
});


it("marks legacy label evidence as unverified without calling it a confirmed fix",()=>{
 const html=render({...score,version:4,unverified_anchor_findings:12});
 expect(html).toContain("tam etiket kanıtı yok");expect(html).toContain("Sınırlı değerlendirme");expect(html).not.toContain("kayıtlı açıklayıcı etiket kanıtıyla çözüldü");
});
