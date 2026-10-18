// The language menu: the same page in every language the portal offers. Pages whose address
// depends on content (the legal texts) pass their own addresses.

import { Globe } from "lucide-react";

import { currentRoute, getDictionary } from "@/i18n";
import { messagesFor } from "@/i18n/catalog";
import { languages } from "@/i18n/languages";
import { routeUrl } from "@/i18n/routes";

export async function LanguageMenu({ hrefs, className = "" }: { hrefs?: Record<string, string>; className?: string }) {
  const [{ locale, t, languages: list }, route, { fallback }] = await Promise.all([getDictionary(), currentRoute(), languages()]);
  if (list.length < 2) return null;
  const current = list.find((language) => language.code === locale);
  const href = (code: string) =>
    hrefs?.[code] ?? (route ? routeUrl(code, messagesFor(code, fallback).routes, route.route, route.params) : `/${code}`);
  return (
    <details className={`lang-menu ${className}`}>
      <summary className="oc-btn ghost small" aria-label={t.nav.language}>
        <Globe size={14} />
        {current?.name ?? locale}
      </summary>
      <ul>
        {list.map((language) => (
          <li key={language.code}>
            <a href={href(language.code)} hrefLang={language.tag} lang={language.tag} aria-current={language.code === locale ? "true" : undefined}>
              {language.name}
            </a>
          </li>
        ))}
      </ul>
    </details>
  );
}
