import type {AnalysisRequest,OpportunityProject} from "../api/opportunities";

export const emptyProject:OpportunityProject={id:"",name:"",site_url:"",gsc_property:"",ga4_property:"",google_connection:"",brand_terms:[],goal:"all-events",event_name:"",ga4_origin_confirmed:false,crawl_id:null};
export function defaultPeriods(now=new Date()):Pick<AnalysisRequest,"current"|"previous"> {
  const today=new Intl.DateTimeFormat("en-CA",{timeZone:"America/Los_Angeles",year:"numeric",month:"2-digit",day:"2-digit"}).format(now);
  const day=new Date(`${today}T12:00:00Z`);
  const date=(offset:number)=>{const next=new Date(day);next.setUTCDate(next.getUTCDate()+offset);return next.toISOString().slice(0,10);};
  return {current:{start:date(-30),end:date(-3)},previous:{start:date(-58),end:date(-31)}};
}
export function newAnalysis(project_id:string,now=new Date()):AnalysisRequest{return{project_id,...defaultPeriods(now),country:"",device:"",brand:"",path_prefix:""};}
export function siteHost(value:string){try{return new URL(value).hostname.toLowerCase().replace(/^www\./,"");}catch{return"";}}
export function validProject(project:OpportunityProject){try{const url=new URL(project.site_url);return !!project.name.trim()&&["http:","https:"].includes(url.protocol)&&!!url.hostname&&!url.username&&!url.password&&(project.goal!=="event"||/^[a-zA-Z][a-zA-Z0-9_]*$/.test(project.event_name));}catch{return false;}}
export function opportunityWarning(value:string,tr:boolean){
  const messages:Record<string,[string,string]>={
    "position-incomplete":["Bazı gösterimlerin konum bilgisi eksik. Bu toplamlar için ortalama konum ve sıralama fırsatı varsayılmadı.","Some impressions have no valid position. Position-based opportunities were not inferred from partial averages."],
    "ga4-business-incomplete":["Eksik, eşiklenmiş veya kesilmiş GA4 ölçümleri ilgili URL’nin önceliğini artırmaz; iş değeri çarpanı tarafsız kalır.","Incomplete, thresholded or truncated GA4 measurements do not boost affected URL priorities."],
    "ga4-incomplete":["GA4 yanıtındaki toplam satır sayısına ulaşılamadı; toplamlar bilinmiyor ve iş değeri katkısı kullanılmadı.","GA4 pagination ended before its reported total; totals remain unknown and no business boost is applied."],
    "loss-incomplete":["Satır sınırı nedeniyle dönem toplamları eksik olabilir; kesin trafik kaybı kuralı uygulanmadı.","Row caps may truncate period totals; confirmed traffic-loss rules were not applied."],
    "unequal-periods":["Dönem uzunlukları farklı; trafik kaybı kuralı uygulanmadı.","Periods differ in length; traffic-loss rules were not applied."],
    "query-subset-totals":["Sayfa toplamları sorgu–sayfa alt kümesinden hesaplandı; mülk toplamı değildir.","Page totals come from the query/page subset, not the full property."],
    "previous-unavailable":["Önceki dönemin karşılaştırılabilir verisi yok.","Comparable previous-period observations are unavailable."],
    "brand-not-configured":["Marka terimleri tanımlanmadı; markalı/markasız sorgu ayrımı yapılmadı.","Brand terms are not configured; branded queries are not separated."],
    "missing-rows-not-zero":["Önceki dönemde bulunan bazı URL’ler güncel veride yok. Sıfır trafik veya kesin kayıp sayılmadı.","Some previous URLs are absent now; absence is not treated as zero traffic or confirmed loss."],
    "ga4-stale":["Son Google güncellemesinde GA4 alınamadı; eski GA4 verileri önceliklendirmeye katılmadı.","GA4 failed during the latest refresh; cached GA4 observations were excluded."],
    "ga4-unavailable":["GA4 verisi yok; iş değeri katkısı tarafsız bırakıldı.","GA4 is unavailable; business contribution remains neutral."],
    "ga4-not-comparable":["GA4 kapsamı, hedefi veya URL eşlemesi karşılaştırılabilir değil; sıralamaya katkı verilmedi.","GA4 scope, goal or URL mapping is not comparable; no ranking boost was applied."],
    "daily-property-wide":["Günlük mülk toplamları proje klasörüyle aynı kapsamda değil; grafik gösterilmiyor.","Daily property totals do not match the project folder scope; the chart is hidden."],
    "daily-unsegmented":["Günlük grafik bu filtrelerle karşılaştırılabilir olmadığından gösterilmiyor.","The unsegmented daily chart is hidden for these filters."],
    "property-overview-totals":["Genel göstergeler, GSC günlük mülk toplamlarından hesaplanır. URL/sorgu ayrıntıları farklı toplulaştırılır ve API sınırlamaları nedeniyle daha az veri içerebilir.","Overview metrics use daily GSC property totals. URL/query details use different aggregation and can contain less data because of API limitations."],
    "observed-page-totals":["Genel göstergeler gözlenen sayfa satırlarının toplamıdır; GSC mülk toplamı değildir. Filtre veya klasör kapsamı korunur.","Overview metrics sum observed page rows, not the GSC property total. Filters and folder scope are preserved."],
    "loss-query-subset":["Yalnız sorgu–sayfa alt kümesi karşılaştırılabiliyor; kesin sayfa trafik kaybı çıkarılmadı.","Only a query/page subset is comparable; no confirmed page traffic loss is inferred."],
    "gsc-top-rows":["GSC API gözlenen üst satırları döndürür; tüm sorgu ve sayfaları garanti etmez.","GSC API returns observed top rows, not guaranteed exhaustive queries or pages."],
    "row-limit":["Satır sınırına ulaşıldı; veri kapsamı kısıtlı.","Row limit reached; coverage is limited."],
    "ga4-limited":["GA4 eşikleme, örnekleme veya toplulaştırma kaybı bildirdi.","GA4 reported thresholding, sampling or aggregation loss."],
    "ga4-unmatched-url":["Bazı GA4 açılış sayfaları URL ile eşleştirilemedi.","Some GA4 landing pages could not be matched to URLs."],
    "imported-coverage-unknown":["Dosya dışa aktarımının tam kapsamı doğrulanamadı.","Completeness of the imported export is unverified."],
    "event-count-unavailable-rate-only":["Seçili olayın oturum oranı mevcut; olay adedi bu veri setinde yok.","The selected event's session rate is available, but its count is not in this dataset."],
  };
  if(value.startsWith("ga4-sync-unavailable:"))return `${tr?"GA4 güncellemesi tamamlanamadı":"GA4 sync could not complete"}: ${value.slice(21)}`;
  return messages[value]?.[tr?0:1]??value;
}

