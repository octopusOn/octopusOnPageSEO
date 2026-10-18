import {renderToStaticMarkup} from "react-dom/server";
import {expect,it,vi} from "vitest";
vi.mock("../api/client",()=>({api:{},errorText:String,isDemo:false}));
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {findingIdentity,findingSummaryRequest} from "../lib/findingComparison";
import {FindingComparison,FindingObservation} from "./FindingComparison";
import type {Cell,TablePage,FindingComparisonRequest,FindingComparisonSummary} from "../api/types";
const columns=["issue_id","finding_change","comparison_reason","before_finding","after_finding"].map(name=>({name,kind:"text"}));
const render=(row:Cell[])=>renderToStaticMarkup(<FindingObservation data={{columns,rows:[row],total:1} as TablePage} row={row}/>);
it("explains a non-comparable lost title finding with its recorded flags and current noindex context",()=>{
  const html=render(["title-missing","unverified","after-not-indexable",true,false]);
  expect(html).toContain("Başlık (title) yok");expect(html).toContain("Doğrulanamayan farklar");
  expect(html).toContain("Bulgu kaydı var");expect(html).toContain("Bulgu kaydı yok");
  expect(html).toContain("indekslenebilir içerik olarak uygun değil");expect(html).not.toContain("Düzeldi");
});
it("preserves unknown historical checks as escaped text without inventing a fix",()=>{
  const html=render(["unknown-<script>","unverified","unknown-rule",true,false]);
  expect(html).toContain("unknown-&lt;script&gt;");expect(html).not.toContain("<script>");expect(html).toContain("güncel katalogda yok");
});
it("distinguishes persistent findings and one-sided URL absence",()=>{
  expect(render(["title-missing","persisting","",true,true])).toContain("iki taramada da kayıtlı");
  expect(render(["title-missing","missing-url","url-only-baseline",true,false])).toContain("yokluğu doğrulanmış bir düzeltme değildir");
});

it("shows which archive lacks hreflang coverage without presenting an SEO fix",()=>{
  for(const [side,label] of [["before","Önceki tarama"],["after","Yeni tarama"]]) {
    const html=render(["hreflang-missing-return","unverified",side+"-hreflang-unrecorded",true,false]);
    expect(html).toContain(label+": Bu taramada HTTP hreflang gözlemleri tam olarak kaydedilmemiş");
    expect(html).toContain("Bulgunun yokluğu bir değişiklik olduğunu göstermez");
    expect(html).toContain("Doğrulanamayan farklar");expect(html).not.toContain("hreflang-unrecorded");
  }
});

it("renders actual capture warnings and explains that indicator filters match table filters",()=>{
  const client=new QueryClient({defaultOptions:{queries:{staleTime:Infinity,retry:false}}});
  const request:FindingComparisonRequest={change:"all",issue_id:"",priority:"",search:"",internal_only:true,offset:0,limit:250};
  const summary:FindingComparisonSummary={counts:{id:"",priority:"",category:"",introduced:100,not_detected:0,persisting:0,new_url:1,missing_url:1,unverified:0,total:102},baseline:1,current:101,rules:[],rule_total:0,warnings:["finding-observations","incomplete-content"]};
  client.setQueryData(["finding-summary",findingIdentity("before","after",findingSummaryRequest(request))],summary);
  client.setQueryData(["finding-page",findingIdentity("before","after",request)],{columns:[],rows:[],total:102});
  const html=renderToStaticMarkup(<QueryClientProvider client={client}><FindingComparison baseline="before" current="after" internalOnly notify={()=>{}}/></QueryClientProvider>);
  expect(html).toContain("Kartlar URL, öncelik ve kontrol filtrelerine uyar");
  expect(html).toContain("İstek/JavaScript hataları veya eksik içerik kaydedilmiş");
  expect(html).not.toContain("Kapsam, ayarlar veya kayıt kalitesi farklı");
  expect(html).toContain('aria-busy="false"');
  expect(html).toMatch(/Yeni saptanan<\/span><strong>100<\/strong>/);
});
