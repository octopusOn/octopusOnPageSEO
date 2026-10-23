import { describe, expect, it } from "vitest";
import type { Opportunity } from "../../api/types";
import { ALL_OPPORTUNITIES, filterOpportunities, seriesChange, spendPage, textValues, titleKey } from "./opportunity";

const item = (extra: Partial<Opportunity>): Opportunity => ({
  key: "near_top|spor ayakkabı",
  rule: "near_top",
  page: "rank",
  params: { keyword: "spor ayakkabı" },
  subject: "spor ayakkabı",
  detail: { position: 6.25, volume: 9900 },
  gain: 640,
  impact: 76,
  effort: "medium",
  priority: 57,
  state: "open",
  until: null,
  returned: false,
  ...extra,
});

describe("opportunities", () => {
  it("filters by area and effort", () => {
    const items = [item({}), item({ key: "low_ctr|a", rule: "low_ctr", page: "gsc", effort: "low" })];
    expect(filterOpportunities(items, ALL_OPPORTUNITIES)).toHaveLength(2);
    expect(filterOpportunities(items, { area: "gsc", effort: "all" }).map((row) => row.rule)).toEqual(["low_ctr"]);
    expect(filterOpportunities(items, { area: "all", effort: "medium" }).map((row) => row.rule)).toEqual(["near_top"]);
  });

  it("writes values and picks the text", () => {
    expect(textValues(item({}), "tr")).toEqual({ subject: "spor ayakkabı", position: "6,3", volume: "9.900" });
    expect(textValues(item({ detail: { arp: null, lost: true } }), "en")).toEqual({ subject: "spor ayakkabı", arp: "—", lost: "true" });
    expect(titleKey(item({ rule: "dropped", detail: { lost: true } }))).toBe("opportunities.title_lost");
    expect(titleKey(item({ rule: "dropped", detail: { lost: false } }))).toBe("opportunities.title_dropped");
  });

  it("names spending by its page", () => {
    expect([spendPage("research"), spendPage("rank"), spendPage("ai_mentions"), spendPage("other")]).toEqual(["keywords", "rank", "ai", "dashboard"]);
  });

  it("measures a series' change", () => {
    expect(seriesChange([{ date: "a", value: 50 }, { date: "b", value: null }, { date: "c", value: 60 }])).toBe(20);
    expect(seriesChange([{ date: "a", value: 0 }, { date: "b", value: 5 }])).toBeNull();
    expect(seriesChange([{ date: "a", value: 5 }])).toBeNull();
  });
});