export const signalLabel=(kind:string,tr:boolean)=>({"low-ctr":tr?"Düşük CTR":"Low CTR","near-win":tr?"Yükselmeye yakın":"Near win","traffic-loss":tr?"Trafik kaybı":"Traffic loss"}[kind]??kind);
export const confidenceLabel=(value:string,tr:boolean)=>({observed:tr?"Gözlenen veri":"Observed evidence",limited:tr?"Sınırlı veri":"Limited coverage",review:tr?"İnceleme adayı":"Review candidate"}[value]??value);
export function metric(value:number|null|undefined,lang:string,percent=false){return value==null?"—":new Intl.NumberFormat(lang==="tr"?"tr-TR":"en-US",{maximumFractionDigits:percent?2:1,style:percent?"percent":"decimal"}).format(value);}


export function analysisKey(request:AnalysisRequest){return JSON.stringify([request.project_id,request.current.start,request.current.end,request.previous.start,request.previous.end,request.country,request.device,request.brand,request.path_prefix]);}

export function opportunityPhase(phase:string,tr:boolean){const labels:Record<string,[string,string]>={starting:["Google güncellemesi başlıyor","Starting Google refresh"],"gsc-pages":["GSC sayfaları alınıyor","Fetching GSC pages"],"gsc-queries":["GSC sorguları alınıyor","Fetching GSC queries"],"gsc-query-pages":["GSC sorgu–sayfa ilişkileri alınıyor","Fetching GSC query/page pairs"],"gsc-daily":["GSC günlük verileri alınıyor","Fetching daily GSC data"],ga4:["GA4 organik trafik verileri alınıyor","Fetching GA4 organic traffic"],"ga4-events":["GA4 seçili önemli olay alınıyor","Fetching selected GA4 key event"],saving:["Veriler kaydediliyor","Saving observations"],analyzing:["Fırsatlar analiz ediliyor","Analyzing opportunities"],cancelling:["İşlem iptal ediliyor","Cancelling operation"]};return labels[phase]?.[tr?0:1]??(tr?"Veriler işleniyor":"Processing observations");}
