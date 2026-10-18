import { useEffect, useRef, useState } from "react";
import { Braces, ExternalLink, Play, ShieldCheck } from "lucide-react";
import { api, errorText, isDemo } from "../api/client";
import type { CrawlConfig, SchemaIssue, SchemaTestRequest, SchemaTestResult } from "../api/types";
import { useI18n } from "../i18n";
import { Button } from "../components/ui";
import { RenderingSettings } from "../components/RenderingSettings";
import { richResultsUrl, schemaInputValid, schemaSummary, SCHEMA_RULE_VERSION } from "../lib/schema";

export function SchemaValidator({initialUrl,config}:{initialUrl:string;config:CrawlConfig}) {
  const {lang}=useI18n();
  const tr=lang==="tr";
  const [mode,setMode]=useState<SchemaTestRequest["mode"]>("url");
  const [input,setInput]=useState(initialUrl);
  const [renderConfig,setRenderConfig]=useState<CrawlConfig>(()=>({...config,rendering:{...config.rendering,mode:"html"}}));
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [result,setResult]=useState<SchemaTestResult|null>(null);
  const [dirty,setDirty]=useState(false);
  const running=useRef(false);
  useEffect(()=>{setInput(initialUrl);setMode("url");setResult(null);setError("");setDirty(false);},[initialUrl]);
  async function test() {
    if(running.current || !schemaInputValid(input,mode))return;
    running.current=true;setBusy(true);setError("");setResult(null);
    try {setResult(await api.schemaTest({input:input.trim(),mode,rendering:renderConfig.rendering}));setDirty(false);}
    catch(error){setError(errorText(error));}
    finally {setBusy(false);running.current=false;}
  }
  function edit() {setDirty(true);setError("");}
  return <div className="schema-validator">
    <section className="oc-panel schema-form">
      <div className="row"><span className="oc-tile teal"><Braces size={20}/></span><div><h2>{tr?"İsteğe bağlı Schema testi":"On-demand Schema test"}</h2><p>{tr?"Yalnızca Test et düğmesine bastığınızda çalışır. Tarama kayıtlarını ve SEO puanını değiştirmez.":"Runs only when you press Test. Crawl archives and SEO scores are unaffected."}</p></div></div>
      <div className="oc-tabs" role="group" aria-label={tr?"Test kaynağı":"Test source"}>
        {(["url","html","jsonld"] as const).map(value=><button key={value} className={`oc-tab ${mode===value?"active":""}`} aria-pressed={mode===value} disabled={busy} onClick={()=>{setMode(value);setInput(value==="url"?initialUrl:"");setResult(null);edit();}}>{value==="url"?"URL":value==="html"?"HTML":"JSON-LD"}</button>)}
      </div>
      <label className="oc-field">{mode==="url"?(tr?"Test edilecek URL":"URL to test"):(tr?"Test edilecek kod (en fazla 4 MiB)":"Source to test (up to 4 MiB)")}
        {mode==="url"?<input className="oc-input mono" type="url" value={input} disabled={busy} placeholder="https://example.com/page" onChange={e=>{setInput(e.target.value);edit();}} onKeyDown={e=>{if(e.key==="Enter")void test();}}/>:<textarea className="oc-input mono schema-source" rows={9} spellCheck={false} value={input} disabled={busy} placeholder={mode==="jsonld"?'{"@context":"https://schema.org","@type":"Product","name":"…"}':'<script type="application/ld+json">…</script>'} onChange={e=>{setInput(e.target.value);edit();}}/>}
      </label>
      {mode==="url"?<fieldset disabled={busy} className="schema-rendering"><RenderingSettings config={renderConfig} onChange={next=>{setRenderConfig(next);edit();}}/><p className="faint">{tr?"Yalnızca bu belge, en fazla 5 HTTP yönlendirmesi ve robots.txt kontrol edilir. JavaScript seçilirse sayfanın tarayıcı kaynakları da yüklenir. Site bağlantıları taranmaz.":"Checks this document, up to 5 HTTP redirects and robots.txt. JavaScript mode also loads browser resources. Page links are not crawled."}</p></fieldset>:<p className="faint">{tr?"Kod yerel olarak incelenir; scriptler çalıştırılmaz, siteye veya uzak JSON-LD bağlamlarına istek gönderilmez.":"Source is inspected locally: scripts are not executed and websites or remote JSON-LD contexts are not requested."}</p>}
      {isDemo&&<div className="oc-banner info">{tr?"Gerçek doğrulama masaüstü uygulamasında kullanılabilir. Web önizlemesi sonuç üretmez.":"Actual validation is available in the desktop app. This web preview does not fabricate results."}</div>}
      {error&&<div className="oc-banner error" role="alert">{error}</div>}
      <div className="row between"><span className="faint"><ShieldCheck size={14}/> {tr?"API anahtarı gerekmez · otomatik toplu test yok":"No API key · no automatic batch tests"}</span><Button primary busy={busy} disabled={isDemo||!schemaInputValid(input,mode)} onClick={()=>void test()}><Play size={15}/>{busy?(tr?"Test ediliyor…":"Testing…"):(tr?"Test et":"Test")}</Button></div>
      {busy&&<p role="status">{tr?"Belge yükleniyor ve seçilen kontroller çalıştırılıyor…":"Loading the document and running selected checks…"}</p>}
    </section>
    {result&&<>{dirty&&<div className="oc-banner warning">{tr?"Girdi değişti. Aşağıdaki sonuç önceki teste aittir; yeniden test edin.":"Input changed. The result below belongs to the previous test; run it again."}</div>}<SchemaResult result={result}/></>}
  </div>;
}

