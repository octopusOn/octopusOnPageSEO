import { describe, expect, it } from "vitest";
import { sortRows } from "@octopusseo/ui";
import type { RankedKeyword } from "../api/types";
import { featureCounts, filterKeywords, NO_FILTER, toCsv } from "./report";
import { shortUrl } from "./tables";

const row = (keyword: string, position: number | null, volume: number | null, intent: string | null, features: string[] = []): RankedKeyword => ({
  keyword,
  position,
  absolute_position: position,
  previous_position: null,
  change: "same",
  url: `https://example.com/${keyword.replace(/ /g, "-")}`,
  volume,
  difficulty: null,
  cpc: null,
  competition: null,
  intent,
  traffic: 1,
  traffic_value_usd: 0,
  serp_features: features,
});

const rows = [
  row("spor ayakkabı", 3, 40500, "commercial", ["ai_overview", "images"]),
  row("ayakkabı bakımı", 12, 4400, "informational", ["ai_overview"]),
  row("deri bot", 25, 900, "transactional"),
  row("bilinmeyen", null, null, null),
];

describe("keyword filters", () => {
  it("keeps everything without a filter", () => {
    expect(filterKeywords(rows, NO_FILTER)).toHaveLength(4);
  });

  it("filters by text, position band, intent and volume", () => {
    expect(filterKeywords(rows, { ...NO_FILTER, query: "AYAKKABI" }).map((r) => r.keyword)).toEqual(["spor ayakkabı", "ayakkabı bakımı"]);
    expect(filterKeywords(rows, { ...NO_FILTER, query: "ayakkabi bakimi" }).map((r) => r.keyword)).toEqual(["ayakkabı bakımı"]);
    expect(filterKeywords(rows, { ...NO_FILTER, query: "deri-bot" }).map((r) => r.keyword)).toEqual(["deri bot"]);
    expect(filterKeywords(rows, { ...NO_FILTER, position: "top10" }).map((r) => r.keyword)).toEqual(["spor ayakkabı"]);
    expect(filterKeywords(rows, { ...NO_FILTER, position: "beyond" }).map((r) => r.keyword)).toEqual(["deri bot"]);
    expect(filterKeywords(rows, { ...NO_FILTER, intent: "informational" })).toHaveLength(1);
    expect(filterKeywords(rows, { ...NO_FILTER, minVolume: 1000 })).toHaveLength(2);
  });

  it("counts result page elements once per keyword", () => {
    expect(featureCounts([...rows, row("x", 1, 1, null, ["images", "images"])])).toEqual([
      ["ai_overview", 2],
      ["images", 2],
    ]);
  });
});

describe("tables", () => {
  it("sorts numbers and text with empty values last", () => {
    const sorted = sortRows(rows, (r) => r.volume, "desc").map((r) => r.keyword);
    expect(sorted).toEqual(["spor ayakkabı", "ayakkabı bakımı", "deri bot", "bilinmeyen"]);
    const ascending = sortRows(rows, (r) => r.position, "asc").map((r) => r.keyword);
    expect(ascending).toEqual(["spor ayakkabı", "ayakkabı bakımı", "deri bot", "bilinmeyen"]);
  });

  it("writes CSV that spreadsheets read safely", () => {
    const csv = toCsv(["a", "b"], [["x,y", '"q"'], ["=SUM(A1)", null], [1.5, "satır\nyeni"]]);
    expect(csv.startsWith("﻿a,b\r\n")).toBe(true);
    expect(csv).toContain('"x,y","""q"""');
    expect(csv).toContain("'=SUM(A1),");
    expect(csv).toContain('1.5,"satır\nyeni"');
  });

  it("shortens addresses to their path", () => {
    expect(shortUrl("https://example.com/a/b?c=1")).toBe("/a/b?c=1");
    expect(shortUrl("https://example.com")).toBe("/");
    expect(shortUrl("not a url")).toBe("not a url");
    expect(shortUrl(null)).toBe("");
  });
});
