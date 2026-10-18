import type {SchemaReport,TablePage} from "../api/types";
import {useI18n} from "../i18n";
import {SchemaResult} from "../pages/SchemaValidator";
import {record} from "../lib/seoRemediation";

export function SavedSchemaResult({url,facts,report,error}:{url:string;facts:TablePage;report?:SchemaReport|null;error?:string|null}) {
 const {lang}=useI18n();const tr=lang==="tr";const f=record(facts);
 const errors:Record<string,string>={"incomplete-document":tr?"HTML/DOM eksik veya JavaScript render başarısız; puanlama yapılmadı.":"Incomplete HTML/DOM or failed JavaScript rendering; not scored.","input-limit":tr?"Belge 4 MiB kontrol sınırını aşıyor; puanlama yapılmadı.":"Document exceeds the 4 MiB validation limit; not scored.","report-limit":tr?"Kanıt raporu saklama sınırını aşıyor; puanlama yapılmadı.":"Evidence exceeds the storage limit; not scored."};
 if(!report)return <p className="oc-banner info">{error?(errors[error]??error):(tr?"Bu URL için tarama sırasında Schema alan raporu kaydedilmemiş. Yeni taramada Schema / zengin sonuç alanlarını tara ve puanla seçeneğini işaretleyin.":"No crawl Schema property report was saved for this URL. Enable Crawl and score Schema / rich-result properties for a new crawl.")}</p>;
 return <div className="saved-schema-result"><p className="oc-banner info">{tr?"Kayıtlı tarama kanıtı · Sayfaya yeni istek gönderilmez. Puan etkisi bu taramanın SEO Röntgeninde gösterilir.":"Saved crawl evidence · No new page request. Score impact appears in this crawl's SEO breakdown."}</p><SchemaResult saved result={{url,status:typeof f.status==="number"?f.status:null,redirects:[],rendered:f.rendered===true,render_error:null,indexability:typeof f.indexability==="string"?f.indexability:null,truncated:false,error:null,report,tested_at_ms:typeof f.crawled_at_ms==="number"?f.crawled_at_ms:0}}/></div>;
}
