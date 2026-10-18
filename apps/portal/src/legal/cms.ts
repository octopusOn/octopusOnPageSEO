// The legal texts, edited in the CMS (pages of type "legal", apps/cms) and read through its API.
// Answers are kept for five minutes; when the CMS cannot be reached, the last answer is used.

import { cmsUrl } from "@/i18n/languages";

export interface LegalSummary {
  /** The same in every language: "privacy", "distance-sales"… */
  key: string;
  /** The language of the text, which differs from the one asked for when it has no translation. */
  locale: string;
  slug: string;
  title: string;
  /** The short name for menus. */
  label: string;
  updated_at: string | null;
}

export interface LegalDocument extends LegalSummary {
  excerpt: string | null;
  alternates: { locale: string; slug: string }[];
  blocks: { type: string; data: Record<string, unknown> }[];
}

const TTL = 5 * 60_000;
const cache = new Map<string, { at: number; value: unknown }>();

async function cmsJson<T>(path: string): Promise<T | null | undefined> {
  const hit = cache.get(path);
  if (hit && Date.now() - hit.at < TTL) return hit.value as T | null;
  try {
    const response = await fetch(`${cmsUrl()}/api/v1/${path}`, { signal: AbortSignal.timeout(3000), headers: { Accept: "application/json" }, cache: "no-store" });
    if (response.status === 404) {
      cache.set(path, { at: Date.now(), value: null });
      return null;
    }
    if (!response.ok) throw new Error(`CMS answered ${response.status}`);
    const value = ((await response.json()) as { data: T }).data;
    cache.set(path, { at: Date.now(), value });
    return value;
  } catch {
    // Unreachable: the last answer, or `undefined` when there is none.
    return hit?.value as T | null | undefined;
  }
}

/** Every legal text in `locale`, in the CMS's order; empty when the CMS cannot be reached. */
export async function legalTexts(locale: string): Promise<LegalSummary[]> {
  return (await cmsJson<LegalSummary[]>(`legal?locale=${encodeURIComponent(locale)}`)) ?? [];
}

/** One text by its slug or key: null when there is none, undefined when the CMS cannot be reached. */
export async function legalText(locale: string, slug: string): Promise<LegalDocument | null | undefined> {
  return cmsJson<LegalDocument>(`legal/${encodeURIComponent(slug)}?locale=${encodeURIComponent(locale)}`);
}

/** The Markdown of a text: its text blocks, one after the other. */
export function legalMarkdown(document: LegalDocument): string {
  return document.blocks
    .filter((block) => block.type === "rich_text" && typeof block.data.body === "string")
    .map((block) => block.data.body as string)
    .join("\n\n");
}

/** The slug of the text with `key` in a list, or the key itself (the legal page redirects it). */
export function legalSlug(texts: LegalSummary[], key: string): string {
  return texts.find((text) => text.key === key)?.slug ?? key;
}
