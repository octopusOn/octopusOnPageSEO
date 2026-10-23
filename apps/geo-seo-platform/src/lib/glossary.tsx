// What each number means, in the reader's language: a tip beside labels and column heads.

import { Tip } from "@octopusseo/ui";
import { useI18n } from "../i18n";

export const TERMS = [
  "volume",
  "kd",
  "cpc",
  "competition",
  "intent",
  "traffic",
  "traffic_value",
  "organic_keywords",
  "position",
  "oa",
  "backlinks",
  "referring_domains",
  "dofollow",
  "spam_score",
  "anchor",
  "visibility",
  "average_position",
  "share_of_voice",
  "serp_features",
  "ai_overview",
  "mention_rate",
  "citation_rate",
  "clicks",
  "impressions",
  "ctr",
  "health",
  "geo_readiness",
  "top3_top10",
  "paid_traffic",
  "new_lost",
] as const;

export type Term = (typeof TERMS)[number];

/** The explanation of a term. */
export function useGlossary(): (term: Term) => string {
  const { t } = useI18n();
  return (term) => t(`glossary.${term}`);
}

/** A label with its explanation beside it. */
export function Explained({ term, label }: { term: Term; label: string }) {
  const { t } = useI18n();
  return (
    <span className="explained">
      {label}
      <Tip text={t(`glossary.${term}`)} label={t("common.whatIs", { label })} />
    </span>
  );
}