const statusText={tr:{error:"Hatalı öğe",warning:"Alan uyarısı / öneri",checked:"Uygulanan kontroller geçti",unchecked:"Doğrulanmadı"},en:{error:"Item with errors",warning:"Property warnings / advice",checked:"Implemented checks passed",unchecked:"Unverified"}};
const messages:Record<string,[string,string]>={
  required:["Zorunlu alan eksik veya beklenen biçimde değil.","Required property is missing or has an unexpected value."],
  recommended:["Önerilen alanı içeriğinize uygunsa ekleyin. Bu alan zorunlu değildir.","Add the recommended property if applicable. It is not required."],
  "property-value":["Verilen alanın değeri beklenen biçimde değil. Bu yerel alan uyarısı puan düşürmez.","The supplied property has an unexpected value. This local property warning does not deduct score."],
  "faq-questions":["FAQPage soru içeriğini mainEntity içindeki Question nesnesi veya dizisiyle tanımlayın. Bu yapısal öneri puan düşürmez.","Describe FAQ questions using a Question object or array in mainEntity. This structural advice does not deduct score."],
  "faq-question-text":["Sorunun name alanında boş olmayan soru metnini belirtin. Bu uyarı puan düşürmez.","Provide nonempty question text in name. This warning does not deduct score."],
  "faq-answer":["Sorunun yanıtını acceptedAnswer veya suggestedAnswer içinde Answer olarak tanımlayın. Bu uyarı puan düşürmez.","Describe the answer using Answer in acceptedAnswer or suggestedAnswer. This warning does not deduct score."],
  "faq-answer-text":["Yanıtın text alanında okunabilir yanıt metni bulunmalı. Boş HTML etiketleri yanıt sayılmaz. Bu uyarı puan düşürmez.","The answer's text should contain readable answer text. Empty HTML tags do not count as an answer. This warning does not deduct score."],
  "json-syntax":["JSON-LD ayrıştırılamadı veya kök değer nesne/dizi değil. Sözdizimini düzeltin.","JSON-LD cannot be parsed or its root is not an object/array. Fix the syntax."],
  "min-items":["Breadcrumb en az iki ListItem içermeli.","Breadcrumb must contain at least two ListItems."],
  "product-alternative":["Product için offers, review veya aggregateRating alanlarından en az biri gerekir. Gerçek dışı değerlendirme eklemeyin.","Product requires offers, review or aggregateRating. Never invent reviews."],
  "unresolved-reference":["Bu referans veya gösterim güvenle çözümlenemedi. Eksik alan olduğu sonucuna varılmadı; elle doğrulayın.","This reference or representation could not be resolved safely. Missing properties are not inferred; verify manually."],
  "unsupported-context":["Bağlam yok veya bu sürümün desteklemediği bir JSON-LD bağlamı/gösterimi kullanılıyor. Alan kontrolleri uygulanmadı.","Context is absent or uses a JSON-LD context/representation this version cannot interpret. Property checks were not applied."],
  "unsupported-format":["Bu biçim tespit edildi. Microdata ve RDFa alan doğrulaması bu sürümde uygulanmıyor; geçerli kabul edilmedi.","Format detected. Microdata and RDFa property validation is not implemented in this version; validity is not assumed."],
  "unsupported-profile":["Tür tespit edildi; bu türe özel Rich Results kuralları uygulanmıyor. Destek dışı olması hata değildir.","Type detected; this type's Rich Results rules are not implemented. Unsupported does not mean invalid."],
  "review-scope":["Değerlendirme/puanlama dalının ayrıntılı kuralları ve içerik politikası elle doğrulanmalı.","Detailed review/rating rules and content policies require manual verification."],
};
const expected:Record<string,[string,string]>={text:["Boş olmayan metin","Nonempty text"],value:["İçeriğe uygun değer","Applicable value"],"postal-address":["PostalAddress nesnesi","PostalAddress object"],"positive-integer":["1 veya daha büyük tam sayı","Integer of at least 1"],"breadcrumb-list":["En az iki ListItem içeren dizi","Array with at least two ListItems"],url:["Sayfa URL’si veya @id","Page URL or @id"],price:["Sıfır veya daha büyük fiyat (ondalık ayırıcı: nokta)","Nonnegative price (decimal separator: dot)"],currency:["Para birimi (ör. TRY)","Currency (e.g. TRY)"],"product-alternative":["offers / review / aggregateRating","offers / review / aggregateRating"],"json-object":["Geçerli JSON nesnesi veya dizisi","Valid JSON object or array"],language:["Dil metni veya Language nesnesi","Language text or Language object"],entity:["İlgili öğe veya @id referansı","Related entity or @id reference"],"breadcrumb-value":["Breadcrumb metni veya BreadcrumbList / @id","Breadcrumb text or BreadcrumbList / @id"],questions:["Question nesnesi veya Question dizisi","Question object or array"],answer:["Yanıt metni içeren Answer nesnesi","Answer object containing answer text"],manual:["Elle doğrulama","Manual verification"]};

