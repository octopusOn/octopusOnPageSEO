// Another market than the project's for a report: country and language. The choice lives in the
// address (`market=de-de`), so links and the back button keep it.

import { Globe2, RotateCcw } from "lucide-react";
import { useI18n } from "../i18n";
import { useShell } from "./context";
import { COUNTRIES, LANGUAGE_CODES, mainLanguage, marketOf, marketParam, type Market } from "./market";

export function MarketPicker({ project }: { project: Market }) {
  const { t, locale } = useI18n();
  const { params, go } = useShell();
  const chosen = marketOf(project, params.market);
  const own = marketParam(project, chosen) === undefined;
  const regions = new Intl.DisplayNames([locale], { type: "region" });
  const languages = new Intl.DisplayNames([locale], { type: "language" });
  const choose = (next: Market) => go({ params: { ...params, market: marketParam(project, next) } });
  const countries: string[] = [...COUNTRIES];
  if (!countries.includes(chosen.country)) countries.unshift(chosen.country);
  const codes: string[] = [...LANGUAGE_CODES];
  if (!codes.includes(chosen.language)) codes.unshift(chosen.language);
  return (
    <div className={`market-picker${own ? "" : " other"}`} role="group" aria-label={t("query.market")}>
      <Globe2 size={15} aria-hidden="true" />
      <select
        className="oc-select"
        value={chosen.country}
        aria-label={t("query.marketCountry")}
        onChange={(event) => {
          const country = event.target.value;
          // A new country is searched in its own language unless it is the project's.
          const language = country === project.country ? project.language : mainLanguage(country);
          choose({ country, language });
        }}
      >
        {countries.map((code) => (
          <option key={code} value={code}>
            {regions.of(code.toUpperCase()) ?? code}
          </option>
        ))}
      </select>
      <select className="oc-select" value={chosen.language} aria-label={t("query.marketLanguage")} onChange={(event) => choose({ ...chosen, language: event.target.value })}>
        {codes.map((code) => (
          <option key={code} value={code}>
            {languages.of(code) ?? code}
          </option>
        ))}
      </select>
      {!own && (
        <button type="button" className="oc-btn small icon ghost" title={t("query.marketReset")} aria-label={t("query.marketReset")} onClick={() => choose(project)}>
          <RotateCcw size={14} aria-hidden="true" />
        </button>
      )}
    </div>
  );
}
