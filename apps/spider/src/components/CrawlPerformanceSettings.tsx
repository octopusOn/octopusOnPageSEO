import type { CrawlConfig } from "../api/types";
import { useI18n, type Key } from "../i18n";
import { activeSpeedProfile, applySpeedProfile, effectiveBrowserConcurrency, performanceNumber, speedProfiles, type SpeedProfile } from "../lib/performance";

const labels: Record<SpeedProfile, Key> = { gentle: "speedGentle", balanced: "speedBalanced", fast: "speedFast" };

export function CrawlPerformanceSettings({config,onChange,disabled=false}:{config:CrawlConfig;onChange:(config:CrawlConfig)=>void;disabled?:boolean}) {
  const {t}=useI18n();
  const active=activeSpeedProfile(config);
  const javascript=config.rendering.mode==="javascript";
  const browserLimit=effectiveBrowserConcurrency(config);
  function numeric(field:"threads"|"max_requests_per_second"|"concurrency",label:Key,min:number,max?:number) {
    const value=field==="concurrency" ? config.rendering.concurrency : config.speed[field];
    return <label className="oc-field">
      {t(label)}
      <input className="oc-input" type="number" disabled={disabled} min={min} max={max} step={field==="max_requests_per_second" ? "any" : 1}
        value={Number.isFinite(value)?value:""} onChange={e=>{
          const next=performanceNumber(e.target.value);
          onChange(field==="concurrency" ? {...config,rendering:{...config.rendering,concurrency:next}} : {...config,speed:{...config.speed,[field]:next}});
        }}/>
      {field==="max_requests_per_second" && <small>{t("unlimitedHint")}</small>}
    </label>;
  }
  return <div className="crawl-performance">
    <div className="row between"><span className="performance-label">{t("crawlSpeed")}</span><small className="muted">{t(active ? labels[active] : "speedCustom")}</small></div>
    <div className="speed-profiles" role="group" aria-label={t("speedProfile")}>
      {(Object.keys(speedProfiles) as SpeedProfile[]).map(profile=><button key={profile} type="button" className={`oc-btn small ${active===profile?"active":""}`} aria-pressed={active===profile} disabled={disabled} onClick={()=>onChange(applySpeedProfile(config,profile))}>{t(labels[profile])}</button>)}
    </div>
    <div className="field-grid performance-fields">
      {numeric("threads","threads",1,64)}
      {numeric("max_requests_per_second","requestsPerSecond",0)}
      {javascript && numeric("concurrency","renderConcurrency",1,8)}
    </div>
    <p className="setting-hint">{t(javascript ? "speedSharedRateHint" : "speedHttpRateHint")}</p>
    {javascript && browserLimit!==null && <p className="setting-hint performance-effective">{t("effectiveBrowserLimit").replace("{count}",String(browserLimit))}</p>}
  </div>;
}
