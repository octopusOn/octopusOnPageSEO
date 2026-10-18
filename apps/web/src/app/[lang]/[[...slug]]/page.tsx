// Every page of the site: `/{lang}/{path}` is looked up in the CMS, which answers with the page
// (rendered from its blocks), a redirect for an old address, or nothing (404).

import type { Metadata } from "next";
import { draftMode } from "next/headers";
import { notFound, permanentRedirect, redirect } from "next/navigation";

import { Blocks } from "@/components/blocks/Blocks";
import { getDictionary } from "@/i18n";
import { getSite, getSitemap, joinPath, resolvePage } from "@/lib/cms";
import { pageJsonLd, jsonLdScript } from "@/lib/jsonld";
import { latestRelease } from "@/lib/releases";
import { pageMetadata } from "@/lib/seo";
import { findProduct } from "@/lib/site";

export const revalidate = 3600;

/** Pages known at build time; pages published later render on their first visit. */
export async function generateStaticParams({ params }: { params: { lang: string } }) {
  try {
    const { urls } = await getSitemap();
    return urls
      .filter((entry) => entry.locale === params.lang)
      .map((entry) => ({ slug: entry.url.split("/").slice(2).filter(Boolean) }));
  } catch {
    return [];
  }
}

async function load(params: PageProps<"/[lang]/[[...slug]]">["params"]) {
  const { lang, slug } = await params;
  const path = joinPath(slug);
  const listingPage = Number(/(?:^|\/)page\/(\d+)$/.exec(path)?.[1] ?? 1);
  return { lang, path, listingPage, result: await resolvePage(lang, path) };
}

export async function generateMetadata({ params }: PageProps<"/[lang]/[[...slug]]">): Promise<Metadata> {
  const { lang, listingPage, result } = await load(params);
  if (!result || "redirect" in result) return {};
  const [site, preview] = await Promise.all([getSite(lang), draftMode()]);
  const page = result.page;
  return pageMetadata(page, site.languages, page.children ? listingPage : 1, preview.isEnabled);
}

export default async function CmsPage({ params }: PageProps<"/[lang]/[[...slug]]">) {
  const { lang, result } = await load(params);
  if (!result) notFound();
  if ("redirect" in result) {
    if (result.redirect.status === 301 || result.redirect.status === 308) permanentRedirect(result.redirect.to);
    redirect(result.redirect.to);
  }
  const page = result.page;
  const site = await getSite(lang);
  const language = site.languages.find((l) => l.code === lang);
  const product = findProduct(page.product);
  const release = product ? (await latestRelease(product.id)).release : null;
  const jsonLd = pageJsonLd(page, release);
  return (
    // A program's page takes the program's own ground (see blocks.css).
    <div className="product-ground" data-product={product?.ground}>
      {jsonLd.length > 0 && <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLdScript(jsonLd) }} />}
      <Blocks
        ctx={{
          page,
          lang,
          tag: language?.locale_tag ?? lang,
          dict: getDictionary(lang),
          downloadUrl: site.navigation.download,
          programs: site.navigation.programs,
          accountUrl: site.navigation.account,
        }}
      />
    </div>
  );
}
