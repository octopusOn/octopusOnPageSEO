import { describe, expect, it } from "vitest";
import type { GscDay, GscQuery } from "../../api/types";
import { ALL_QUERIES, auditState, dailySeries, exportRows, filterQueries, pageCsv, percentChange, queryCsv } from "./gsc";

const day = (date: string, period: "now" | "before", clicks: number): GscDay => ({ date, period, clicks, impressions: clicks * 10, position: 5, sessions: null, key_events: null });

const query = (text: string, extra: Partial<GscQuery> = {}): GscQuery => ({
  query: text,
  clicks: 10,
  impressions: 200,
  ctr: 5,
  position: 6.2,
  clicks_change: null,
  position_change: null,
  pages: 1,
  tracked: false,
  near_top: false,
  low_ctr: false,
  brand: false,
  ...extra,
});

describe("search console", () => {
  it("computes changes and lays the days before beside the current ones", () => {
    expect(percentChange(30, 10)).toBe(200);
    expect(percentChange(5, 0)).toBeNull();
    const series = dailySeries([day("2026-09-01", "before", 1), day("2026-09-02", "before", 2), day("2026-09-03", "now", 5), day("2026-09-04", "now", 7)], "2026-09-03", "2026-09-01");
    expect(series.dates).toEqual(["2026-09-03", "2026-09-04"]);
    expect(series.clicks).toEqual([5, 7]);
    expect(series.clicksBefore).toEqual([1, 2]);
    // A day Google has no data for stays empty instead of shifting the others.
    expect(dailySeries([day("2026-09-02", "before", 2), day("2026-09-03", "now", 5), day("2026-09-04", "now", 7)], "2026-09-03", "2026-09-01").clicksBefore).toEqual([null, 2]);
  });

  it("filters queries by text, reach and tracking", () => {
    const rows = [query("Spor Ayakkabı", { near_top: true }), query("sneaker", { tracked: true }), query("bot")];
    expect(filterQueries(rows, { ...ALL_QUERIES, query: "ayakkabi" }).map((row) => row.query)).toEqual(["Spor Ayakkabı"]);
    expect(filterQueries(rows, { ...ALL_QUERIES, nearTop: true })).toHaveLength(1);
    expect(filterQueries(rows, { ...ALL_QUERIES, untracked: true }).map((row) => row.query)).toEqual(["Spor Ayakkabı", "bot"]);
    const named = [...rows, query("örnek mağaza", { brand: true })];
    expect(filterQueries(named, { ...ALL_QUERIES, brand: "brand" }).map((row) => row.query)).toEqual(["örnek mağaza"]);
    expect(filterQueries(named, { ...ALL_QUERIES, brand: "other" })).toHaveLength(rows.length);
    expect(exportRows([["a", 1, true, null, false]])).toEqual([["a", "1", "1", "", "0"]]);
  });

  it("reads how the audit saw a page and writes CSV rows", () => {
    expect(auditState(null)).toBeNull();
    expect(auditState({ status: 200, indexability: "indexable", findings: 0 })).toBe("ok");
    expect(auditState({ status: 200, indexability: "indexable", findings: 3 })).toBe("findings");
    expect(auditState({ status: 200, indexability: "noindex", findings: 1 })).toBe("noindex");
    expect(auditState({ status: 301, indexability: "non-indexable", findings: 0 })).toBe("redirect");
    expect(auditState({ status: null, indexability: "", findings: 0 })).toBe("error");
    expect(queryCsv([query("a", { clicks_change: -2, tracked: true })])).toEqual([["a", "10", "-2", "200", "5", "6.2", "", "1", "1"]]);
    expect(pageCsv([{ page: "https://x.com/", clicks: 1, impressions: 2, ctr: 50, position: 3, clicks_change: null, sessions: 4, key_events: null, revenue: null, audit: null }])).toEqual([
      ["https://x.com/", "1", "", "2", "50", "3", "4", "", ""],
    ]);
  });
});
