export type Locale = "tr" | "en";
export const LOCALE_COOKIE = "lang";

export function pickLocale(cookie: string | undefined, acceptLanguage: string | null): Locale {
  if (cookie === "tr" || cookie === "en") return cookie;
  const first = (acceptLanguage ?? "").split(",")[0]?.trim().toLowerCase() ?? "";
  return first.startsWith("en") ? "en" : "tr";
}
