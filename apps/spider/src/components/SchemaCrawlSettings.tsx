import type {CrawlConfig} from "../api/types";
import {useI18n} from "../i18n";

export function SchemaCrawlSettings({config,onChange,disabled=false}:{config:CrawlConfig;onChange:(config:CrawlConfig)=>void;disabled?:boolean}) {
 const {lang}=useI18n();const tr=lang==="tr";
 return <div className="schema-crawl-settings"><label className="setting-switch"><span>{tr?"Schema / zengin sonuç alanlarını tara ve puanla":"Crawl and score Schema / rich-result properties"}</span><input type="checkbox" role="switch" disabled={disabled} checked={config.schema_validation===true} onChange={e=>onChange({...config,schema_validation:e.target.checked})}/></label><p className="setting-hint">{tr?"Desteklenen JSON-LD türlerinin zorunlu alan hataları SEO Röntgenine ve puana dahil edilir. Schema yokluğu ve önerilen alanlar puan düşürmez. JavaScript modunda oluşturulan DOM incelenir. Ek ağ isteği yapılmaz; Google zengin sonuç onayı değildir.":"Required-property errors in supported JSON-LD types contribute to the SEO score and breakdown. Absent Schema and recommended fields do not deduct points. JavaScript mode inspects the rendered DOM. No extra requests; this is not Google rich-result approval."}</p></div>;
}
