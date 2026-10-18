// The portal's languages: those the CMS lists as active that also have a messages file, in the
// CMS's order, with the CMS's default language first in line. Cached for a minute; when the CMS
// cannot be reached, every messages file is offered and the last known default stays.

import { catalog } from "./catalog";

export interface Language {
  code: string;
  /** The language's own name: "Türkçe", "Deutsch". */
  name: string;
  /** BCP 47 tag for <html lang> and number formats: "tr-TR". */
  tag: string;
}

export interface Languages {
  list: Language[];
  /** The language for people who have not chosen one. */
  fallback: string;
}

interface CmsLanguage {
  code: string;
  native_name: string;
  locale_tag: string;
  is_default: boolean;
}

/** Base URL of the CMS (apps/cms), which also serves the legal texts. */
export function cmsUrl(): string {
  return (process.env.CMS_API_URL?.trim() || "http://localhost:8000").replace(/\/+$/, "");
}

let cached: { value: Languages; at: number } | undefined;
let lastDefault: string | undefined;

export async function languages(): Promise<Languages> {
  if (cached && Date.now() - cached.at < 60_000) return cached.value;
  const files = catalog();
  let cms: CmsLanguage[] = [];
  try {
    const response = await fetch(`${cmsUrl()}/api/v1/languages`, { signal: AbortSignal.timeout(2000), headers: { Accept: "application/json" }, cache: "no-store" });
    if (response.ok) cms = ((await response.json()) as { data?: CmsLanguage[] }).data ?? [];
  } catch {
    // The CMS is down: offer every messages file below.
  }
  let list: Language[] = cms
    .filter((language) => files.has(language.code))
    .map((language) => ({ code: language.code, name: language.native_name, tag: language.locale_tag }));
  const cmsDefault = cms.find((language) => language.is_default && files.has(language.code))?.code;
  if (cmsDefault) lastDefault = cmsDefault;
  if (list.length === 0) {
    list = [...files.entries()].map(([code, messages]) => ({ code, name: messages.language.name, tag: messages.language.tag }));
  }
  const fallback = [cmsDefault, lastDefault, process.env.PORTAL_DEFAULT_LANGUAGE].find((code) => code && list.some((l) => l.code === code)) ?? list[0]?.code ?? "";
  const value = { list, fallback };
  // A failed lookup is retried after ten seconds instead of a minute.
  cached = { value, at: cms.length ? Date.now() : Date.now() - 50_000 };
  return value;
}

/** The language to send someone to: a remembered choice when it is still offered, else the default. */
export function preferredLanguage(remembered: string | undefined, { list, fallback }: Languages): string {
  return remembered && list.some((language) => language.code === remembered) ? remembered : fallback;
}
