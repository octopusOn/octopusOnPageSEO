// Structured data of a CMS page: breadcrumbs, the program it presents, its FAQ and, for posts,
// the article.

import type { CmsPage } from "./cms";
import { list } from "./cms";
import type { Release } from "./releases";
import { findProduct, site } from "./site";

type Thing = Record<string, unknown>;

const absolute = (url: string) => (url.startsWith("http") ? url : `${site.url}${url}`);

export function pageJsonLd(page: CmsPage, release: Release | null): Thing[] {
  const out: Thing[] = [];
  if (page.type !== "home" && page.breadcrumbs.length > 1) {
    out.push({
      "@context": "https://schema.org",
      "@type": "BreadcrumbList",
      itemListElement: page.breadcrumbs.map((crumb, index) => ({
        "@type": "ListItem",
        position: index + 1,
        name: crumb.title,
        item: absolute(crumb.url),
      })),
    });
  }
  const product = findProduct(page.product);
  if (product) {
    out.push({
      "@context": "https://schema.org",
      "@type": "SoftwareApplication",
      name: product.name,
      applicationCategory: "BusinessApplication",
      operatingSystem: "Windows, macOS, Linux",
      description: page.seo.description ?? undefined,
      url: absolute(page.url),
      offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
      publisher: { "@type": "Organization", name: site.name, url: site.url },
      ...(release ? { softwareVersion: release.version } : {}),
    });
  }
  const faq = page.blocks.filter((block) => block.type === "faq").flatMap((block) => list<{ question: string; answer: string }>(block.data, "items"));
  if (faq.length > 0) {
    out.push({
      "@context": "https://schema.org",
      "@type": "FAQPage",
      mainEntity: faq.map((item) => ({ "@type": "Question", name: item.question, acceptedAnswer: { "@type": "Answer", text: item.answer } })),
    });
  }
  if (page.type === "post") {
    out.push({
      "@context": "https://schema.org",
      "@type": "BlogPosting",
      headline: page.title,
      description: page.seo.description ?? undefined,
      datePublished: page.published_at ?? undefined,
      dateModified: page.updated_at ?? undefined,
      inLanguage: page.locale,
      mainEntityOfPage: absolute(page.url),
      ...(page.seo.image ? { image: page.seo.image.url } : {}),
      publisher: { "@type": "Organization", name: site.name, url: site.url },
    });
  }
  if (page.type === "home") {
    out.push({ "@context": "https://schema.org", "@type": "Organization", name: site.name, url: site.url, logo: `${site.url}/icon.svg` });
  }
  return out;
}

/** JSON for a `<script type="application/ld+json">`, safe inside HTML. */
export const jsonLdScript = (things: Thing[]) => JSON.stringify(things.length === 1 ? things[0] : things).replace(/</g, "\\u003c");
