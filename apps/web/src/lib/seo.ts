import type { Metadata } from "next";

import type { CmsPage, Language } from "./cms";
import { site } from "./site";

/**
 * Title, description, canonical URL, hreflang alternates (the page's published language
 * versions, x-default the default language's) and social cards of a CMS page. `listingPage`
 * is set on `<category>/page/N`.
 */
export function pageMetadata(page: CmsPage, languages: Language[], listingPage = 1, preview = false): Metadata {
  const tag = languages.find((language) => language.code === page.locale)?.locale_tag ?? page.locale;
  const alternates: Record<string, string> = Object.fromEntries(page.alternates.map((a) => [a.locale, a.url]));
  if (page.x_default) alternates["x-default"] = page.x_default;
  const paged = listingPage > 1;
  const url = paged ? `${page.url}/page/${listingPage}` : page.url;
  const title = paged ? `${page.seo.title} (${listingPage})` : page.seo.title;
  const description = page.seo.description ?? undefined;
  const image = page.seo.image;
  return {
    title: page.type === "home" ? { absolute: title } : title,
    description,
    alternates: {
      canonical: paged ? url : page.seo.canonical || page.url,
      // Later listing pages have no language versions of their own.
      languages: paged ? undefined : alternates,
    },
    robots: page.seo.noindex || preview ? { index: false, follow: !preview } : undefined,
    openGraph: {
      title,
      description,
      url,
      siteName: site.name,
      locale: tag.replace("-", "_"),
      type: page.type === "post" ? "article" : "website",
      ...(page.type === "post" && page.published_at ? { publishedTime: page.published_at, modifiedTime: page.updated_at ?? undefined } : {}),
      ...(image ? { images: [{ url: image.url, width: image.width ?? undefined, height: image.height ?? undefined, alt: image.alt ?? undefined }] } : {}),
    },
    twitter: { card: "summary_large_image", title, description, ...(image ? { images: [image.url] } : {}) },
  };
}
