import { describe, expect, it } from "vitest";

import { catalog, messagesFor } from "./catalog";
import { localizedRewrites, matchRoute, ROUTES, routeUrl } from "./routes";

type Tree = { [key: string]: string | Tree };

/** Every text's dotted key with its `{placeholders}`. */
function leaves(tree: Tree, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(tree)) {
    if (typeof value === "string") out.set(prefix + key, [...value.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(","));
    else for (const [k, v] of leaves(value, `${prefix}${key}.`)) out.set(k, v);
  }
  return out;
}

const all = catalog();
const reference = leaves(all.get("tr") as unknown as Tree);

describe("messages files", () => {
  it("exist for at least Turkish and English", () => {
    expect([...all.keys()]).toEqual(expect.arrayContaining(["tr", "en"]));
  });

  it.each([...all.keys()])("%s has every text of tr.json with the same placeholders", (code) => {
    const own = leaves(all.get(code) as unknown as Tree);
    expect([...own.keys()].sort()).toEqual([...reference.keys()].sort());
    for (const [key, placeholders] of reference) expect(`${key}: ${own.get(key)}`).toBe(`${key}: ${placeholders}`);
  });

  it.each([...all.keys()])("%s names every route segment once", (code) => {
    const names = all.get(code)!.routes as Record<string, string>;
    const segments = new Set(ROUTES.flatMap((route) => route.split("/")).filter((s) => s && !s.startsWith("[")));
    for (const segment of segments) expect(names[segment], `${code}: ${segment}`).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
    const used = Object.values(names);
    expect(new Set(used).size, `${code}: two segments share a name`).toBe(used.length);
  });

  it("fall back to another language for a missing text", () => {
    expect(messagesFor("xx", "en").nav.dashboard).toBe(all.get("en")!.nav.dashboard);
  });
});

describe("addresses", () => {
  const tr = all.get("tr")!.routes as Record<string, string>;
  const en = all.get("en")!.routes as Record<string, string>;

  it("are written in the page's language", () => {
    expect(routeUrl("tr", tr, "credits/confirm", {}, { amount: 50000 })).toBe("/tr/kredi/onay?amount=50000");
    expect(routeUrl("en", en, "credits/payment/[id]", { id: "a b" })).toBe("/en/credits/payment/a%20b");
    expect(routeUrl("tr", tr, "", {}, { next: undefined })).toBe("/tr");
  });

  it("are rewritten to the page folders, specific routes first", () => {
    const rewrites = localizedRewrites([["tr", tr], ["en", en]]);
    expect(rewrites).toContainEqual({ source: "/tr/kredi/odeme/:id", destination: "/tr/credits/payment/:id" });
    expect(rewrites).toContainEqual({ source: "/en/devices", destination: "/en/tokens" });
    expect(rewrites.some((rewrite) => rewrite.source === "/en/credits")).toBe(false);
    const sources = rewrites.map((rewrite) => rewrite.source);
    expect(sources.indexOf("/tr/admin/baglayicilar/yeni")).toBeLessThan(sources.indexOf("/tr/admin/baglayicilar/:id"));
  });

  it("match in their language, another language or by English route names", () => {
    expect(matchRoute(["kredi", "onay"], tr)).toEqual({ route: "credits/confirm", params: {} });
    expect(matchRoute(["credits", "confirm"], tr)).toEqual({ route: "credits/confirm", params: {} });
    expect(matchRoute(["kredi", "onay"], en, [tr])).toEqual({ route: "credits/confirm", params: {} });
    expect(matchRoute(["admin", "baglayicilar", "yeni"], tr)).toEqual({ route: "admin/connectors/new", params: {} });
    expect(matchRoute(["admin", "baglayicilar", "dataforseo"], tr)).toEqual({ route: "admin/connectors/[id]", params: { id: "dataforseo" } });
    expect(matchRoute(["yasal", "kvkk"], tr)).toEqual({ route: "legal/[slug]", params: { slug: "kvkk" } });
    expect(matchRoute([], tr)).toEqual({ route: "", params: {} });
    expect(matchRoute(["nothing"], tr, [en])).toBeNull();
    expect(matchRoute(["fatura", "%E0", "pdf"], tr)).toBeNull();
  });
});
