import { describe, expect, it } from "vitest";
import type { ContentBrief } from "../../api/types";
import { briefMarkdown, countWords, inlineParts, markdownBlocks, rangePlace, scoreBand } from "./brief";

describe("content", () => {
  it("bands scores and places numbers against the ranking pages", () => {
    expect([scoreBand(82), scoreBand(70), scoreBand(52), scoreBand(12)]).toEqual(["good", "good", "fair", "poor"]);
    const spread = { low: 800, median: 1200, high: 1600 };
    expect([rangePlace(500, spread), rangePlace(1200, spread), rangePlace(2000, spread), rangePlace(10, null)]).toEqual(["below", "within", "above", null]);
  });

  it("reads Markdown into blocks without HTML", () => {
    const blocks = markdownBlocks("# Brief\n\nGiriş satırı\ndevamı.\n\n## Başlık ##\n- bir\n  - iki\n1. üç\n> alıntı\n---\n<script>x</script>");
    expect(blocks).toEqual([
      { kind: "heading", level: 1, text: "Brief" },
      { kind: "paragraph", level: 0, text: "Giriş satırı devamı." },
      { kind: "heading", level: 2, text: "Başlık" },
      { kind: "item", level: 0, text: "bir" },
      { kind: "item", level: 1, text: "iki" },
      { kind: "number", level: 0, text: "üç" },
      { kind: "quote", level: 0, text: "alıntı" },
      { kind: "rule", level: 0, text: "" },
      { kind: "paragraph", level: 0, text: "<script>x</script>" },
    ]);
    const plain = { strong: false, em: false, code: false };
    expect(inlineParts("Önce **kalın** sonra `kod`, *eğik* 2 * 3 son")).toEqual([
      { ...plain, text: "Önce " },
      { ...plain, text: "kalın", strong: true },
      { ...plain, text: " sonra " },
      { ...plain, text: "kod", code: true },
      { ...plain, text: ", " },
      { ...plain, text: "eğik", em: true },
      { ...plain, text: " 2 * 3 son" },
    ]);
  });

  it("counts words and writes a brief out", () => {
    expect(countWords("# Spor ayakkabı\n- hafif taban, 2026 — iyi")).toBe(6);
    const brief: ContentBrief = {
      keyword: "spor ayakkabı",
      intent: "commercial",
      words: { low: 900, median: 1200, high: 1500 },
      h2: { low: 4, median: 6, high: 8 },
      title: { median_chars: 54.4, median_px: 480, keyword_first: 0.6, with_year: 0.3, with_number: 0.2 },
      headings: ["Taban"],
      outline: ["Taban", "Hangi numara?"],
      questions: ["Hangi numara?"],
      ideas: [],
      schema: ["Product"],
      short_answer: false,
      terms: [{ term: "taban", pages: 8, share: 0.8, average: 3.2 }],
      competitors: [{ position: 1, url: "https://a.com/", domain: "a.com", title: "A", words: 1000, h2: 5, score: 70, status: 200 }],
      internal_links: [{ url: "https://ornek.com/kosu", title: "Koşu", relevance: 0.5 }],
    };
    const text = briefMarkdown(brief, { title: "Title", words: "Words", h2: "H2", outline: "Outline", terms: "Terms", questions: "Questions", schema: "Schema", links: "Links", competitors: "Pages" });
    expect(text).toContain("# spor ayakkabı\n");
    expect(text).toContain("- Words: 900–1500 (1200)");
    expect(text).toContain("## Terms\n\n- taban (80%, ~3.2)");
    expect(text).toContain("- Koşu — https://ornek.com/kosu");
    expect(text).toContain("- 1. A — https://a.com/");
  });
});
