import { describe, expect, it } from "vitest";
import type { BulkRow, GapKeyword } from "../../api/types";
import { ALL_GAPS, filterGap, trafficShares } from "./Gap";

const gap = (keyword: string, volume: number, difficulty: number, competitors: number, weak = false): GapKeyword => ({
  keyword,
  volume,
  difficulty,
  cpc: null,
  intent: null,
  own: null,
  competitors: Array.from({ length: competitors }, (_, index) => ({ domain: `c${index}.com`, position: index + 1, url: null, traffic: 0 })),
  weak,
});

describe("gaps", () => {
  const rows = [gap("koşu ayakkabısı", 14800, 51, 2, true), gap("trail ayakkabı", 1900, 30, 1), gap("spor çorap", 4400, 22, 2)];

  it("filters by competitors, volume, difficulty, weakness and text", () => {
    expect(filterGap(rows, ALL_GAPS)).toHaveLength(3);
    expect(filterGap(rows, { ...ALL_GAPS, minCompetitors: 2 }).map((row) => row.keyword)).toEqual(["koşu ayakkabısı", "spor çorap"]);
    expect(filterGap(rows, { ...ALL_GAPS, minVolume: 2000 })).toHaveLength(2);
    expect(filterGap(rows, { ...ALL_GAPS, maxDifficulty: 30 })).toHaveLength(2);
    expect(filterGap(rows, { ...ALL_GAPS, weakOnly: true })).toHaveLength(1);
    expect(filterGap(rows, { ...ALL_GAPS, query: "KOSU" })).toHaveLength(1);
  });

  it("shares traffic among the compared sites", () => {
    const sites: BulkRow[] = [
      { target: "a.com", organic_traffic: 300, organic_keywords: 1, paid_traffic: 0, rank: null },
      { target: "b.com", organic_traffic: 100, organic_keywords: 1, paid_traffic: 0, rank: null },
    ];
    const shares = trafficShares(sites);
    expect(shares.get("a.com")).toBe(75);
    expect(shares.get("b.com")).toBe(25);
    expect(trafficShares([{ ...sites[0]!, organic_traffic: 0 }]).get("a.com")).toBe(0);
  });
});
