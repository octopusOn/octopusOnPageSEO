import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Breakdown } from "../api/types";
import { BreakdownList, ScoreRing } from "./Score";

const difficulty: Breakdown = {
  score: 46.3,
  model: "kd-1",
  confidence: 0.8,
  components: [
    { id: "competition", weight: 1, value: 0.45, normalized: 0.45, contribution: 45, reason: "kd.competition", params: { measured: 9, results: 10 } },
    { id: "forums", weight: 0, value: 0.1, contribution: -1, reason: "kd.forums" },
    { id: "pagerank", weight: 30, normalized: null, contribution: 0, reason: "site.pagerank.unknown", params: { open_page_rank: null } },
  ],
};

it("shows a bounded meter for any score", () => {
  for (const [value, shown] of [[46.3, "46.3"], [140, "100"], [-5, "0"]] as const) {
    const html = renderToStaticMarkup(<ScoreRing value={value} label="KD" tone="warning" />);
    expect(html).toContain('role="meter"');
    expect(html).toContain(`aria-valuenow="${shown}"`);
  }
});

it("explains every component in the reader's language, with missing data marked", () => {
  const html = renderToStaticMarkup(<BreakdownList breakdown={difficulty} />);
  expect(html).toContain("İlk onun gücü");
  expect(html).toContain("tıklama payıyla ağırlıklı");
  expect(html).toContain("ölçülen");
  expect(html).toContain("+45");
  expect(html).toContain("-1");
  expect(html).toContain("Veri yok");
  expect(html).toContain("kd-1");
  expect(html).not.toMatch(/NaN|undefined/);
  // Known components come first, the one without data last.
  expect(html.indexOf("İlk onun gücü")).toBeLessThan(html.indexOf("Open PageRank"));
});
