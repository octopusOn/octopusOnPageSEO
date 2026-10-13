"use client";

import { useEffect, useRef, useState } from "react";
import { usePathname } from "next/navigation";
import { Check, Globe } from "lucide-react";

import { LOCALE_COOKIE, localeNames, locales, type Locale } from "@/i18n/config";

/** Switches the language of the current page and remembers the choice. */
export function LanguageSwitcher({ current, label }: { current: Locale; label: string }) {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;
    const close = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => event.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  const hrefFor = (locale: Locale) => pathname.replace(/^\/[a-z]{2}(?=\/|$)/, `/${locale}`);
  const remember = (locale: Locale) => {
    document.cookie = `${LOCALE_COOKIE}=${locale}; path=/; max-age=31536000; samesite=lax`;
  };

  return (
    <div className="lang" ref={root}>
      <button
        type="button"
        className="lang-button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={label}
        onClick={() => setOpen(!open)}
      >
        <Globe size={16} aria-hidden="true" />
        <span>{current.toUpperCase()}</span>
      </button>
      {open && (
        <ul className="lang-menu" role="listbox" aria-label={label}>
          {locales.map((locale) => (
            <li key={locale} role="option" aria-selected={locale === current}>
              <a href={hrefFor(locale)} hrefLang={locale} lang={locale} onClick={() => remember(locale)}>
                <span>{localeNames[locale]}</span>
                {locale === current && <Check size={15} aria-hidden="true" />}
              </a>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
