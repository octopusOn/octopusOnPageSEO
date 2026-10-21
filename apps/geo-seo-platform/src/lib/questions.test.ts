import { describe, expect, it } from "vitest";
import type { KeywordData } from "../api/types";
import { ALL_KEYWORDS, filterKeywordData } from "../modules/keywords/KeywordTable";
import { isQuestion } from "./questions";

const keyword = (name: string, volume: number | null, difficulty: number | null, intent: string | null): KeywordData => ({
  keyword: name,
  volume,
  cpc: null,
  competition: null,
  competition_level: null,
  difficulty,
  intent,
  other_intents: [],
  monthly: [],
  trend: { monthly: null, quarterly: null, yearly: null },
  serp_features: [],
  results: null,
  top_backlinks: null,
  top_referring_domains: null,
});

describe("questions", () => {
  it("finds question words of the keyword's language", () => {
    expect(isQuestion("spor ayakkabı nasıl yıkanır", "tr")).toBe(true);
    expect(isQuestion("NASIL YIKANIR spor ayakkabı", "tr")).toBe(true);
    expect(isQuestion("spor ayakkabı mı sneaker mı", "tr")).toBe(true);
    expect(isQuestion("ayakkabı nedir", "tr")).toBe(true);
    expect(isQuestion("beyaz spor ayakkabı", "tr")).toBe(false);
    // English words do not count in Turkish: "iş" folds to "is".
    expect(isQuestion("iş ayakkabısı", "tr")).toBe(false);
    expect(isQuestion("how to clean sneakers", "en")).toBe(true);
    expect(isQuestion("cómo limpiar zapatillas", "es")).toBe(true);
    expect(isQuestion("wie reinige ich sneaker", "de")).toBe(true);
    expect(isQuestion("sneaker sale?", "en")).toBe(true);
  });

  it("filters keywords by text, intent, volume and difficulty", () => {
    const rows = [keyword("spor ayakkabı", 40500, 62, "commercial"), keyword("ayakkabı bakımı", 880, 12, "informational"), keyword("bot", null, null, null)];
    expect(filterKeywordData(rows, ALL_KEYWORDS)).toHaveLength(3);
    expect(filterKeywordData(rows, { ...ALL_KEYWORDS, query: "AYAKKABI" })).toHaveLength(2);
    expect(filterKeywordData(rows, { ...ALL_KEYWORDS, maxDifficulty: 30 }).map((row) => row.keyword)).toEqual(["ayakkabı bakımı", "bot"]);
    expect(filterKeywordData(rows, { ...ALL_KEYWORDS, minVolume: 1000 })).toHaveLength(1);
    expect(filterKeywordData(rows, { ...ALL_KEYWORDS, intent: "informational" })).toHaveLength(1);
  });
});
