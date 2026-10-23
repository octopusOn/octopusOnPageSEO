// What the link work needs beyond the rows: where a broken page that still gets links should
// redirect, and the words of a page address.

import { fold } from "../../lib/report";

/** The words of an address's path, folded: `/Spor-Ayakkabı/kadın` gives spor, ayakkabi, kadin. */
export function pathWords(url: string): string[] {
  let path = url;
  try {
    path = new URL(url).pathname;
  } catch {
    // A bare path stays as it is.
  }
  return fold(decodeURIComponent(path))
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => word.length > 1 && !/^\d+$/.test(word) && !["html", "htm", "php", "aspx"].includes(word));
}

/**
 * Where a broken page should redirect: the live page sharing the most words of its address
 * (the shorter one on a tie), else the home page.
 */
export function redirectFor(broken: string, live: readonly string[]): string {
  const words = new Set(pathWords(broken));
  let best: { url: string; shared: number } | null = null;
  for (const url of live) {
    if (url === broken) continue;
    const shared = pathWords(url).filter((word) => words.has(word)).length;
    if (shared > 0 && (!best || shared > best.shared || (shared === best.shared && url.length < best.url.length))) best = { url, shared };
  }
  if (best) return best.url;
  try {
    return `${new URL(broken).origin}/`;
  } catch {
    return "/";
  }
}
