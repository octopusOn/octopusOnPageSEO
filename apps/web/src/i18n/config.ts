// Languages. The CMS decides which languages the site has (its `languages` table, read through
// the API). The interface texts around the content (buttons, labels, table headings) exist for
// the languages below; a language added in the CMS shows them in English until it gets its own
// dictionary.

export const locales = ["en", "tr", "de", "es"] as const;
export type Locale = (typeof locales)[number];
export const defaultLocale: Locale = "en";

/** Languages assumed when the CMS cannot be reached (build without CMS, outage). */
export const fallbackLanguages = [
  { code: "en", name: "English", native_name: "English", locale_tag: "en-US", is_default: true },
  { code: "tr", name: "Turkish", native_name: "Türkçe", locale_tag: "tr-TR", is_default: false },
  { code: "de", name: "German", native_name: "Deutsch", locale_tag: "de-DE", is_default: false },
  { code: "es", name: "Spanish", native_name: "Español", locale_tag: "es-ES", is_default: false },
];

export const hasLocale = (value: string): value is Locale => (locales as readonly string[]).includes(value);

/** A URL language prefix: `tr`, `pt-br`. */
export const LANGUAGE_CODE = /^[a-z]{2,3}(-[a-z0-9]{2,8})?$/;

/** Cookie remembering an explicit language choice. */
export const LOCALE_COOKIE = "NEXT_LOCALE";
