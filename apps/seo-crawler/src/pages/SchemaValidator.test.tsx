import {renderToStaticMarkup} from "react-dom/server";
import {expect,it,vi} from "vitest";
vi.mock("../api/client",()=>({api:{schemaTest:vi.fn()},errorText:String,isDemo:false}));
import {SchemaResult,SchemaValidator} from "./SchemaValidator";
import {defaultConfig,type SchemaTestResult} from "../api/types";

const result:SchemaTestResult={url:"https://example.com/page?q=<script>",status:200,rendered:false,render_error:null,redirects:[],indexability:"indexable",truncated:false,error:null,tested_at_ms:1792310000000,report:{rule_version:"2026-10-18.1",jsonld_blocks:1,limited:false,items:[{block:1,format:"jsonld",path:"/@graph/0",types:["Product"],profile:"Product snippet",status:"error",issues:[{code:"required",severity:"error",path:"/@graph/0/name",current:'"<script>alert(1)</script>"',expected:"text",example:'"Example"',documentation:"https://developers.google.com/search/docs/appearance/structured-data/product-snippet"}]}]}};
it("shows exact paths, observed values and corrections without executing source HTML",()=>{
  const html=renderToStaticMarkup(<SchemaResult result={result}/>);
  expect(html).toContain("/@graph/0/name");
  expect(html).toContain("Mevcut değer");expect(html).toContain("Beklenen / yapılacak düzeltme");
  expect(html).toContain("&lt;script&gt;");expect(html).not.toContain("<script>");
  expect(html).toContain("Google onayı veya zengin sonuç gösterimi garantisi değildir");
});
it("does not confuse an incomplete render with missing Schema or a passed test",()=>{
  const html=renderToStaticMarkup(<SchemaResult result={{...result,render_error:"timeout",truncated:true,report:null}}/>);
  expect(html).toContain("ham HTML başarılı bir render testi sayılmadı");
  expect(html).toContain("eksik belge üzerinden geçerlilik kararı verilmedi");
  expect(html).not.toContain("öğesi bulunamadı");expect(html).not.toContain("schema-stats");
});
it("explains absent Schema and unsupported formats without inventing errors or score deductions",()=>{
  const html=renderToStaticMarkup(<SchemaResult result={{...result,report:{...result.report!,items:[],jsonld_blocks:0}}}/>);
  expect(html).toContain("tek başına bir SEO hatası değildir");
  expect(html).toContain("öneriler ve Schema bulunmaması puan düşürmez");
});
it("shows current FAQ property advice and retirement context without implying score loss",()=>{
  const html=renderToStaticMarkup(<SchemaResult result={{...result,report:{rule_version:"2026-10-18.2",jsonld_blocks:1,limited:false,items:[{block:1,format:"jsonld",path:"",types:["FAQPage"],profile:"FAQPage (Schema.org)",status:"warning",issues:[{severity:"warning",code:"faq-answer-text",path:"/mainEntity/acceptedAnswer/text",current:'"<p></p>"',expected:"text",example:'"Yanıt"',documentation:"https://schema.org/Answer"}]}]}}}/>);
  expect(html).toContain("/mainEntity/acceptedAnswer/text");
  expect(html).toContain("Boş HTML etiketleri yanıt sayılmaz");
  expect(html).toContain("Bu uyarı puan düşürmez");
  expect(html).toContain("7 Mayıs 2026’da kaldırıldı");
  expect(html).toContain("Alan uyarısı / öneri");
  expect(html).not.toContain("önceki kural sürümüyle");
});
it("keeps historical reports explicit instead of suggesting new checks were applied retroactively",()=>{
  const html=renderToStaticMarkup(<SchemaResult saved result={result}/>);
  expect(html).toContain("WebPage / FAQPage kontrolleri geriye dönük uygulanmadı");
  expect(html).toContain("Kayıtlı puan değiştirilmedi");
});
it("opening the form offers an explicit test and never automatically requests a URL",async()=>{
  const html=renderToStaticMarkup(<SchemaValidator initialUrl="https://example.com" config={defaultConfig}/>);
  expect(html).toContain("Yalnızca Test et düğmesine bastığınızda çalışır");
  expect(html).toContain("Site bağlantıları taranmaz");
  const {api}=await import("../api/client");expect(api.schemaTest).not.toHaveBeenCalled();
});
