import { describe, expect, it } from "vitest";
import type { AiCell, AiPromptRow, Brand } from "../../api/types";
import { cellState, csvRows, filterPrompts, highlight, parsePrompts, platformName, tagsOf, toneOf } from "./ai";

const cell = (extra: Partial<AiCell> = {}): AiCell => ({
  mentioned: false,
  position: null,
  cited: false,
  sentiment: null,
  error: null,
  checked_at: "2026-10-21T09:00:00Z",
  ...extra,
});

const own: Brand = { key: "ornekmagaza.com", names: ["Örnek Mağaza", "ornekmagaza.com", "ornekmagaza"], own: true };
const rival: Brand = { key: "rakip-magaza.com", names: ["Rakip Mağaza", "rakip-magaza.com", "rakip-magaza"], own: false };

describe("ai visibility", () => {
  it("reads one question per line once", () => {
    expect(parsePrompts("  En iyi  spor ayakkabı? \n\nen İyi spor ayakkabı?\n" + "x".repeat(501) + "\nNereden alınır?")).toEqual([
      "En iyi spor ayakkabı?",
      "Nereden alınır?",
    ]);
  });

  it("tells cited, mentioned, absent and failed answers apart", () => {
    expect(cellState(undefined)).toBe("missing");
    expect(cellState(cell({ error: "HTTP 500" }))).toBe("error");
    expect(cellState(cell({ mentioned: true, cited: true }))).toBe("cited");
    expect(cellState(cell({ mentioned: true, position: 2 }))).toBe("mentioned");
    expect(cellState(cell())).toBe("absent");
    expect([toneOf(0.4), toneOf(0), toneOf(-0.5), toneOf(null)]).toEqual(["positive", "neutral", "negative", null]);
    expect([platformName("chat_gpt"), platformName("google"), platformName("new_one")]).toEqual(["ChatGPT", "Google AI Overview", "new one"]);
  });

  it("highlights brands as whole words in any case or accent", () => {
    const segments = highlight("RAKİP MAĞAZA ve örnek mağaza; ornekmagazacılar değil. ornekmagaza.com da var.", [own, rival]);
    const named = segments.filter((segment) => segment.brand).map((segment) => [segment.text, segment.brand!.key]);
    expect(named).toEqual([
      ["RAKİP MAĞAZA", "rakip-magaza.com"],
      ["örnek mağaza", "ornekmagaza.com"],
      ["ornekmagaza.com", "ornekmagaza.com"],
    ]);
    expect(segments.map((segment) => segment.text).join("")).toBe("RAKİP MAĞAZA ve örnek mağaza; ornekmagazacılar değil. ornekmagaza.com da var.");
    expect(highlight("Hiçbiri yok.", [own])).toEqual([{ text: "Hiçbiri yok.", brand: null }]);
  });

  it("filters questions and writes the matrix as CSV", () => {
    const rows: AiPromptRow[] = [
      { prompt_id: 1, prompt: "En iyi spor ayakkabı?", tags: ["ayakkabı"], results: { claude: cell({ mentioned: true, position: 2 }), chatgpt: cell() } },
      { prompt_id: 2, prompt: "Örnek Mağaza güvenilir mi?", tags: ["marka"], results: {} },
    ];
    expect(tagsOf(rows)).toEqual(["ayakkabı", "marka"]);
    expect(filterPrompts(rows, "ORNEK", "").map((row) => row.prompt_id)).toEqual([2]);
    expect(filterPrompts(rows, "", "ayakkabı").map((row) => row.prompt_id)).toEqual([1]);
    expect(csvRows(rows, ["chatgpt", "claude"], (state) => state)).toEqual([
      ["En iyi spor ayakkabı?", "ayakkabı", "absent", "mentioned (2)"],
      ["Örnek Mağaza güvenilir mi?", "marka", "missing", "missing"],
    ]);
  });
});
