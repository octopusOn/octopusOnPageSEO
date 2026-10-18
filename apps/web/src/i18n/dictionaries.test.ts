import { describe, expect, it } from "vitest";

import { locales } from "./config";
import { getDictionary } from "./index";

type Tree = string | Tree[] | { [key: string]: Tree };

/** Every leaf string of a dictionary with its path, e.g. `faq.items.0.q`. */
function leaves(tree: Tree, path = ""): [string, string][] {
  if (typeof tree === "string") return [[path, tree]];
  const entries = Array.isArray(tree) ? tree.map((v, i) => [String(i), v] as const) : Object.entries(tree);
  return entries.flatMap(([key, value]) => leaves(value, path ? `${path}.${key}` : key));
}

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("dictionaries", () => {
  const english = new Map(leaves(getDictionary("en") as Tree));

  for (const locale of locales.filter((l) => l !== "en")) {
    it(`${locale} translates every English text with the same placeholders`, () => {
      const translated = new Map(leaves(getDictionary(locale) as Tree));
      expect(english.size).toBeGreaterThan(60);
      expect([...translated.keys()]).toEqual([...english.keys()]);
      for (const [path, text] of translated) {
        expect(text.trim(), `${locale}: ${path}`).not.toBe("");
        expect(placeholders(text), `${locale}: ${path}`).toEqual(placeholders(english.get(path) ?? ""));
      }
    });
  }

  it("falls back to English for languages without a dictionary", () => {
    expect(getDictionary("pt-br")).toBe(getDictionary("en"));
  });
});
