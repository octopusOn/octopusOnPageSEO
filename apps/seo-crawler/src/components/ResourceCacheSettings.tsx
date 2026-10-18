import type { CrawlConfig } from "../api/types";
import { useI18n } from "../i18n";

export function ResourceCacheSettings({config,onChange,disabled=false}:{config:CrawlConfig;onChange:(config:CrawlConfig)=>void;disabled?:boolean}) {
  const {t}=useI18n();
  return <div className="oc-field">
    <label className="setting-switch"><span>{t("renderResourceCache")}</span><input type="checkbox" role="switch" disabled={disabled} checked={config.rendering.cache_resources!==false} onChange={e=>onChange({...config,rendering:{...config.rendering,cache_resources:e.target.checked}})}/></label>
    <small>{t("renderResourceCacheHint")}</small>
  </div>;
}
