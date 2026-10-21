import { describe, expect, it } from "vitest";
import type { KeywordRank, TrackerRow } from "../../api/types";
import { ALL_RANKS, csvRows, filterRanks, heights, parseKeywords, parseTags, placeLabel, tagsOf } from "./rank";

function row(keyword: string, position: number | null, extra: Partial<KeywordRank> = {}): KeywordRank {
  return {
    keyword_id: keyword.length,
    keyword,
    tags: [],
    volume: null,
    position,
    previous: null,
    change: null,
    best: position,
    url: null,
    cannibalised: false,
    checked_at: "2026-10-21T09:00:00Z",
    history: [],
    error: null,
    pages: [],
    features: [],
    ai_overview: false,
    ai_cited: false,
    snippet: false,
    ...extra,
  };
}

const ROWS = [
  row("Spor Ayakkabı", 2, { tags: ["ayakkabı"], ai_overview: true }),
  row("koşu ayakkabısı", 14, { tags: ["koşu", "ayakkabı"] }),
  row("bot", null),
  row("çizme", 8),
];

describe("rank tracker", () => {
  it("names a place by engine, city or country and device", () => {
    const tracker: TrackerRow = { id: 1, project_id: 1, engine: "google", country: "tr", language: "tr", device: "mobile", location: null, created_at: "" };
    const device = (name: string) => (name === "mobile" ? "Mobil" : "Masaüstü");
    expect(placeLabel(tracker, device)).toBe("Google · TR · Mobil");
    expect(placeLabel({ ...tracker, engine: "yandex", location: "Istanbul,Istanbul,Turkey" }, device)).toBe("Yandex · Istanbul · Mobil");
  });

  it("filters by folded text, tag, position band and AI Overview", () => {
    const names = (rows: KeywordRank[]) => rows.map((entry) => entry.keyword);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, query: "AYAKKABI" }))).toEqual(["Spor Ayakkabı", "koşu ayakkabısı"]);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, tag: "koşu" }))).toEqual(["koşu ayakkabısı"]);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, band: "top3" }))).toEqual(["Spor Ayakkabı"]);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, band: "top10" }))).toEqual(["Spor Ayakkabı", "çizme"]);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, band: "page2" }))).toEqual(["koşu ayakkabısı"]);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, band: "none" }))).toEqual(["bot"]);
    expect(names(filterRanks(ROWS, { ...ALL_RANKS, aiOnly: true }))).toEqual(["Spor Ayakkabı"]);
    expect(tagsOf(ROWS)).toEqual(["ayakkabı", "koşu"]);
  });

  it("reads pasted keywords and tags without repeats", () => {
    expect(parseKeywords(" spor  ayakkabı \n\nSpor Ayakkabı\nbot\t880\n")).toEqual([{ keyword: "spor ayakkabı" }, { keyword: "bot" }]);
    expect(parseTags("marka, kış ,marka,")).toEqual(["marka", "kış"]);
  });

  it("draws positions best at the top and writes the CSV rows", () => {
    expect(
      heights([
        ["2026-10-20", 1],
        ["2026-10-21", null],
        ["2026-10-22", 100],
      ]),
    ).toEqual([100, 0, 1]);
    expect(
      heights([
        ["2026-10-20", 5],
        ["2026-10-21", 3],
      ]),
    ).toEqual([1, 3]);
    expect(csvRows([row("bot", 3, { tags: ["a", "b"], ai_overview: true })])[0]).toEqual(["bot", "a b", 3, null, 3, null, null, "yes", "no", "2026-10-21T09:00:00Z"]);
  });
});
