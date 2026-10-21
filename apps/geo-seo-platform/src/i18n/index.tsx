// Translations: one JSON file per module and language (`tr/overview.json`), keys read as
// `module.key`. Turkish is the reference; a key missing in another language falls back to
// English, then to Turkish. Tests check every language has every key and every key in use exists.

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

export const LANGUAGES = [
  { code: "tr", name: "Türkçe", locale: "tr-TR" },
  { code: "en", name: "English", locale: "en-GB" },
  { code: "de", name: "Deutsch", locale: "de-DE" },
  { code: "es", name: "Español", locale: "es-ES" },
] as const;

export type Lang = (typeof LANGUAGES)[number]["code"];

export type Messages = Record<string, string>;

/** Messages by language from files named `./<lang>/<module>.json`. */
export function buildMessages(files: Record<string, Record<string, string>>): Record<string, Messages> {
  const messages: Record<string, Messages> = {};
  for (const [path, entries] of Object.entries(files)) {
    const match = /\.\/([a-z]{2})\/([a-z-]+)\.json$/.exec(path);
    if (!match) continue;
    const [, lang, module] = match;
    const target = (messages[lang!] ??= {});
    for (const [key, value] of Object.entries(entries)) target[`${module}.${key}`] = value;
  }
  return messages;
}

export const MESSAGES = buildMessages(
  import.meta.glob<Record<string, string>>("./*/*.json", { eager: true, import: "default" }),
);

/** A message with `{name}` placeholders filled. */
export function translate(lang: Lang, key: string, values?: Record<string, string | number>): string {
  const text = MESSAGES[lang]?.[key] ?? MESSAGES.en?.[key] ?? MESSAGES.tr?.[key] ?? key;
  if (!values) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in values ? String(values[name]) : whole));
}

const STORAGE = "platform.lang";

function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE);
    if (saved && LANGUAGES.some((language) => language.code === saved)) return saved as Lang;
  } catch {
    // Storage may be unavailable; the browser's language is used.
  }
  const wanted = typeof navigator === "undefined" ? "tr" : navigator.language.slice(0, 2);
  return (LANGUAGES.find((language) => language.code === wanted)?.code ?? "tr") as Lang;
}

interface I18n {
  lang: Lang;
  locale: string;
  setLang: (lang: Lang) => void;
  t: (key: string, values?: Record<string, string | number>) => string;
}

const Context = createContext<I18n | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(initialLang);
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  const setLang = useCallback((next: Lang) => {
    setLangState(next);
    try {
      localStorage.setItem(STORAGE, next);
    } catch {
      // Not remembered without storage.
    }
  }, []);
  const value = useMemo<I18n>(
    () => ({
      lang,
      locale: LANGUAGES.find((language) => language.code === lang)?.locale ?? "tr-TR",
      setLang,
      t: (key, values) => translate(lang, key, values),
    }),
    [lang, setLang],
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useI18n(): I18n {
  const value = useContext(Context);
  if (!value) throw new Error("useI18n outside I18nProvider");
  return value;
}
