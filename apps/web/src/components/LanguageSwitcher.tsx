"use client";

import { useEffect, useRef, useState } from "react";
import { usePathname } from "next/navigation";
import { Check, Globe } from "lucide-react";

import { LOCALE_COOKIE } from "@/i18n/config";
import type { Language } from "@/lib/cms";

/**
 * Switches to the current page in another language and remembers the choice. Translated pages
 * have their own slugs (`/en/features` ↔ `/tr/ozellikler`), so the targets come from the page's
 * hreflang links; a language without this page leads to its home page.
 */
export function LanguageSwitcher({ current, label, languages }: { current: string; label: string; languages: Language[] }) {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const [targets, setTargets] = useState<Record<string, string>>({});
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const found: Record<string, string> = {};
    document.querySelectorAll<HTMLLinkElement>('link[rel="alternate"][hreflang]').forEach((link) => {
      const code = link.hreflang;
      if (code === "x-default") return;
      try {
        found[code] = new URL(link.href).pathname;
      } catch {
        // Ignore malformed links.
      }
    });
    setTargets(found);
  }, [pathname, open]);

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

  const remember = (code: string) => {
    document.cookie = `${LOCALE_COOKIE}=${code}; path=/; max-age=31536000; samesite=lax`;
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
          {languages.map((language) => (
            <li key={language.code} role="option" aria-selected={language.code === current}>
              <a
                href={language.code === current ? pathname : (targets[language.code] ?? `/${language.code}`)}
                hrefLang={language.code}
                lang={language.code}
                onClick={() => remember(language.code)}
              >
                <span>{language.native_name}</span>
                {language.code === current && <Check size={15} aria-hidden="true" />}
              </a>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
