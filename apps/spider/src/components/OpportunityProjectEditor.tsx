import {useState} from "react";
import {useQuery} from "@tanstack/react-query";
import type {CrawlMeta} from "../api/types";
import {opportunitiesApi,type OpportunityProject,type WorkspaceStatus} from "../api/opportunities";
import {useI18n} from "../i18n";
import {siteHost,validProject} from "../lib/opportunities";
import {Button,Modal} from "./ui";
import {errorText} from "../api/client";

export function OpportunityProjectEditor({initial,status,crawls,onSave,onClose}:{initial:OpportunityProject;status:WorkspaceStatus;crawls:CrawlMeta[];onSave:(project:OpportunityProject)=>Promise<void>;onClose:()=>void}){
 const {lang}=useI18n();const tr=lang==="tr";
 const [project,setProject]=useState(initial),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const catalog=useQuery({queryKey:["opportunity-catalog",project.google_connection],queryFn:()=>opportunitiesApi.catalog(project.google_connection),enabled:!!project.google_connection});
 const update=<K extends keyof OpportunityProject>(key:K,value:OpportunityProject[K])=>setProject(old=>({...old,[key]:value}));
 const snapshots=crawls.filter(c=>c.status==="completed"&&siteHost(c.start_urls[0]??"")===siteHost(project.site_url));
 async function save(){if(!validProject(project))return;setBusy(true);setError("");try{await onSave({...project,crawl_id:snapshots.some(c=>c.id===project.crawl_id)?project.crawl_id:null});}catch(error){setError(errorText(error));}finally{setBusy(false);}}
 return <Modal title={tr?"Fırsat projesi":"Opportunity project"} onClose={onClose} footer={<><Button disabled={busy} onClick={onClose}>{tr?"Vazgeç":"Cancel"}</Button><Button primary busy={busy} disabled={!validProject(project)} onClick={()=>void save()}>{tr?"Projeyi kaydet":"Save project"}</Button></>}>
  <div className="field-grid">
   <label className="oc-field">{tr?"Proje adı":"Project name"}<input className="oc-input" value={project.name} maxLength={200} onChange={e=>update("name",e.target.value)}/></label>
   <label className="oc-field">{tr?"Site adresi":"Site URL"}<input className="oc-input" type="url" placeholder="https://example.com/" value={project.site_url} onChange={e=>update("site_url",e.target.value)}/></label>
  </div>
  <label className="oc-field">{tr?"Google bağlantısı (dosya aktarımı için gerekmez)":"Google connection (not required for imports)"}<select className="oc-select" value={project.google_connection} onChange={e=>update("google_connection",e.target.value)}><option value="">{tr?"Bağlantı seçilmedi":"No connection"}</option>{status.connections.map(c=><option key={c.id} value={c.id}>{c.label} · {c.id.slice(0,6)}</option>)}</select></label>
  {catalog.isFetching&&<p role="status">{tr?"Google mülkleri yükleniyor…":"Loading Google properties…"}</p>}
  {catalog.error&&<div className="oc-banner warning">{errorText(catalog.error)}</div>}
  {catalog.data?.warnings.map((warning,i)=><p className="faint" key={i}>{warning}</p>)}
  <div className="field-grid">
   <label className="oc-field">{tr?"Search Console mülkü":"Search Console property"}<PropertyPicker value={project.gsc_property} properties={catalog.data?.gsc??[]} empty={tr?"Mülk seçin":"Choose a property"} placeholder="sc-domain:example.com" onChange={value=>update("gsc_property",value)}/></label>
   <label className="oc-field">{tr?"GA4 mülkü (isteğe bağlı)":"GA4 property (optional)"}<PropertyPicker value={project.ga4_property} properties={catalog.data?.ga4??[]} empty={tr?"GA4 kullanılmayacak":"Do not use GA4"} placeholder="properties/123456789" onChange={value=>update("ga4_property",value)}/></label>
  </div>
  <label className="oc-field">{tr?"Marka terimleri (her satıra bir tane)":"Brand terms (one per line)"}<textarea className="oc-input" rows={3} value={project.brand_terms.join("\n")} placeholder={tr?"Marka adınız\nYaygın yazım varyasyonu":"Brand name\nCommon spelling variation"} onChange={e=>update("brand_terms",e.target.value.split("\n"))}/></label>
  <div className="field-grid"><label className="oc-field">{tr?"İş hedefi":"Business goal"}<select className="oc-select" value={project.goal} onChange={e=>update("goal",e.target.value as OpportunityProject["goal"])}><option value="all-events">{tr?"Tüm GA4 önemli olayları":"All GA4 key events"}</option><option value="event">{tr?"Belirli bir önemli olay":"One specific key event"}</option><option value="revenue">{tr?"Satın alma geliri":"Purchase revenue"}</option></select></label>{project.goal==="event"&&<label className="oc-field">{tr?"GA4 olay adı":"GA4 event name"}<input className="oc-input mono" value={project.event_name} placeholder="generate_lead" onChange={e=>update("event_name",e.target.value)}/></label>}</div>
  <label className="op-check"><input type="checkbox" checked={project.ga4_origin_confirmed} onChange={e=>update("ga4_origin_confirmed",e.target.checked)}/><span>{tr?"GA4 açılış sayfası yollarının bu site adresine ait olduğunu doğruluyorum. Çok alan adlı mülklerde belirsiz eşlemeyi kullanmayın.":"I confirm that GA4 landing-page paths belong to this site URL. Do not enable ambiguous mapping for multi-domain properties."}</span></label>
  <label className="oc-field">{tr?"Teknik kanıt için kayıtlı SEO Crawler taraması":"Saved SEO Crawler crawl for technical evidence"}<select className="oc-select" value={project.crawl_id??""} onChange={e=>update("crawl_id",e.target.value||null)}><option value="">{tr?"Tarama bağlanmadı":"No crawl linked"}</option>{snapshots.map(c=><option key={c.id} value={c.id}>{c.name} · {new Date(c.created_at_ms).toLocaleDateString(tr?"tr-TR":"en-US")}</option>)}</select></label>
  <p className="faint">{tr?"Teknik kanıt kayıtlı taramadan okunur; yeni tarama başlatılmaz. İş hedefi değişirse GA4 verilerini yeniden alın.":"Technical evidence is read from the saved crawl; no new crawl starts. Refresh GA4 data after changing the business goal."}</p>
  {error&&<div className="oc-banner error" role="alert">{error}</div>}
 </Modal>;
}

function PropertyPicker({value,properties,empty,placeholder,onChange}:{value:string;properties:{id:string;name:string}[];empty:string;placeholder:string;onChange:(value:string)=>void}){
 if(!properties.length)return <input className="oc-input" value={value} placeholder={placeholder} onChange={e=>onChange(e.target.value)}/>;
 return <select className="oc-select op-property-picker" value={value} onChange={e=>onChange(e.target.value)}><option value="">{empty}</option>{value&&!properties.some(p=>p.id===value)&&<option value={value}>{value}</option>}{properties.map(p=><option key={p.id} value={p.id}>{p.name===p.id?p.id:`${p.name} · ${p.id}`}</option>)}</select>;
}
