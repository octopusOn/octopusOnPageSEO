import { describe, expect, it } from "vitest";
import type { PageRow, RankedKeyword, RankMetrics } from "../../api/types";
import { brandSplit, brandTerms, featureShares, folderOf, intentShares, isBranded, subfolders } from "./organic";

const metrics = (traffic: number, keywords: number): RankMetrics => ({
  keywords,
  traffic,
  traffic_value_usd: traffic * 2,
  pos_1: 1,
  pos_2_3: 1,
  pos_4_10: 2,
  pos_11_20: 0,
  pos_21_100: 0,
  new: 0,
  up: 0,
  down: 0,
  lost: 0,
});

const keyword = (text: string, traffic: number, intent: string | null, features: string[] = []): RankedKeyword => ({
  keyword: text,
  position: 3,
  absolute_position: 3,
  previous_position: null,
  change: "same",
  url: null,
  volume: 1000,
  difficulty: 20,
  cpc: 1,
  competition: 0.5,
  intent,
  traffic,
  traffic_value_usd: 0,
  serp_features: features,
});

describe("Site Explorer extras", () => {
  it("sums pages by their first folder", () => {
    expect(folderOf("https://ornek.com/blog/spor/x")).toBe("/blog/");
    expect(folderOf("https://ornek.com/hakkimizda")).toBe("/");
    expect(folderOf("https://ornek.com/")).toBe("/");
    expect(folderOf("/kategori/ayakkabi")).toBe("/kategori/");
    const pages: PageRow[] = [
      { url: "https://ornek.com/blog/a", organic: metrics(100, 10) },
      { url: "https://ornek.com/blog/b", organic: metrics(50, 5) },
      { url: "https://ornek.com/urun/x", organic: metrics(400, 3) },
      { url: "https://ornek.com/", organic: metrics(20, 2) },
    ];
    const rows = subfolders(pages);
    expect(rows.map((row) => row.folder)).toEqual(["/urun/", "/blog/", "/"]);
    expect(rows[1]).toEqual({ folder: "/blog/", pages: 2, traffic: 150, keywords: 15, top10: 8, value: 300 });
  });

  it("shares keywords and traffic by intent and result-page element", () => {
    const rows = [keyword("a", 10, "informational", ["ai_overview", "people_also_ask"]), keyword("b", 30, "transactional", ["ai_overview", "organic"]), keyword("c", 5, null)];
    const intents = intentShares(rows);
    expect(intents.map((share) => share.key)).toEqual(["informational", "navigational", "commercial", "transactional", "unknown"]);
    expect(intents[3]).toEqual({ key: "transactional", keywords: 1, traffic: 30 });
    expect(intentShares([keyword("a", 1, "commercial")]).some((share) => share.key === "unknown")).toBe(false);
    expect(featureShares(rows)).toEqual([
      { key: "ai_overview", keywords: 2, traffic: 40 },
      { key: "people_also_ask", keywords: 1, traffic: 10 },
    ]);
  });

  it("tells searches for the brand from the rest", () => {
    const terms = brandTerms("www.ornekmagaza.com", ["Örnek Mağaza", "OM"]);
    expect(terms).toEqual(["ornek magaza", "ornekmagaza"]);
    expect(isBranded("örnek mağaza iade", terms)).toBe(true);
    expect(isBranded("ORNEKMAGAZA kargo", terms)).toBe(true);
    expect(isBranded("spor ayakkabı", terms)).toBe(false);
    const split = brandSplit([keyword("örnek mağaza", 70, null), keyword("spor ayakkabı", 30, null)], terms);
    expect(split.branded).toEqual({ key: "branded", keywords: 1, traffic: 70 });
    expect(split.other.traffic).toBe(30);
    expect(brandSplit([keyword("örnek mağaza", 70, null)], []).branded.keywords).toBe(0);
  });
});
