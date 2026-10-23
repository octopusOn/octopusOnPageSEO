import { describe, expect, it } from "vitest";
import type { BulkRow, MatrixKeyword } from "../../api/types";
import { filterMatrix, NO_MATRIX_FILTER, shareSeries, trafficShares } from "./groups";

const row = (keyword: string, volume: number, difficulty: number, cpc: number, intent: string, groups: MatrixKeyword["groups"]): MatrixKeyword => ({
  keyword,
  volume,
  difficulty,
  cpc,
  intent,
  rankings: [null, { domain: "c.com", position: 3, url: null, traffic: 0 }],
  groups,
});

describe("gaps", () => {
  const rows = [row("koşu ayakkabısı", 14800, 51, 1.2, "commercial", ["shared", "weak"]), row("trail ayakkabı", 1900, 30, 0.4, "commercial", ["missing"]), row("spor çorap nasıl seçilir", 4400, 22, 0.1, "informational", ["untapped"])];

  it("keeps a group's keywords that pass the filters", () => {
    expect(filterMatrix(rows, "all", NO_MATRIX_FILTER)).toHaveLength(3);
    expect(filterMatrix(rows, "weak", NO_MATRIX_FILTER).map((entry) => entry.keyword)).toEqual(["koşu ayakkabısı"]);
    expect(filterMatrix(rows, "all", { ...NO_MATRIX_FILTER, minVolume: 2000 })).toHaveLength(2);
    expect(filterMatrix(rows, "all", { ...NO_MATRIX_FILTER, maxDifficulty: 30 })).toHaveLength(2);
    expect(filterMatrix(rows, "all", { ...NO_MATRIX_FILTER, minCpc: 0.4 })).toHaveLength(2);
    expect(filterMatrix(rows, "all", { ...NO_MATRIX_FILTER, intent: "informational" })).toHaveLength(1);
    expect(filterMatrix(rows, "all", { ...NO_MATRIX_FILTER, query: "KOSU" })).toHaveLength(1);
  });

  it("shares each month's traffic among the sites", () => {
    const series = shareSeries([
      { target: "a.com", months: [{ year: 2026, month: 9, traffic: 300, keywords: 1 }, { year: 2026, month: 8, traffic: 100, keywords: 1 }] },
      { target: "b.com", months: [{ year: 2026, month: 9, traffic: 100, keywords: 1 }] },
    ]);
    expect(series.months).toEqual(["2026-08", "2026-09"]);
    expect(series.series[0]!.shares).toEqual([100, 75]);
    expect(series.series[1]!.shares).toEqual([0, 25]);
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
