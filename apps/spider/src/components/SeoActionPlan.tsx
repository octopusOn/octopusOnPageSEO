import { useState } from "react";
import { ArrowRight, ListOrdered } from "lucide-react";
import { categories } from "../catalog";
import { number, useI18n } from "../i18n";
import type { SeoAction } from "../lib/seoActionPlan";
import { scoreLossText } from "../lib/seoAnatomy";
import { Button } from "./ui";

export function SeoActionPlan({ actions, available, onIssue, onDetails }: {
  actions: SeoAction[];
  available: boolean;
  onIssue: (id: string) => void;
  onDetails?: (id:string)=>void;
}) {
  const { t, lang } = useI18n();
  const [showAll, setShowAll] = useState(false);
  const shown = showAll ? actions : actions.slice(0, 3);
  return <section className="seo-action-plan" aria-label={t("seoActionTitle")}>
    <header>
      <div><h3><ListOrdered size={17}/>{t("seoActionTitle")}</h3><p>{t("seoActionOrder")}</p></div>
      <span>{number(actions.length, lang)} {t("seoActionCount")}</span>
    </header>
    {actions.length > 0 ? <>
      <ol className="seo-action-list">
        {shown.map(({ rule, deduction, rank }) => <li className={`seo-action ${rule.priority}`} key={rule.id}>
          <span className="seo-action-rank" aria-label={`${t("seoActionRank")} ${rank}`}>{rank}</span>
          <div className="seo-action-content">
            <div className="seo-action-heading"><h4>{rule[lang].title}</h4><span className={`seo-action-priority ${rule.priority}`}>{t(rule.priority as "high"|"medium"|"low")}</span>{rank === 1 && <span className="seo-action-first">{t("seoActionFirst")}</span>}</div>
            <div className="seo-action-meta"><span>{categories.find(category => category.id === rule.category)?.[lang] ?? rule.category}</span><span>{number(deduction.count, lang)} {t("seoScoreAffectedUrls")}</span><span>{t("seoAnatomyGlobalImpact")}: <b>{scoreLossText(deduction.points, lang)} {t("seoScorePoints")}</b></span></div>
            <div className="seo-action-fix"><strong>{t("seoActionFix")}</strong><p>{rule[lang].fix}</p></div>
            <div className="seo-action-footer"><details><summary>{t("seoActionWhy")}</summary><p>{rule[lang].description}</p></details><div className="seo-action-buttons">{onDetails&&<Button small primary onClick={()=>onDetails(rule.id)}>{t("seoRemediationTitle")}</Button>}<Button small onClick={() => onIssue(rule.id)}>{t("seoActionUrls")}<ArrowRight size={13}/></Button></div></div>
          </div>
        </li>)}
      </ol>
      <footer><p>{t("seoActionImpactNote")}</p>{actions.length > 3 && <Button small onClick={() => setShowAll(!showAll)} aria-expanded={showAll}>{t(showAll ? "seoActionShowTop" : "seoActionShowAll")}{!showAll && ` (${number(actions.length, lang)})`}</Button>}</footer>
    </> : <p className="seo-action-empty">{t(available ? "seoActionEmpty" : "seoActionNeedsScore")}</p>}
  </section>;
}
