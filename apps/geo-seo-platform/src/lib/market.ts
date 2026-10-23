// Markets a report is asked for: a country and a language, the project's unless another is
// chosen. The address carries another one as `market=country-language` (`de-de`).

/** Countries agencies work with most; the platform knows their DataForSEO locations. */
export const COUNTRIES = [
  "tr", "us", "gb", "de", "at", "ch", "nl", "be", "fr", "es", "pt", "it", "gr", "bg", "ro", "pl", "se", "no", "dk", "fi",
  "ie", "ca", "au", "az", "cy", "ru", "ua", "kz", "uz", "by", "ge", "ae", "sa", "qa", "eg", "il", "in", "br", "mx", "jp",
] as const;

export const LANGUAGE_CODES = ["tr", "en", "de", "es", "fr", "it", "pt", "nl", "ru", "ar", "pl", "sv", "no", "da", "fi", "el", "bg", "ro", "uk", "ja", "az", "kk", "uz"] as const;

/** The language most searches of a country are in. */
const MAIN_LANGUAGE: Record<string, string> = {
  tr: "tr", us: "en", gb: "en", ie: "en", ca: "en", au: "en", in: "en", de: "de", at: "de", ch: "de", nl: "nl", be: "nl",
  fr: "fr", es: "es", mx: "es", pt: "pt", br: "pt", it: "it", gr: "el", cy: "el", bg: "bg", ro: "ro", pl: "pl", se: "sv",
  no: "no", dk: "da", fi: "fi", az: "az", ru: "ru", ua: "uk", kz: "ru", uz: "uz", by: "ru", ge: "ru", ae: "ar", sa: "ar",
  qa: "ar", eg: "ar", il: "en", jp: "ja",
};

export interface Market {
  country: string;
  language: string;
}

/** The language a country is searched in most, English when unknown. */
export function mainLanguage(country: string): string {
  return MAIN_LANGUAGE[country] ?? "en";
}

/** A market from `country-language`, or nothing when it is not one. */
export function parseMarket(text: string | undefined): Market | null {
  const match = /^([a-z]{2})-([a-z]{2,3})$/.exec((text ?? "").trim().toLowerCase());
  return match ? { country: match[1]!, language: match[2]! } : null;
}

/** The market a report is asked for: the address's, else the project's. */
export function marketOf(project: Market, param: string | undefined): Market {
  return parseMarket(param) ?? { country: project.country, language: project.language };
}

/** The address's `market` for a choice: nothing when it is the project's own. */
export function marketParam(project: Market, chosen: Market): string | undefined {
  return chosen.country === project.country && chosen.language === project.language ? undefined : `${chosen.country}-${chosen.language}`;
}
