import { expect, it } from "vitest";
import { percent, tables } from "./i18n";

const TURKISH = /[çğıöşüÇĞİÖŞÜ]/;

// Interface text lives in i18n.tsx; the pages and components only hold keys.
const sources = import.meta.glob<string>(
  ["./App.tsx", "./pages/*.tsx", "./components/*.tsx"],
  { query: "?raw", import: "default", eager: true },
);

it("keeps Turkish out of the English table", () => {
  for (const [key, text] of Object.entries(tables.en)) {
    expect(TURKISH.test(text), `${key}: ${text}`).toBe(false);
  }
});

it("keeps Turkish text out of the pages and components", () => {
  expect(Object.keys(sources).length).toBeGreaterThan(5);
  for (const [path, text] of Object.entries(sources)) {
    const lines = text
      .split("\n")
      .map((line, index) => ({ line, number: index + 1 }))
      // The language picker names Turkish in Turkish.
      .filter(({ line }) => TURKISH.test(line) && !line.includes("Türkçe (TR)"));
    expect(lines, path).toEqual([]);
  }
});

it("writes percentages the way each language does", () => {
  expect(percent(45, "tr")).toBe("%45");
  expect(percent(45, "en")).toBe("45%");
});
