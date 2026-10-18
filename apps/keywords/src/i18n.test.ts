import { describe, expect, it } from "vitest";
import { tables } from "./i18n";

// Keys built at run time from values the desktop side sends.
const FAMILIES: Record<string, string[]> = {
  job: ["discover", "enrich", "serp", "cluster", "search-console", "google-login"],
  status: ["running", "done", "partial", "failed", "cancelled"],
  run: ["import", "discover", "enrich", "serp", "cluster", "map"],
  method: ["lexical", "serp", "hybrid"],
  methodHint: ["lexical", "serp", "hybrid"],
  source: [
    "manual",
    "import",
    "gsc",
    "wordstat",
    "ads-ideas",
    "labs-ideas",
    "related",
    "suggestions",
    "competitors",
    "autocomplete",
    "serpapi-autocomplete",
    "google-ads",
    "google-suggest",
    "yandex-suggest",
    "dataforseo-ads-ideas",
    "dataforseo-ideas",
    "dataforseo-related",
    "dataforseo-suggestions",
    "dataforseo-competitors",
    "dataforseo-autocomplete",
    "google-ads-ideas",
    "dataforseo-ads",
    "dataforseo-kd",
    "dataforseo-intent",
  ],
  sourceHint: [
    "wordstat",
    "ads-ideas",
    "labs-ideas",
    "related",
    "suggestions",
    "competitors",
    "autocomplete",
    "serpapi-autocomplete",
    "google-ads",
    "google-suggest",
    "yandex-suggest",
  ],
  expansion: ["alphabet", "turkish-letters", "questions", "commercial"],
  enrich: ["volume", "difficulty", "intent"],
  intent: ["informational", "navigational", "commercial", "transactional", "local"],
  role: ["keyword", "volume", "cpc", "competition", "competition-index", "difficulty", "clicks", "impressions", "position", "month"],
  feature: [
    "ads-top",
    "ads-bottom",
    "featured-snippet",
    "people-also-ask",
    "local-pack",
    "videos",
    "images",
    "ai-overview",
    "related-searches",
    "top-stories",
    "knowledge-panel",
    "shopping",
    "other",
  ],
  view: ["gaps", "competing", "mapped"],
  viewEmpty: ["gaps", "competing", "mapped"],
  offer: ["discover", "volume", "trend", "difficulty", "intent", "competitors", "autocomplete", "site pages", "result pages"],
  secret: ["api_key", "folder_id", "credentials", "client_secret", "refresh_token", "developer_token"],
  secretState: ["saved", "environment", "unset", "rebind"],
  check: ["consent", "captcha", "blocked"],
  secretPlaceholder: ["api_key", "folder_id", "credentials"],
};

describe("texts", () => {
  it("has every key in both languages", () => {
    expect(Object.keys(tables.tr).sort()).toEqual(Object.keys(tables.en).sort());
    for (const [key, text] of Object.entries(tables.tr)) expect(text.trim(), key).not.toBe("");
  });

  it("has a text for every value the desktop side sends", () => {
    for (const [family, values] of Object.entries(FAMILIES))
      for (const value of values) expect(Object.keys(tables.en), `${family}.${value}`).toContain(`${family}.${value}`);
  });

  it("keeps the placeholders of each text", () => {
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
    for (const [key, text] of Object.entries(tables.en))
      expect(placeholders(tables.tr[key as keyof typeof tables.tr]), key).toEqual(placeholders(text));
  });
});
