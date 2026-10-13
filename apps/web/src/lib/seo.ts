import type { Metadata } from "next";

import { locales, localeTags, type Locale } from "@/i18n/config";
import { site } from "./site";

/** Title, description, canonical URL, hreflang alternates and social cards of a page. */
export function pageMetadata(lang: Locale, path: string, title: string, description: string): Metadata {
  const languages: Record<string, string> = Object.fromEntries(locales.map((locale) => [locale, `/${locale}${path}`]));
  languages["x-default"] = `/en${path}`;
  return {
    title,
    description,
    alternates: { canonical: `/${lang}${path}`, languages },
    openGraph: {
      title,
      description,
      url: `/${lang}${path}`,
      siteName: site.name,
      locale: localeTags[lang].replace("-", "_"),
      type: "website",
    },
    twitter: { card: "summary_large_image", title, description },
  };
}
