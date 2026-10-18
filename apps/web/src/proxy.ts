// Sends visitors without a language in the URL to their language: an explicit earlier choice
// (cookie), else the best match of the browser's Accept-Language, else the default language.
// The languages come from the CMS (cached for a minute; the built-in list if it is down).

import { NextResponse, type NextRequest } from "next/server";

import { LOCALE_COOKIE, fallbackLanguages } from "./i18n/config";

interface Languages {
  codes: string[];
  fallback: string;
}

let cached: { value: Languages; at: number } | null = null;

async function languages(): Promise<Languages> {
  if (cached && Date.now() - cached.at < 60_000) return cached.value;
  let list = fallbackLanguages;
  try {
    const base = (process.env.CMS_API_URL?.trim() || "http://localhost:8000").replace(/\/$/, "");
    const response = await fetch(`${base}/api/v1/languages`, { signal: AbortSignal.timeout(2000), headers: { Accept: "application/json" } });
    const json = (await response.json()) as { data?: typeof fallbackLanguages };
    if (response.ok && json.data?.length) list = json.data;
  } catch {
    // Keep the built-in list.
  }
  const value = { codes: list.map((l) => l.code), fallback: (list.find((l) => l.is_default) ?? list[0])?.code ?? "en" };
  cached = { value, at: Date.now() };
  return value;
}

function preferredLanguage(request: NextRequest, { codes, fallback }: Languages): string {
  const remembered = request.cookies.get(LOCALE_COOKIE)?.value;
  if (remembered && codes.includes(remembered)) return remembered;
  const header = request.headers.get("accept-language") ?? "";
  const ranked = header
    .split(",")
    .map((part) => {
      const [tag = "", ...params] = part.trim().split(";");
      const q = params.find((p) => p.trim().startsWith("q="));
      return { tag: tag.toLowerCase(), q: q ? Number(q.trim().slice(2)) || 0 : 1 };
    })
    .filter((entry) => entry.tag && entry.q > 0)
    .sort((a, b) => b.q - a.q);
  for (const { tag } of ranked) {
    if (codes.includes(tag)) return tag;
    const base = tag.split("-")[0] ?? "";
    if (codes.includes(base)) return base;
  }
  return fallback;
}

export async function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const known = await languages();
  const first = pathname.split("/")[1] ?? "";
  if (known.codes.includes(first)) return;
  const url = request.nextUrl.clone();
  url.pathname = `/${preferredLanguage(request, known)}${pathname === "/" ? "" : pathname}`;
  const response = NextResponse.redirect(url);
  response.headers.set("Vary", "Accept-Language, Cookie");
  return response;
}

export const config = {
  // Pages only: skip Next.js internals, API routes and files (anything with a dot: sitemap.xml…).
  matcher: ["/((?!_next|api/|.*\\..*).*)"],
};
