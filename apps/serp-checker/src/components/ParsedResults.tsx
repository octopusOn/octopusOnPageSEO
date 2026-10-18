import { ExternalLink } from "lucide-react";
import type { Feature, Organic } from "../api/types";
import { hasKey, useI18n, type Key } from "../i18n";

/** Organic results and SERP features as a parser read them. */
export function ParsedResults({ organic, features }: { organic: Organic[]; features: Feature[] }) {
  const { t } = useI18n();
  return (
    <div className="parsed">
      {features.length > 0 && (
        <div className="oc-chips feature-chips">
          {features.map((feature, index) => {
            const key = `feature.${feature.kind}`;
            return (
              <span className="oc-chip" key={`${feature.kind}-${index}`} title={feature.items?.map((item) => item.text ?? item.url).join("\n")}>
                {hasKey(key) ? t(key as Key) : feature.kind}
                {(feature.items?.length ?? 0) > 1 && <span>{feature.items?.length}</span>}
              </span>
            );
          })}
        </div>
      )}
      <ol className="organic-list">
        {organic.map((result) => (
          <li key={`${result.position}-${result.url}`}>
            <span className="organic-position oc-num">{result.position}</span>
            <div>
              <a href={result.url} target="_blank" rel="noreferrer">
                {result.title || result.url}
                <ExternalLink size={11} />
              </a>
              <small>
                {result.displayed_url ?? result.url}
                {result.site_only && (
                  <span className="oc-badge" title={t("siteOnlyHelp")}>
                    {t("siteOnly")}
                  </span>
                )}
              </small>
              {result.snippet && <p>{result.snippet}</p>}
            </div>
          </li>
        ))}
      </ol>
    </div>
  );
}
