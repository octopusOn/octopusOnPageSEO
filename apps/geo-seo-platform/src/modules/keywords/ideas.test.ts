import { describe, expect, it } from "vitest";
import type { KeywordData, PastSerp } from "../../api/types";
import { questionPattern } from "../../lib/questions";
import { serpTracks, wordGroups } from "./ideas";

const idea = (keyword: string, volume: number): KeywordData => ({
  keyword,
  volume,
  cpc: null,
  competition: null,
  competition_level: null,
  difficulty: null,
  intent: null,
  other_intents: [],
  monthly: [],
  trend: { monthly: null, quarterly: null, yearly: null },
  serp_features: [],
  results: null,
  top_backlinks: null,
  top_referring_domains: null,
});

describe("keyword research helpers", () => {
  it("groups ideas by the words they share besides the seed", () => {
    const rows = [idea("spor ayakkabı kadın", 900), idea("Kadın spor ayakkabı beyaz", 300), idea("spor ayakkabı erkek", 800), idea("erkek spor ayakkabı siyah", 100), idea("spor ayakkabı ve bot", 50)];
    const groups = wordGroups(rows, "Spor Ayakkabı", "tr");
    expect(groups).toEqual([
      { word: "kadın", keywords: 2, volume: 1200 },
      { word: "erkek", keywords: 2, volume: 900 },
    ]);
  });

  it("asks DataForSEO for questions as whole words", () => {
    const pattern = new RegExp(questionPattern("tr")!);
    expect(pattern.test("spor ayakkabı nasıl temizlenir")).toBe(true);
    expect(pattern.test("ayakkabı numarası kaç olmalı")).toBe(true);
    expect(pattern.test("nasılsın")).toBe(false);
    expect(pattern.test("ayakkabı neden kokar?")).toBe(true);
    expect(pattern.test("kimya ayakkabı")).toBe(false);
    expect(questionPattern("xx")).toBeNull();
  });

  it("lays past result pages out as tracks, a column a month", () => {
    const page = (date: string, results: [number, string][]): PastSerp => ({ date, results_count: null, features: [], results: results.map(([rank, url]) => ({ rank, url, domain: null, title: null })) });
    const grid = serpTracks([
      page("2026-10-04", [[1, "a"], [2, "b"]]),
      page("2026-09-03", [[1, "b"], [2, "c"]]),
      page("2026-09-20", [[1, "b"], [2, "a"]]),
    ]);
    expect(grid.dates).toEqual(["2026-09-20", "2026-10-04"]);
    expect(grid.tracks.map((track) => [track.url, track.ranks])).toEqual([
      ["a", [2, 1]],
      ["b", [1, 2]],
    ]);
  });
});
