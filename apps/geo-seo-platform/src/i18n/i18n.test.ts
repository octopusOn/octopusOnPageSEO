import { describe, expect, it } from "vitest";
import { LANGUAGES, MESSAGES, translate } from "./index";

/** Every source file of the interface, as text. */
const SOURCES = import.meta.glob<string>(["../**/*.ts", "../**/*.tsx", "!../**/*.test.ts"], {
  eager: true,
  query: "?raw",
  import: "default",
});

describe("translations", () => {
  it("has every key in every language", () => {
    const reference = Object.keys(MESSAGES.tr ?? {}).sort();
    expect(reference.length).toBeGreaterThan(100);
    for (const { code } of LANGUAGES) {
      const keys = Object.keys(MESSAGES[code] ?? {}).sort();
      expect(keys, code).toEqual(reference);
      for (const key of keys) expect(MESSAGES[code]?.[key]?.trim(), `${code} ${key}`).toBeTruthy();
    }
  });

  it("keeps the placeholders of every message", () => {
    const placeholders = (text: string) => (text.match(/\{\w+\}/g) ?? []).sort();
    for (const [key, text] of Object.entries(MESSAGES.tr ?? {})) {
      for (const { code } of LANGUAGES) {
        expect(placeholders(MESSAGES[code]?.[key] ?? ""), `${code} ${key}`).toEqual(placeholders(text));
      }
    }
  });

  it("knows every key the interface asks for", () => {
    const asked = new Set<string>();
    for (const text of Object.values(SOURCES)) {
      for (const match of text.matchAll(/\bt\(\s*"([a-z]+\.[A-Za-z0-9]+)"/g)) asked.add(match[1]!);
      for (const match of text.matchAll(/"((?:common|projects|overview|sources|settings|keywords|backlinks|gap|rank|audit|ai|gsc|reports|snapshots|entity|glossary|query|local|content|project|opportunities|alerts)\.[A-Za-z0-9]+)"/g)) asked.add(match[1]!);
    }
    expect(asked.size).toBeGreaterThan(50);
    const missing = [...asked].filter((key) => !(key in (MESSAGES.tr ?? {})));
    expect(missing).toEqual([]);
  });

  it("fills placeholders and falls back to English", () => {
    expect(translate("tr", "common.credits", { n: 12 })).toBe("12 kredi");
    expect(translate("de", "projects.deleteConfirm", { name: "X" })).toContain("„X“");
    expect(translate("en", "no.such")).toBe("no.such");
  });
});