function Finding({finding,tr}:{finding:SchemaIssue;tr:boolean}) {
  const index=tr?0:1;
  return <li className={`schema-finding ${finding.severity}`}><div className="row between"><b>{messages[finding.code]?.[index]??finding.code}</b><a href={finding.documentation} target="_blank" rel="noreferrer">{tr?"Kılavuz":"Guide"}<ExternalLink size={12}/></a></div>
    <code>{finding.path||"/"}</code><div className="schema-evidence"><div><small>{tr?"Mevcut değer":"Current value"}</small><pre>{finding.current??(tr?"Kaynakta yok":"Absent from source")}</pre></div><div><small>{tr?"Beklenen / yapılacak düzeltme":"Expected / correction"}</small><p>{expected[finding.expected]?.[index]??finding.expected}</p>{finding.example&&<><small>{tr?"Temsili değer · içeriğinize uyarlayın":"Illustrative value · adapt to your content"}</small><pre>{finding.example}</pre></>}</div></div>
  </li>;
}

export function SchemaResult({result,saved=false}:{result:SchemaTestResult;saved?:boolean}) {
  const {lang}=useI18n();const tr=lang==="tr";
  const labels=statusText[tr?"tr":"en"];
  const [filter,setFilter]=useState("all");
  const report=result.report;
  const currentRules=report?.rule_version===SCHEMA_RULE_VERSION;
  const summary=report?schemaSummary(report):null;
  const google=result.url?richResultsUrl(result.url):null;
  const visible=report?.items.filter(item=>filter==="all"||item.status===filter)??[];
  return <section className="oc-panel schema-result" aria-label={tr?"Schema test sonucu":"Schema test result"}>
    <header className="row between"><div><h2>{saved?(tr?"Tarama Schema sonucu":"Crawl Schema result"):(tr?"Test sonucu":"Test result")}</h2><p>{new Date(result.tested_at_ms).toLocaleString(tr?"tr-TR":"en-US")}{report&&` · ${tr?"Kural sürümü":"Rule version"}: ${report.rule_version}`}</p></div>{google&&<a className="oc-btn small" href={google} target="_blank" rel="noreferrer">Google Rich Results Test<ExternalLink size={14}/></a>}</header>
    {result.url&&<div className="schema-observation"><code>{result.url}</code><span>HTTP {result.status??"—"} · {result.rendered?"JavaScript DOM":"HTML"} · {result.indexability??"—"}</span>{result.redirects.length>0&&<p>{tr?"Yönlendirmeler":"Redirects"}: {result.redirects.join(" → ")}</p>}</div>}
    {result.error&&<div className="oc-banner error" role="alert">{tr?"Test tamamlanamadı":"Test could not complete"}: {result.error}</div>}
    {result.render_error&&<div className="oc-banner warning">{tr?"JavaScript çalıştırılamadı; ham HTML başarılı bir render testi sayılmadı":"JavaScript failed; fallback HTML is not reported as a successful render test"}: {result.render_error}</div>}
    {result.truncated&&<div className="oc-banner warning">{tr?"Kaynak veya DOM kesildi; eksik belge üzerinden geçerlilik kararı verilmedi.":"Source or DOM was truncated; validity was not inferred from an incomplete document."}</div>}
    <div className="oc-banner info">{tr?"Yerel ön kontrol: JSON-LD sözdizimi ve seçili alan kuralları incelenir. Google onayı veya zengin sonuç gösterimi garantisi değildir. Görünür içerikle uyum, tüm değer türleri, politika, erişilebilirlik ve destek dışı alanlar ayrıca doğrulanmalıdır.":"Local preflight: JSON-LD syntax and selected property rules are inspected. This is not Google approval or a rich-result guarantee. Visible-content consistency, all value types, policy, access and unsupported fields require additional verification."}</div>
    {report&&<><p className="faint">{tr?"Alan kontrolü kapsamı: Article / NewsArticle / BlogPosting, BreadcrumbList, Product snippet (Offer / AggregateOffer fiyat alanları), Organization, LocalBusiness, WebPage ve FAQPage. WebPage için yalnızca verilen alanların temel biçimi; FAQPage için soru, yanıt ve metin yapısı kontrol edilir. ListItem / Question / Answer alt öğeleri incelendiyse üst öğenin raporunda gösterilir. Diğer türler ve biçimler doğrulanmadı olarak kalır. WebPage / FAQPage alan uyarıları, öneriler ve Schema bulunmaması puan düşürmez.":"Property checks: Article / NewsArticle / BlogPosting, BreadcrumbList, Product snippets (Offer / AggregateOffer price fields), Organization, LocalBusiness, WebPage and FAQPage. WebPage checks basic shapes of supplied properties; FAQPage checks question, answer and text structure. Checked ListItem / Question / Answer components are reported within their parent. Other types and formats remain unverified. WebPage / FAQPage property warnings, recommendations and absent Schema do not deduct score."}</p>
      {!currentRules&&<p className="oc-banner info">{tr?"Bu rapor önceki kural sürümüyle kaydedildi; WebPage / FAQPage kontrolleri geriye dönük uygulanmadı. Güncel kontrol için URL’yi yeniden test edin veya Schema açık yeni bir tarama başlatın. Kayıtlı puan değiştirilmedi.":"This report was saved with an earlier rule version; WebPage / FAQPage checks were not applied retroactively. Retest the URL or start a new Schema-enabled crawl for current checks. The saved score is unchanged."}</p>}
      {currentRules&&report.items.some(item=>item.types.includes("FAQPage"))&&<p className="faint">{tr?"FAQPage Schema.org yapısı olarak incelenir; Google FAQ zengin sonuç özelliği 7 Mayıs 2026’da kaldırıldı. Bu özellik için uygunluk şartı veya puan kesintisi uygulanmaz.":"FAQPage is checked as Schema.org structure; Google retired FAQ rich results on May 7, 2026. No eligibility requirements or score deductions are applied for that feature."} <a href="https://developers.google.com/search/updates#may-2026" target="_blank" rel="noreferrer">{tr?"Google duyurusu":"Google announcement"}</a></p>}
      {report.limited&&<div className="oc-banner warning">{tr?"İşleme sınırına ulaşıldı; rapor kısmidir.":"Processing limit reached; report is partial."}</div>}
      <div className="schema-stats">{summary&&Object.entries(summary).map(([key,count])=><button key={key} className={`schema-stat ${key} ${filter===key?"active":""}`} aria-pressed={filter===key} onClick={()=>setFilter(filter===key?"all":key)}><strong>{count}</strong><span>{labels[key as keyof typeof labels]}</span></button>)}</div>
      <div className="row between"><span>{report.jsonld_blocks} JSON-LD · {report.items.length} {tr?"öğe / biçim kaydı":"item / format records"}</span><Button small onClick={()=>setFilter("all")}>{tr?"Tümünü göster":"Show all"}</Button></div>
      {!report.items.length&&<p className="schema-empty">{tr?"Bu kaynakta tür içeren Schema öğesi bulunamadı. Bu, tek başına bir SEO hatası değildir.":"No typed Schema items were found in this source. This alone is not an SEO error."}</p>}
      {report.items.length>0&&!visible.length&&<p className="schema-empty">{tr?"Bu filtrede öğe yok.":"No items match this filter."}</p>}
      {visible.map((item,index)=><article className="schema-item" key={`${item.format}:${item.block}:${item.path}:${index}`}><header className="row between"><div><h3>{item.types.join(" · ")||(tr?"JSON-LD bloğu":"JSON-LD block")}</h3><small>{item.format.toUpperCase()} · {tr?"Blok":"Block"} {item.block||"—"} · <code>{item.path||"/"}</code>{item.profile&&` · ${item.profile}`}</small></div><span className={`oc-badge ${item.status==="error"?"error":item.status==="checked"?"good":"warning"}`}>{labels[item.status]}</span></header>{item.issues.length?<ul className="schema-findings">{item.issues.map((finding,index)=><Finding key={`${finding.path}:${finding.code}:${index}`} finding={finding} tr={tr}/>)}</ul>:<p>{tr?"Bu öğe için uygulanan alan kontrollerinde sorun bulunmadı. Tam uygunluk ayrıca doğrulanmalıdır.":"Implemented property checks found no issues for this item. Full eligibility requires further verification."}</p>}</article>)}
    </>}
  </section>;
}
