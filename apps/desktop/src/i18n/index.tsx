// Minimal i18n: typed message keys, `{name}` placeholders, English and Turkish.

import { createContext, useCallback, useContext, useMemo, type ReactNode } from "react";

import { useStored } from "../lib/storage";
import { en, type MessageKey } from "./en";
import { tr, trTemplates } from "./tr";

export type Language = "en" | "tr";

const dictionaries: Record<Language, Record<MessageKey, string>> = { en, tr };

export type Translate = (key: MessageKey, values?: Record<string, string | number>) => string;

interface I18n {
  language: Language;
  locale: string;
  setLanguage: (language: Language) => void;
  t: Translate;
  /** Localized title and description of a report template (falls back to English). */
  template: (id: string, title: string, description: string) => { title: string; description: string };
}

const I18nContext = createContext<I18n | null>(null);

function initialLanguage(): Language {
  return navigator.language.toLowerCase().startsWith("tr") ? "tr" : "en";
}

export function interpolate(text: string, values?: Record<string, string | number>): string {
  if (!values) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in values ? String(values[name]) : match,
  );
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguage] = useStored<Language>("language", initialLanguage());
  const t = useCallback<Translate>(
    (key, values) => interpolate(dictionaries[language][key] ?? en[key] ?? key, values),
    [language],
  );
  const value = useMemo<I18n>(
    () => ({
      language,
      locale: language === "tr" ? "tr-TR" : "en-US",
      setLanguage,
      t,
      template: (id, title, description) =>
        language === "tr" && trTemplates[id] ? trTemplates[id] : { title, description },
    }),
    [language, setLanguage, t],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  const context = useContext(I18nContext);
  if (!context) throw new Error("useI18n outside I18nProvider");
  return context;
}
