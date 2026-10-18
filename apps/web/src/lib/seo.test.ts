import { describe, expect, it } from "vitest";

import type { CmsPage, Language } from "./cms";
import { pageJsonLd } from "./jsonld";
import { pageMetadata } from "./seo";

const languages: Language[] = [
  { code: "en", name: "English", native_name: "English", locale_tag: "en-US", is_default: true },
  { code: "tr", name: "Turkish", native_name: "Türkçe", locale_tag: "tr-TR", is_default: false },
];

const page: CmsPage = {
  id: 2,
  page_id: 1,
  type: "page",
  product: "seo-crawler",
  icon: null,
  locale: "tr",
  path: "ozellikler",
  url: "/tr/ozellikler",
  title: "Özellikler",
  nav_title: null,
  excerpt: "Özet",
  status: "published",
  published_at: "2026-10-18T10:00:00+03:00",
  updated_at: "2026-10-18T10:00:00+03:00",
  seo: { title: "Tüm özellikler", description: "Açıklama", canonical: null, noindex: false, image: null },
  alternates: [
    { locale: "en", url: "/en/features" },
    { locale: "tr", url: "/tr/ozellikler" },
  ],
  x_default: "/en/features",
  breadcrumbs: [
    { title: "Ana sayfa", url: "/tr" },
    { title: "Özellikler", url: "/tr/ozellikler" },
  ],
  blocks: [{ id: "f", type: "faq", data: { title: "SSS", items: [{ question: "Ücretsiz mi?", answer: "Evet." }] } }],
  children: null,
};

describe("page metadata", () => {
  it("links every language version with translated slugs and x-default", () => {
    const meta = pageMetadata(page, languages);
    expect(meta.title).toBe("Tüm özellikler");
    expect(meta.alternates?.canonical).toBe("/tr/ozellikler");
    expect(meta.alternates?.languages).toEqual({ en: "/en/features", tr: "/tr/ozellikler", "x-default": "/en/features" });
    expect(meta.openGraph).toMatchObject({ locale: "tr_TR", url: "/tr/ozellikler" });
    expect(meta.robots).toBeUndefined();
  });

  it("keeps the home title whole, honours noindex and canonical, and keeps previews out", () => {
    expect(pageMetadata({ ...page, type: "home" }, languages).title).toEqual({ absolute: "Tüm özellikler" });
    const hidden = pageMetadata({ ...page, seo: { ...page.seo, noindex: true, canonical: "https://example.com/x" } }, languages);
    expect(hidden.robots).toEqual({ index: false, follow: true });
    expect(hidden.alternates?.canonical).toBe("https://example.com/x");
    expect(pageMetadata(page, languages, 1, true).robots).toEqual({ index: false, follow: false });
  });

  it("gives later listing pages their own canonical address", () => {
    const meta = pageMetadata({ ...page, type: "category" }, languages, 3);
    expect(meta.alternates?.canonical).toBe("/tr/ozellikler/page/3");
    expect(meta.alternates?.languages).toBeUndefined();
  });
});

describe("structured data", () => {
  it("describes breadcrumbs, the program and the FAQ", () => {
    const types = pageJsonLd(page, null).map((thing) => thing["@type"]);
    expect(types).toEqual(["BreadcrumbList", "SoftwareApplication", "FAQPage"]);
  });
});
