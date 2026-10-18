import type { Cell, RemediationDetail, TablePage } from "../api/types";
import type { Issue } from "../catalog";
import { number, type Lang } from "../i18n";

export type SolutionGuide = {observations:{label:string;value:string}[];steps:string[];before:string|null;after:string|null;validation:string[];warning:string|null;sources:{title:string;url:string}[]};
export function record(table:TablePage):Record<string,Cell> {
  return Object.fromEntries(table.columns.map((column,index)=>[column.name,table.rows[0]?.[index]??null]));
}
const escape=(text:string)=>text.replaceAll("&","&amp;").replaceAll('"',"&quot;").replaceAll("<","&lt;").replaceAll(">","&gt;");
const shell=(text:string)=>`'${text.replaceAll("'",`'\\''`)}'`;
export function solutionGuide(rule:Issue,detail:RemediationDetail,lang:Lang):SolutionGuide {
  const text=(tr:string,en:string)=>lang==="tr"?tr:en;
  const f=record(detail.facts), e=record(detail.evidence), url=detail.selected_url??"";
  const string=(value:Cell|undefined)=>typeof value==="string"?value:"";
  const show=(value:Cell|undefined)=>value===null||value===undefined?text("Kaydedilmemiş / boş","Not recorded / empty"):value===""?text("Boş metin","Empty text"):typeof value==="boolean"?text(value?"Evet":"Hayır",value?"Yes":"No"):typeof value==="number"?number(value,lang):String(value);
  const field=(name:string,tr:string,en:string)=>({label:text(tr,en),value:show(f[name])});
  const observations=[field("status","HTTP durumu","HTTP status"),field("indexability","İndekslenebilirlik","Indexability")];
  const add=(name:string,tr:string,en:string)=>observations.push(field(name,tr,en));
  const guide:SolutionGuide={observations,steps:[rule[lang].fix,text("Değişikliği sayfanın ortak şablonunda mı, yalnızca bu URL’de mi yapmak gerektiğini belirleyin.","Determine whether the change belongs in a shared template or only on this URL.")],before:null,after:null,validation:[text("Aynı tarama ayarlarıyla yeniden tarayın; bu URL’de ilgili bulgunun kaybolduğunu ve başka bir sorun oluşmadığını doğrulayın.","Recrawl with the same settings; verify this URL no longer has the finding and that the change introduces no new issue.")],warning:null,sources:[]};
  const target=string(e.destination)||string(e.target);
  if(rule.id.startsWith("hreflang-")) {
    add("lang","Sayfa dili","Page language");add("canonical","HTML canonical","HTML canonical");add("header_canonical","HTTP canonical","HTTP canonical");
    guide.steps=[text("Önce bu URL ile alternatif hedefin gerçekten aynı içeriğin farklı dil sürümleri olduğunu doğrulayın. Canonical ve indekslenebilirlik tercihini netleştirin.","First confirm the source and target are language versions of the same content. Resolve the canonical and indexability policy."),rule[lang].fix,text("Her canonical dil sürümünün head bölümünde kendisini ve mevcut eşdeğer sürümleri tam URL ile listeleyin. Yalnızca bir dilde yayımlanan içerik için çeviri veya ilgisiz bir alternatif oluşturmayın.","In each canonical language version’s head, list itself and the existing equivalent versions using absolute URLs. Content published in one language does not require translations or unrelated alternates."),text("Bu çift dışındaki mevcut eşdeğer dilleri de tam kümeye ekleyin. Dil kodlarını ve yönlendirme olmayan nihai hedefleri kontrol edin.","Include existing equivalent languages beyond this pair in the full cluster. Check language codes and final, nonredirecting targets.")];
    const srcLang=/^[a-z]{2,3}(?:-[a-z0-9]{2,8})*$/i.test(string(f.lang))?string(f.lang):"SOURCE_LANGUAGE";
    const targetLang=string(e.lang)||"TARGET_LANGUAGE";
    if(target) {
      guide.before=`<link rel="alternate" hreflang="${escape(targetLang)}" href="${escape(target)}">\n${text("Hedefte bu kaynak URL’ye dönüş kaydı:","Return annotation on target to this exact source:")} ${show(e.return_link)}`;
      const pair=`<link rel="alternate" hreflang="${escape(srcLang)}" href="${escape(url)}">\n<link rel="alternate" hreflang="${escape(targetLang)}" href="${escape(target)}">`;
      guide.after=`<!-- ${text("Kaynak sayfanın head bölümü","Source page head")}: ${escape(url)} -->\n${pair}\n\n<!-- ${text("Hedef sayfanın head bölümü","Target page head")}: ${escape(target)} -->\n${pair}`;
    }
    if(f.indexability&&f.indexability!=="indexable") {
      guide.warning=text(`Bu URL’nin kayıtlı durumu “${show(f.indexability)}”. Canonical tercihi kasıtlıysa önce dil kümesini canonical hedeflerle hizalayın; örneği olduğu gibi eklemeyin.`,`This URL is recorded as “${show(f.indexability)}”. If canonicalization is intentional, align the language cluster with canonical targets first; do not paste the example unchanged.`);
      if((f.canonical||f.header_canonical)&&String(f.canonical||f.header_canonical)!==url) {
        guide.after=null;
        guide.steps.unshift(text("Bu URL başka bir adrese canonical veriyor. Önce canonical hedefinin içeriğini, HTTP durumunu ve dil kümesini doğrulayın. Yalnızca parametreyi silerek veya bu URL’ye dönüş etiketi ekleyerek bulguyu kapatmayın.","This URL declares another canonical. First verify its content, HTTP response and language cluster. Do not resolve the finding by stripping parameters or adding a return to this URL before deciding the intended canonical."));
      }
    }
    guide.validation.unshift(text("Her iki URL’nin HTTP 200 yanıtını, doğru canonical’ı ve karşılıklı hreflang kümesini yeniden kontrol edin.","Recheck HTTP 200 responses, intended canonicals and reciprocal hreflang sets on both URLs."));
    guide.sources=[{title:"Google Search Central · hreflang",url:"https://developers.google.com/search/docs/specialty/international/localized-versions"}];
  } else if(["anchor-empty","anchor-generic","links-to-broken","links-to-broken-external","links-to-redirects","http-links","links-internal-nofollow"].includes(rule.id)) {
    observations.push({label:text("İlk kanıtın hedefi","First witness target"),value:show(e.destination)},{label:text("Bağlantı metni / etiketi","Link text / label"),value:show(e.text)},{label:text("Hedef HTTP durumu","Target HTTP status"),value:show(e.destination_status)});
    for(const [key,tr,en] of [["visible_text","Gözlenen metin içeriği","Observed text content"],["name_source","Etiket kaynağı","Label source"],["has_svg","SVG ikon var","SVG icon present"],["has_image","Görsel var","Image present"]] as const) {
      if(key in e)observations.push({label:text(tr,en),value:show(e[key])});
    }
    guide.steps=[text("Aşağıdaki kanıt tablosundan tam hedefi ve sayfadaki konumunu bulun; aynı bağlantı menü veya altbilgide tekrarlanıyorsa ortak bileşeni düzeltin.","Locate the exact target and its position in the evidence table; fix the shared component when the link repeats in navigation or the footer."),rule[lang].fix,text("Hedef değişecekse eşdeğer, çalışan sayfayı seçin. Sırf bulguyu kapatmak için ilgisiz bir ana sayfaya yönlendirmeyin.","When changing the target, choose a working equivalent page. Avoid redirecting to an unrelated home page just to remove the finding.")];
    if(target) {
      guide.before=`href: ${escape(target)}\n${text("Kayıtlı metin / etiket","Recorded text / label")}: ${show(e.text)}\n${text("Etiket kaynağı","Label source")}: ${show(e.name_source)}`;
      if(e.has_svg===true)guide.before+=`\n${text("SVG ikon mevcut; bu özet SVG yol verisini içermez.","SVG icon present; this summary does not contain its path data.")}`;
      if(e.name_source===null||e.name_source===undefined)guide.warning=text("Bu eski kayıtta etiket kaynağı doğrulanamadı. Boş metin, bağlantının içinin boş olduğunu kanıtlamaz; SVG, ARIA etiketi veya görsel bulunabilir. Tam gözlem için yeniden tarayın.","The label source could not be verified in this older record. Empty text does not prove empty link markup; it may contain SVG, an ARIA label or an image. Recrawl for a complete observation.");
      let anchorCopy=text("Hedef sayfanın konusunu açıklayan metin","Descriptive text about the destination page");
      try {if(new URL(target).hostname==="wa.me")anchorCopy=text("WhatsApp ile iletişime geçin","Contact us on WhatsApp");}catch{/* A non-HTTP recorded target still uses the descriptive placeholder. */}
      guide.after=rule.id.startsWith("anchor-")?`<a href="${escape(target)}">${escape(anchorCopy)}</a>`:`<a href="${text("https://example.com/dogru-hedef","https://example.com/correct-target")}">${escape(string(e.text)||text("Açıklayıcı bağlantı metni","Descriptive anchor text"))}</a>`;
      if(rule.id==="anchor-empty"&&e.has_svg===true)guide.after=`<a href="${escape(target)}" aria-label="${escape(anchorCopy)}">\n  <svg aria-hidden="true"><!-- ${text("Mevcut SVG ikonunun yollarını koruyun","Keep the existing SVG icon paths")} --></svg>\n</a>`;
    }
    if(rule.id.startsWith("anchor-"))guide.steps.splice(1,0,text("SVG/görsel içeren bağlantıyı yalnızca görünür metni olmadığı için boş saymayın. Açıklayıcı aria-label, geçerli aria-labelledby, görsel alt metni veya SVG başlığını kontrol edin; yalnızca etiketsiz bir ikon açıklayıcı ad sağlamaz.","Do not treat a graphic link as empty solely because it has no visible text. Check aria-label, valid aria-labelledby, image alt or SVG title; an unlabeled icon alone does not provide a descriptive name."));
    guide.validation.unshift(text("Kaynak sayfadaki bağlantıyı tekrar kontrol edin; hedef yanıtı, görünür metin ve varsa görsel alt metni beklenen şekilde olmalı.","Recheck the link on the source page: target response, visible text and any image alt text should match the intended result."));
    guide.sources=[{title:"Google Search Central · links",url:"https://developers.google.com/search/docs/crawling-indexing/links-crawlable"}];
  } else if(rule.id==="uncompressed") {
    add("content_encoding","Kayıtlı Content-Encoding","Recorded Content-Encoding");add("size_bytes","Yanıt boyutu (bayt)","Response size (bytes)");add("content_type","İçerik türü","Content type");
    guide.steps=[text("İsteğin geçtiği CDN, ters proxy ve uygulama sunucusunu belirleyin; sıkıştırmanın hangi katmanda yapılacağını seçin.","Identify the CDN, reverse proxy and application server; choose the layer that should compress responses."),rule[lang].fix,text("Nginx kullanılıyorsa aşağıdaki örneği uygun http/server bağlamına uyarlayın. Başka altyapıda eşdeğer sıkıştırma ayarını kullanın; örnek sunucu türünüzün tespiti değildir.","If you use Nginx, adapt the example to the appropriate http/server context. Use the equivalent setting on another stack; this example does not identify your server."),text("CDN önbelleğini uygun şekilde yenileyin; sıkıştırma destekleyen gerçek bir GET isteğinde Content-Encoding’i doğrulayın.","Refresh the relevant CDN cache and verify Content-Encoding using a real GET request that accepts compression.")];
    guide.before=`Content-Encoding: ${show(f.content_encoding)}\n${text("Kayıtlı yanıt boyutu","Recorded response size")}: ${show(f.size_bytes)} bytes`;
    guide.after="# Nginx · http/server\ngzip on;\ngzip_vary on;\ngzip_types text/plain text/css application/javascript application/json image/svg+xml;";
    guide.validation.unshift(`curl -sS -D - -o /dev/null -H 'Accept-Encoding: gzip, br' ${shell(url)}`);
    guide.sources=[{title:"Nginx · gzip module",url:"https://nginx.org/en/docs/http/ngx_http_gzip_module.html"}];
  } else if(rule.id==="slow-response") {
    add("response_ms","Ölçülen yanıt süresi (ms)","Measured response time (ms)");
    observations.push({label:text("Bu kontrolün eşiği","This check’s threshold"),value:"> 1.000 ms"});
    guide.steps=[text("Bu URL’yi birkaç kez ölçerek ilk istek, önbellekli istek ve farklı saatlerdeki süreleri ayırın; tek tarama ölçümü kalıcı darboğazı kanıtlamaz.","Measure this URL repeatedly, separating cold and cached requests and different times; one crawl measurement does not prove a persistent bottleneck."),rule[lang].fix,text("Yavaş veritabanı sorgularını, sunucu tarafı render işlemini ve harici API çağrılarını profilleyin. Kişiye özel yanıtları ortak önbelleğe almadan, uygun içerikte önbellek kullanın.","Profile slow queries, server rendering and external API calls. Cache suitable content without putting personalized responses into a shared cache.")];
    guide.before=`response_ms: ${show(f.response_ms)}`;
    guide.after=`curl -sS -o /dev/null -w 'TTFB: %{time_starttransfer}s\\n' ${shell(url)}`;
    guide.validation.unshift(text("Aynı koşullarda ölçümü tekrarlayın; taramadaki yanıt süresini ve gerçek kullanıcı performansını ayrı değerlendirin.","Repeat measurements under the same conditions; assess crawl response time and real-user performance separately."));
  } else if(["titles","meta-descriptions","headings"].includes(rule.category)) {
    const key=rule.category==="titles"?"title":rule.category==="meta-descriptions"?"meta_description":rule.id.startsWith("h2-")?"h2":"h1";
    add(key,text("Mevcut metin","Current text"),"Current text");add(`${key}_count`,"Öğe sayısı","Element count");
    if(typeof f[key]==="string") observations.push({label:text("Karakter sayısı (kayıtlı metinden)","Character count (from recorded text)"),value:number([...String(f[key])].length,lang)});
    guide.steps=[text("Aşağıdaki mevcut metni sayfanın amacı ve ana içeriğiyle karşılaştırın; ortak şablondan gelen tekrarları ilgili bileşende düzeltin.","Compare the recorded text below with the page’s purpose and main content; correct repeated template text in the shared component."),rule[lang].fix,text("Örnek metin bir taslaktır. Gerçek ürün, hizmet ve marka bilgisini kullanın; bilinmeyen içerik hakkında iddia eklemeyin.","The sample copy is a draft. Use the actual product, service and brand details; do not add claims about unknown content.")];
    const tag=(value:string)=>key==="meta_description"?`<meta name="description" content="${escape(value)}">`:`<${key}>${escape(value)}</${key}>`;
    guide.before=typeof f[key]==="string"?tag(String(f[key])):text("Bu metin alanı arşivde boş veya kaydedilmemiş.","This text field is empty or unrecorded in the archive.");
    guide.after=tag(key==="title"?text("Sayfanın ana konusu | Marka","Main page topic | Brand"):key==="meta_description"?text("Sayfanın sunduğu ürün veya hizmeti, kullanıcıya sağladığı somut faydayı ve ayırt edici ayrıntıyı anlatan özgün açıklama.","A unique description of the product or service, its concrete user benefit and a distinguishing detail."):text("Bölümün gerçek konusunu anlatan başlık","Heading describing the section’s actual topic"));
    if(rule.id==="headings-non-sequential") {
      guide.before=typeof e.level==="number"&&e.level>=1&&e.level<=6?(typeof e.previous_level==="number"&&e.previous_level>0?`<h${e.previous_level}>${escape(string(e.previous_text))}</h${e.previous_level}>`:text("<!-- Öncesinde başlık kaydı yok -->","<!-- No preceding heading recorded -->"))+`\n<h${show(e.level)}>${escape(string(e.text))}</h${show(e.level)}>`:null;
      guide.after=`<h1>${text("Sayfanın ana konusu","Main page topic")}</h1>\n<h2>${text("Alt bölüm","Subsection")}</h2>\n<h3>${text("Alt bölümün ayrıntısı","Subsection detail")}</h3>`;
    }
    guide.validation.unshift(text("Ham HTML ve JavaScript sonrası DOM’da etiket sayısını, metni ve sıralamayı tekrar kontrol edin. Kopya kontrolünde eşleşen diğer URL’leri de inceleyin.","Recheck tag count, text and order in raw HTML and rendered DOM. For duplicates, inspect the other matching URLs too."));
  } else if(rule.category==="images") {
    observations.push({label:text("İlk görsel","First image"),value:show(e.image)},{label:text("Görsel boyutu (bayt)","Image size (bytes)"),value:show(e.size_bytes)},{label:"alt",value:show(e.alt)});
    guide.steps=[text("Kanıttaki görsel URL’sini ilgili sayfa bileşeninde bulun; büyük dosya, eksik alt veya boyut bulgusuna göre varlığı ya da işaretlemeyi düzeltin.","Locate the evidence image in the page component; fix the asset or markup according to the size, alt or dimensions finding."),rule[lang].fix,text("Bilgi veren görsele anlamlı alt metin yazın; yalnızca dekoratif görselde alt=\"\" kullanın. Örnekteki dosya ve ölçüleri gerçek görsele göre değiştirin.","Write meaningful alt text for informative images; use alt=\"\" for purely decorative images. Replace the sample file and dimensions with actual values.")];
    if(e.image) guide.before=`<img src="${escape(string(e.image))}"${e.alt===null?"":` alt="${escape(string(e.alt))}"`}>`;
    guide.after=`<img src="/images/example-optimized.webp" alt="${text("Görseldeki içeriğin kısa açıklaması","Brief description of the image content")}" width="800" height="600">`;
  } else if(rule.category==="canonicals") {
    add("canonical","HTML canonical","HTML canonical");add("header_canonical","HTTP canonical","HTTP canonical");
    guide.steps=[text("Bu URL’nin dizine girmesi gereken asıl sürüm mü, yoksa kasıtlı bir kopya mı olduğuna karar verin.","Decide whether this URL should be the indexed primary version or an intentional duplicate."),rule[lang].fix,text("İç bağlantıları, site haritasını ve hreflang kümesini seçilen canonical stratejisiyle hizalayın.","Align internal links, sitemap and hreflang annotations with the chosen canonical policy.")];
    if(f.canonical||f.header_canonical)guide.before=`<link rel="canonical" href="${escape(string(f.canonical)||string(f.header_canonical))}">`;
    guide.after=`<!-- ${text("Yalnızca bu URL asıl sürümse","Only if this URL is the primary version")} -->\n<link rel="canonical" href="${escape(url)}">`;
    if(f.pagination_candidate===true) {
      add("canonical_content_equal","Canonical ile kayıtlı metin aynı","Recorded text matches canonical");
      guide.steps.unshift(text("Bu URL sayfalama veya sayısal parametre içeriyor. Canonical hedefiyle liste öğelerini karşılaştırın: farklı öğeler gösteren gerçek sayfalamada her sayfa kendisini canonical göstermeli; aynı içerik kopyasında ana canonical korunabilir.","This URL has pagination or a numeric parameter. Compare its list items with the canonical: distinct paginated content should use a self canonical per page; an actual duplicate can retain the primary canonical."));
      guide.warning=text("Sayfalama parametresi tek başına kopya içerik kanıtı değildir. Kayıtlı metnin farklı olması da tek başına liste öğelerinin farklı olduğunu kanıtlamaz; öğeleri kontrol edin. Hreflang yalnızca mevcut eşdeğer dil sürümlerini kapsamalı.","A pagination parameter alone does not prove duplicate content. Different recorded text alone does not prove different list items; inspect the items. Hreflang should include only existing equivalent language versions.");
      guide.sources=[{title:"Google Search Central · pagination",url:"https://developers.google.com/search/docs/specialty/ecommerce/pagination-and-incremental-page-loading"}];
    }
  } else if(rule.category==="security") {
    add("hsts","HSTS kaydı","HSTS recorded");add("mixed_content","Karışık içerik sayısı","Mixed content count");add("insecure_forms","Güvensiz form sayısı","Insecure form count");
    guide.steps=[text("HTTPS kapsamını, sertifikaları ve ilgili kaynak/form hedeflerini doğrulayın.","Verify HTTPS coverage, certificates and resource/form targets."),rule[lang].fix,text("HSTS’yi HTTPS kapsamı doğrulandıktan sonra kontrollü etkinleştirin; alt alan kapsamını ayrıca incelemeden genişletmeyin.","Enable HSTS in a controlled rollout after HTTPS coverage is confirmed; assess subdomains before expanding coverage.")];
    if(rule.id==="missing-hsts")guide.after="Strict-Transport-Security: max-age=31536000";
  } else if(rule.category==="mobile") {
    add("viewport","Mevcut viewport","Current viewport");guide.before=typeof f.viewport==="string"?`<meta name="viewport" content="${escape(String(f.viewport))}">`:null;
    guide.after='<meta name="viewport" content="width=device-width, initial-scale=1">';
  } else if(rule.category==="directives") {
    add("meta_robots","Meta robots","Meta robots");add("x_robots_tag","X-Robots-Tag","X-Robots-Tag");add("noindex","noindex","noindex");add("nofollow","nofollow","nofollow");
    guide.steps.unshift(text("Kısıtlamanın kasıtlı olup olmadığını doğrulayın. Dizine girmemesi gereken sayfadan noindex’i kaldırmayın.","Confirm whether the restriction is intentional. Keep noindex on pages that should remain outside the index."));
    guide.after='<!-- Only for pages intended to be indexable -->\n<meta name="robots" content="index, follow">';
  } else if(rule.category==="rendering") {
    add("rendered","Rendering başarılı","Rendering succeeded");add("render_error","Rendering hatası","Rendering error");add("raw_title","İlk yanıttaki başlık","Original title");add("title","DOM başlığı","DOM title");
    guide.steps.unshift(text("Kaydedilmiş tarayıcı hatasını ve ilk yanıt/DOM farkını inceleyin; kritik metin ve bağlantıların seçili bekleme süresinde oluştuğunu doğrulayın.","Inspect the recorded browser error and response/DOM difference; verify critical content and links appear within the configured wait."));
  } else if(rule.category==="performance") {
    add("size_bytes","Yanıt boyutu (bayt)","Response size (bytes)");add("truncated","Yanıt kırpılmış","Truncated response");
    guide.after=text("Örnek düzenleme: büyük satır içi veriyi ve kullanılmayan HTML’yi çıkarın; uzun listeleri erişilebilir sayfalara ayırın.","Example change: remove large inline data and unused markup; split long lists into accessible pages.");
  } else if(rule.category==="response-codes") {
    add("error","İstek hatası","Request error");add("redirect_url","Yönlendirme hedefi","Redirect target");add("robots_rule","Robots kuralı","Robots rule");
    guide.steps.unshift(text("Yanıt durumunu ve hatanın kasıtlı olup olmadığını doğrulayın. Kaldırılmış içerik, yönlendirme ve erişim kısıtlamasını ayrı ele alın.","Verify the response outcome and whether it is intentional. Treat removed content, redirects and access restrictions separately."));
  } else if(rule.category==="url") {
    add("canonical","Canonical","Canonical");guide.before=url;
    guide.after=text("Örnek eşleme: eski URL → seçilen temiz canonical URL (gerçek hedefi siz belirleyin).","Example mapping: old URL → chosen clean canonical URL (choose the actual destination).");
    guide.steps.push(text("Parametreleri körlemesine silmeyin; içerik ve filtre işlevlerini koruyun. Değişen URL’ler için uygun yönlendirmeyi ve iç bağlantıları güncelleyin.","Do not blindly remove parameters; preserve content and filtering behavior. Update appropriate redirects and internal links for changed URLs."));
  } else {
    for(const name of ["word_count","canonical","rel_next","rel_prev","og_title","og_description"])if(name in f)add(name,name,name);
    guide.steps.push(text("Bu kontrolün ayrıntılı bağlamı saklanmamışsa ilgili kaynağı yakalayacak ayarlarla yeniden tarayın; eksik veriyi tahminle tamamlamayın.","If detailed context was not stored, recrawl with settings that capture it; do not fill missing evidence with guesses."));
  }
  return guide;
}
