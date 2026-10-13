import type { MetadataRoute } from "next";

import { locales } from "@/i18n/config";
import { site } from "@/lib/site";

const PAGES = ["", "/features", "/download", "/docs", "/changelog"];

export default function sitemap(): MetadataRoute.Sitemap {
  return PAGES.flatMap((path) =>
    locales.map((locale) => ({
      url: `${site.url}/${locale}${path}`,
      changeFrequency: path === "/changelog" || path === "/download" ? ("weekly" as const) : ("monthly" as const),
      priority: path === "" ? 1 : 0.7,
      alternates: {
        languages: Object.fromEntries(locales.map((other) => [other, `${site.url}/${other}${path}`])),
      },
    })),
  );
}
