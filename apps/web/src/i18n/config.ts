// Supported languages: the most used languages of the web plus Turkish, our home market.

export const locales = ["en", "tr", "de", "es"] as const;
export type Locale = (typeof locales)[number];
export const defaultLocale: Locale = "en";

export const localeNames: Record<Locale, string> = {
  en: "English",
  tr: "Türkçe",
  de: "Deutsch",
  es: "Español",
};

/** BCP 47 tags for `<html lang>`, Open Graph and number/date formatting. */
export const localeTags: Record<Locale, string> = {
  en: "en-US",
  tr: "tr-TR",
  de: "de-DE",
  es: "es-ES",
};

export const hasLocale = (value: string): value is Locale => (locales as readonly string[]).includes(value);

/** Cookie remembering an explicit language choice. */
export const LOCALE_COOKIE = "NEXT_LOCALE";
