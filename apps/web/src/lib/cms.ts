// Content from the OctopusOn SEO CMS (apps/cms): languages, menus and pages with their blocks.
// Every request is cached under the `cms` tag. The admin expires that tag through
// /api/revalidate after each change, and cached data also expires hourly in case a call is
// lost. In draft mode (see /api/preview) requests skip the cache and include drafts.

import { cache } from "react";
import { draftMode } from "next/headers";

export const CMS_TAG = "cms";
const REVALIDATE = 3600;

/** Base URL of the CMS, e.g. `http://localhost:8000`. */
export function cmsUrl(): string {
  return (process.env.CMS_API_URL?.trim() || "http://localhost:8000").replace(/\/$/, "");
}

export interface Language {
  code: string;
  name: string;
  native_name: string;
  /** BCP 47 tag for <html lang>, Open Graph and number formats: `tr-TR`. */
  locale_tag: string;
  is_default: boolean;
}

export interface NavLink {
  label: string;
  url: string;
  product: string | null;
}

export type FooterColumn = "product" | "resources" | "company";

export interface Navigation {
  /** The account site: credits and API connections (my.octopuson.com). */
  account: string;
  header: NavLink[];
  footer: Record<FooterColumn, NavLink[]>;
  /** Every program's page, for the header's program menu. */
  programs: ProductCard[];
  /** Page with every program's downloads, for the header button. */
  download: string | null;
}

export interface Site {
  languages: Language[];
  navigation: Navigation;
}

export interface MediaFile {
  id: number;
  url: string;
  alt: string | null;
  width: number | null;
  height: number | null;
  mime: string;
}

/** A page that presents a program, as product cards and download sections show it. */
export interface ProductCard {
  product: string;
  name: string;
  title: string;
  excerpt: string | null;
  url: string;
  icon: string | null;
}

export interface Block {
  id: string | null;
  type: string;
  data: Record<string, unknown>;
  /** Filled for `products` and `downloads` blocks. */
  products?: ProductCard[];
}

export interface ChildPage {
  title: string;
  excerpt: string | null;
  url: string;
  type: string;
  icon: string | null;
  product: string | null;
  published_at: string | null;
  image: MediaFile | null;
}

export interface Listing {
  items: ChildPage[];
  page: number;
  last_page: number;
  total: number;
  base_url: string;
}

export interface CmsPage {
  id: number;
  page_id: number;
  type: "home" | "page" | "category" | "product" | "post" | "doc";
  product: string | null;
  icon: string | null;
  locale: string;
  path: string;
  url: string;
  title: string;
  nav_title: string | null;
  excerpt: string | null;
  status: "draft" | "published";
  published_at: string | null;
  updated_at: string | null;
  seo: {
    title: string;
    description: string | null;
    canonical: string | null;
    noindex: boolean;
    image: MediaFile | null;
  };
  alternates: { locale: string; url: string }[];
  x_default: string | null;
  breadcrumbs: { title: string; url: string }[];
  blocks: Block[];
  children: Listing | null;
}

export type Resolved = { page: CmsPage } | { redirect: { to: string; status: number } } | null;

export interface SitemapEntry {
  url: string;
  locale: string;
  type: string;
  updated_at: string | null;
  alternates: Record<string, string>;
}

async function preview(): Promise<boolean> {
  try {
    return (await draftMode()).isEnabled;
  } catch {
    // Outside a request (build steps, sitemap generation).
    return false;
  }
}

async function request<T>(path: string, drafts = false): Promise<T | null> {
  const secret = process.env.CMS_PREVIEW_SECRET?.trim();
  const headers: Record<string, string> = { Accept: "application/json" };
  const init: RequestInit =
    drafts && secret
      ? { headers: { ...headers, Authorization: `Bearer ${secret}` }, cache: "no-store" }
      : { headers, next: { revalidate: REVALIDATE, tags: [CMS_TAG] } };
  const response = await fetch(`${cmsUrl()}/api/v1/${path}`, init);
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`CMS ${path}: HTTP ${response.status}`);
  return (await response.json()) as T;
}

/** Active languages, the default first. */
export const getLanguages = cache(async (): Promise<Language[]> => {
  const json = await request<{ data: Language[] }>("languages");
  return json?.data ?? [];
});

/** Languages and the menus of one language. */
export const getSite = cache(async (locale: string): Promise<Site> => {
  const json = await request<{ data: Site }>(`site?${new URLSearchParams({ locale })}`, await preview());
  if (!json) throw new Error("CMS site: no data");
  return json.data;
});

/** The page at `/{locale}/{path}`, a redirect for an old address, or null. */
export const resolvePage = cache(async (locale: string, path: string): Promise<Resolved> => {
  const drafts = await preview();
  const json = await request<{ data?: CmsPage; redirect?: { to: string; status: number } }>(
    `pages/resolve?${new URLSearchParams({ locale, path })}`,
    drafts,
  );
  if (!json) return null;
  if (json.redirect) return { redirect: json.redirect };
  return json.data ? { page: json.data } : null;
});

export async function getSitemap(): Promise<{ default_locale: string; urls: SitemapEntry[] }> {
  const json = await request<{ data: { default_locale: string; urls: SitemapEntry[] } }>("sitemap");
  return json?.data ?? { default_locale: "en", urls: [] };
}

/** `/tr/blog/page/2` → `["blog", "page", "2"]` → `blog/page/2`. */
export function joinPath(slug: string[] | undefined): string {
  return (slug ?? []).map((part) => decodeURIComponent(part)).join("/");
}

/** Typed access to a block's data. */
export function text(data: Record<string, unknown>, key: string): string {
  const value = data[key];
  return typeof value === "string" ? value : "";
}

export function list<T = string>(data: Record<string, unknown>, key: string): T[] {
  const value = data[key];
  return Array.isArray(value) ? (value as T[]) : [];
}

export function flag(data: Record<string, unknown>, key: string): boolean {
  return data[key] === true;
}

export function media(data: Record<string, unknown>, key: string): MediaFile | null {
  const value = data[key];
  return value && typeof value === "object" && "url" in value ? (value as MediaFile) : null;
}
