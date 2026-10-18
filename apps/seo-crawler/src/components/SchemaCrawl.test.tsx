import {renderToStaticMarkup} from "react-dom/server";
import {expect,it} from "vitest";
import {defaultConfig,type SeoScore,type SchemaReport} from "../api/types";
import {SchemaCrawlSettings} from "./SchemaCrawlSettings";
import {SchemaCrawlSummary} from "./SchemaCrawlSummary";
import {SavedSchemaResult} from "./SavedSchemaResult";
import {SeoScoreCard} from "./SeoScoreCard";
import {seoActionPlan} from "../lib/seoActionPlan";
import {NewCrawl} from "./NewCrawl";
import {SeoRemediationContent} from "./SeoRemediation";
import {issueById} from "../catalog";

it("exposes opt-in Schema scoring in new crawls and retains a saved enabled choice",()=>{
 const initial=renderToStaticMarkup(<SchemaCrawlSettings config={defaultConfig} onChange={()=>{}}/>);
 expect(initial).toContain("Schema / zengin sonuç alanlarını tara ve puanla");expect(initial).not.toContain('checked=""');
 const enabled=renderToStaticMarkup(<NewCrawl config={{...defaultConfig,schema_validation:true}} initialUrl="https://example.com/" onClose={()=>{}} onStart={async()=>{}}/>);
 expect(enabled).toContain('role="switch" checked=""');expect(enabled).toContain("Schema yokluğu ve önerilen alanlar puan düşürmez");
});

const score:SeoScore={version:5,value:80,urls:1,affected_urls:1,analyzed_html:1,limited_urls:0,unknown_findings:0,deductions:[{id:"schema-required-invalid",count:1,points:20}],categories:[{id:"structured-data",value:80,points:20,urls:1,affected_urls:1,checks:5,deductions:[{id:"schema-required-invalid",count:1,points:20}]}],schema_validation:{checked_urls:4,scored_urls:1,error_urls:1,warning_urls:1,missing_urls:1,unverified_urls:1}};
it("shows the v5 score, schema coverage and exact impact in the existing action plan",()=>{
 const html=renderToStaticMarkup(<SchemaCrawlSummary score={score} onIssue={()=>{}}/>);
 expect(html).toContain("−20 puan");expect(html).toContain("Schema bulunmayan URL");expect(html).toContain("Kısmi / doğrulanamayan URL");
 expect(seoActionPlan(score)[0]?.rule.id).toBe("schema-required-invalid");
 const meta={id:"test",name:"test",config:defaultConfig,status:"completed",progress:{},limit_reached:false} as Parameters<typeof SeoScoreCard>[0]["meta"];
 expect(renderToStaticMarkup(<SeoScoreCard score={score} meta={meta} onIssue={()=>{}} onIssues={()=>{}}/>)).toContain('aria-valuenow="80"');
});

it("renders saved JSON paths and corrections safely, without treating absent evidence as a pass",()=>{
 const report:SchemaReport={rule_version:"fixture",limited:false,jsonld_blocks:1,items:[{block:1,format:"jsonld",types:["Product"],path:"",profile:"Product snippet",status:"error",issues:[{severity:"error",code:"required",path:"/name",current:'"<script>test</script>"',expected:"text",example:'"Example"',documentation:"https://developers.google.com/search/docs/appearance/structured-data/product-snippet"}]}]};
 const facts={columns:[],rows:[],total:0};
 const html=renderToStaticMarkup(<SavedSchemaResult url="https://example.com/" facts={facts} report={report}/>);
 expect(html).toContain("Kayıtlı tarama kanıtı");expect(html).toContain("/name");expect(html).toContain("&lt;script&gt;");expect(html).not.toContain("<script>");
 const remedy=renderToStaticMarkup(<SeoRemediationContent rule={issueById.get("schema-required-invalid")!} detail={{urls:facts,selected_url:"https://example.com/",facts,evidence:facts,evidence_kind:"structured-data",schema_report:report}}/>);
 expect(remedy).toContain("/name");expect(remedy).not.toContain("güvenilir bir kod örneği üretilemiyor");
 const missing=renderToStaticMarkup(<SavedSchemaResult url="https://example.com/" facts={facts} error="incomplete-document"/>);
 expect(missing).toContain("puanlama yapılmadı");expect(missing).not.toContain("Bu öğe için uygulanan alan kontrollerinde sorun bulunmadı");
});
