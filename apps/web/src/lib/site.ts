// Site-wide settings, from the environment (see .env.example), and the products.

import productList from "./products.json";

/** The open-source repository of every program's code; set the variable to `none` to disable GitHub. */
const repo = process.env.OCTOPUSSEO_GITHUB_REPO?.trim() || "octopusOn/octopusOnPageSEO";

export const site = {
  name: "OctopusOn SEO",
  url: (process.env.OCTOPUSSEO_SITE_URL?.trim() || "https://octopuson.com").replace(/\/$/, ""),
  /** `owner/name` of the source repository; `null` when GitHub is disabled. */
  repo: repo && /^[\w.-]+\/[\w.-]+$/.test(repo) ? repo : null,
  email: process.env.OCTOPUSSEO_CONTACT_EMAIL?.trim() || null,
} as const;

export const githubUrl = (path = "") => (site.repo ? `https://github.com/${site.repo}${path}` : null);

/** A program's own GitHub project (its releases, stars and issues). */
export const programUrl = (product: Product, path = "") => (site.repo ? `https://github.com/${product.repo}${path}` : null);

export type ProductId = "seo-crawler" | "log-analyser" | "serp-checker" | "seo-keyword-research" | "index-checker";

export interface Product {
  id: ProductId;
  /** Short key: the CLI suffix, the tag prefix and the commit scope (`crawler`). */
  key: string;
  /** Full name, as installers and release titles show it. */
  name: string;
  /** Name within the OctopusOn SEO family. */
  short: string;
  /** Monorepo tags of its releases start with this, followed by the version (`crawler-v0.2.0`). */
  tagPrefix: string;
  /** Prefixes of releases tagged before the current scheme (`v0.1.0` of the Log Analyser). */
  legacyTagPrefixes?: string[];
  /** Command-line program. */
  cli: string;
  /** Cargo package that builds the command-line program. */
  package: string;
  /** Folder of the desktop app (its `src-tauri/tauri.conf.json` names the installers). */
  app: string;
  /** The program's own GitHub project, where its releases are published (`octopusOn/seo-crawler`). */
  repo: string;
  /** The program's window ground in @octopusseo/design (`data-product`); `log` is the default violet. */
  ground: string;
  /** Folders whose commits belong to this program's release notes (scripts/release.mjs). */
  paths: string[];
}

/** Every OctopusOn SEO program, shared with scripts/collect-downloads.mjs, scripts/release.mjs and the release workflow. */
export const products = productList as Product[];

export function productById(id: ProductId): Product {
  return products.find((product) => product.id === id) ?? products[0]!;
}

/** The product with this id, or null for a program the site has no releases of. */
export function findProduct(id: string | null | undefined): Product | null {
  return products.find((product) => product.id === id) ?? null;
}

/**
 * The product and version of a release tag: the longest matching prefix followed by a version
 * (`log-v0.2.0` and the older `v0.1.0` are the Log Analyser, `spider-v0.1.0` the Spider), or
 * `null`.
 */
export function parseTag(tag: string, within?: Product): { product: Product; version: string } | null {
  // In a program's own repository the tags are plain versions (`v0.2.0`).
  if (within) {
    const prefix = [within.tagPrefix, ...(within.legacyTagPrefixes ?? []), "v", ""].find((p) => tag.startsWith(p) && /^\d/.test(tag.slice(p.length)));
    return prefix === undefined ? null : { product: within, version: tag.slice(prefix.length) };
  }
  const prefixes = products.flatMap((product) => [product.tagPrefix, ...(product.legacyTagPrefixes ?? [])].map((prefix) => ({ product, prefix })));
  const match = prefixes
    .sort((a, b) => b.prefix.length - a.prefix.length)
    .find(({ prefix }) => tag.startsWith(prefix) && /^\d/.test(tag.slice(prefix.length)));
  return match ? { product: match.product, version: tag.slice(match.prefix.length) } : null;
}

export function productOfTag(tag: string): Product | null {
  return parseTag(tag)?.product ?? null;
}
