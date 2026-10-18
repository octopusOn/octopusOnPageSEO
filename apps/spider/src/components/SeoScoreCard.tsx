import { ArrowRight, CircleAlert, ShieldCheck } from "lucide-react";
import type { CrawlMeta, SeoScore } from "../api/types";
import { issueById } from "../catalog";
import { useI18n, number, type Key } from "../i18n";
import { scorePointText } from "../lib/seoAnatomy";
import { Button } from "./ui";

export function SeoScoreCard({score,meta,onIssue,onIssues}:{score?:SeoScore;meta:CrawlMeta;onIssue:(id:string)=>void;onIssues:()=>void}) {
  const {t,lang}=useI18n();
  const supported=!!score&&[2,3,4,5].includes(score.version);
  const value=supported && score.value!==null && Number.isFinite(score.value) ? Math.max(0,Math.min(100,score.value)) : null;
  const grade:Key=value===null ? "seoScoreUnavailable" : value>=90 ? "seoScoreExcellent" : value>=75 ? "seoScoreGood" : value>=50 ? "seoScoreImprove" : "seoScorePoor";
  const tone=value===null ? "unknown" : value>=90 ? "excellent" : value>=75 ? "good" : value>=50 ? "improve" : "poor";
  const partial=meta.status!=="completed" || meta.limit_reached || meta.config.mode==="list";
  const limited=partial || !!score?.limited_urls || !!score?.unverified_anchor_findings || !!score?.schema_validation?.unverified_urls;
  return <section className={`oc-panel seo-score-panel score-${tone}`} aria-label={t("seoScoreTitle")}>
    <div className="seo-score-gauge" style={{background:`conic-gradient(var(--score-tone) ${(value??0)*3.6}deg,var(--surface-2) 0)`}}
      role={value===null ? "img" : "meter"} aria-label={t("seoScoreTitle")} aria-valuemin={value===null ? undefined : 0} aria-valuemax={value===null ? undefined : 100} aria-valuenow={value??undefined} aria-valuetext={value===null ? t(grade) : `${number(value,lang)} / 100 · ${t(grade)}`}>
      <div><strong>{value===null ? "—" : number(value,lang)}</strong><span>/ 100</span></div>
    </div>
    <div className="seo-score-copy">
      <div className="row seo-score-heading"><h2>{t("seoScoreTitle")}</h2><span className="seo-score-grade">{t(grade)}</span></div>
      <p>{t("seoScoreDescription")}</p>
      {supported && <div className="seo-score-counts"><span><b>{number(score.urls,lang)}</b> {t("seoScoreInternalUrls")}</span><span><b>{number(score.affected_urls,lang)}</b> {t("seoScoreAffectedUrls")}</span><span><b>{number(score.analyzed_html,lang)}</b> {t("seoScoreHtmlPages")}</span></div>}
      {!!(score?.excluded_findings||score?.excluded_links||score?.excluded_urls) && <p className="seo-score-excluded"><ShieldCheck size={14}/>{t("seoScoreCloudflareExcluded")} · {number(score.excluded_findings??0,lang)} {t("seoExcludedFindings")}</p>}
      {!!score?.canonical_context_findings&&<p className="seo-score-excluded">{number(score.canonical_context_findings,lang)} {t("seoCanonicalContextExcluded")}</p>}
      {!!score?.anchor_context_findings&&<p className="seo-score-excluded">{number(score.anchor_context_findings,lang)} {t("seoAnchorContextExcluded")}</p>}
      {!!score?.advisory_findings&&<p className="seo-score-excluded">{number(score.advisory_findings,lang)} {t("seoAdvisoryNoPenalty")}</p>}
      {!!score?.contact_redirect_findings&&<p className="seo-score-excluded">{number(score.contact_redirect_findings,lang)} {t("seoContactRedirectExcluded")}</p>}
      {!!score?.unverified_anchor_findings&&<p className="seo-score-limited"><CircleAlert size={14}/>{number(score.unverified_anchor_findings,lang)} {t("seoAnchorEvidenceMissing")}</p>}
      {limited && <p className="seo-score-limited"><CircleAlert size={14}/>{t("seoScorePartial")}{!!score?.limited_urls && ` · ${number(score.limited_urls,lang)} ${t("seoScoreLimitedUrls")}`}</p>}
      {value===null && <p className="seo-score-empty">{t(!supported ? "seoScoreBackendMissing" : score.unknown_findings ? "seoScoreUnknownRules" : "seoScoreNeedsHtml")}</p>}
      <details className="seo-score-method"><summary>{t("seoScoreMethod")}</summary><p>{t("seoScoreFormula")}</p><p>{t("seoScoreScope")}</p><p>{t("seoScoreFilters")}</p></details>
    </div>
    <div className="seo-score-drivers">
      <div className="row between"><h3>{t("seoScoreDrivers")}</h3><Button small className="ghost" onClick={onIssues}>{t("allIssues")}<ArrowRight size={13}/></Button></div>
      {supported && score.deductions.slice(0,3).map(driver=><button className="seo-score-driver" key={driver.id} onClick={()=>onIssue(driver.id)}>
        <span>{issueById.get(driver.id)?.[lang].title??driver.id}<small>{number(driver.count,lang)} {t("seoScoreAffectedUrls")}</small></span>
        <b>−{scorePointText(driver.points,lang)}<small>{t("seoScorePoints")}</small></b><ArrowRight size={13}/>
      </button>)}
      {value!==null && score?.deductions.length===0 && <p className="seo-score-clean"><ShieldCheck size={17}/>{t("seoScoreNoFindings")}</p>}
    </div>
  </section>;
}
