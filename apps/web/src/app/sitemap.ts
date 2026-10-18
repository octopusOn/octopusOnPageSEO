import type { MetadataRoute } from "next";

import { getSitemap } from "@/lib/cms";
import { site } from "@/lib/site";

export const revalidate = 3600;

const absolute = (url: string) => `${site.url}${url}`;

/** Every published, indexable page of the CMS in every language, with its language versions. */
export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  try {
    const { default_locale: defaultLocale, urls } = await getSitemap();
    return urls.map((entry) => {
      const languages: Record<string, string> = Object.fromEntries(Object.entries(entry.alternates).map(([code, url]) => [code, absolute(url)]));
      const fallback = entry.alternates[defaultLocale];
      if (fallback) languages["x-default"] = absolute(fallback);
      return {
        url: absolute(entry.url),
        lastModified: entry.updated_at ?? undefined,
        changeFrequency: entry.type === "post" || entry.type === "category" ? "weekly" : "monthly",
        priority: entry.type === "home" ? 1 : 0.7,
        alternates: { languages },
      };
    });
  } catch {
    // The CMS is unreachable: an empty sitemap until the next revalidation.
    return [];
  }
}
