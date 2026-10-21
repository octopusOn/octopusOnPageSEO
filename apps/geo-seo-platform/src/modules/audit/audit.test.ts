import { describe, expect, it } from "vitest";
import type { CategoryReport, IssueReport } from "../../api/types";
import { ALL_ISSUES, changeOf, creditsPerThousand, filterCategories, pageChoices, scoreChange, totals } from "./audit";

function issue(id: string, kind: IssueReport["kind"], count: number, previous: number | null = null): IssueReport {
  return { id, title: id.replace(/-/g, " "), kind, priority: "medium", count, previous };
}

const CATEGORIES: CategoryReport[] = [
  { id: "titles", name: "Sayfa başlıkları", issues: [issue("title-missing", "issue", 3, 9), issue("title-too-long", "warning", 57)] },
  { id: "canonicals", name: "Canonical", issues: [issue("canonical-missing", "warning", 0, 12)] },
  { id: "ai", name: "AI hazırlığı", issues: [issue("llms-txt-missing", "opportunity", 1)] },
];

describe("site audit", () => {
  it("offers page limits up to the host's", () => {
    expect(pageChoices(5000)).toEqual([100, 500, 1000, 5000]);
    expect(pageChoices(100_000).at(-1)).toBe(100_000);
  });

  it("filters by kind and folded text, hiding fixed checks unless asked", () => {
    const ids = (categories: CategoryReport[]) => categories.flatMap((category) => category.issues.map((entry) => entry.id));
    expect(ids(filterCategories(CATEGORIES, ALL_ISSUES))).toEqual(["title-missing", "title-too-long", "llms-txt-missing"]);
    expect(ids(filterCategories(CATEGORIES, { ...ALL_ISSUES, openOnly: false }))).toContain("canonical-missing");
    expect(ids(filterCategories(CATEGORIES, { ...ALL_ISSUES, kind: "warning" }))).toEqual(["title-too-long"]);
    expect(ids(filterCategories(CATEGORIES, { ...ALL_ISSUES, query: "SAYFA BASLIK" }))).toEqual(["title-missing", "title-too-long"]);
    expect(filterCategories(CATEGORIES, { ...ALL_ISSUES, query: "nothing" })).toEqual([]);
  });

  it("counts open checks by kind and changes since the audit before", () => {
    expect(totals(CATEGORIES)).toEqual({ issue: 1, warning: 1, opportunity: 1 });
    expect(changeOf(issue("a", "issue", 3, 9))).toBe(-6);
    expect(changeOf(issue("a", "issue", 3))).toBeNull();
    expect(scoreChange(78.44, 74.9)).toBe(3.5);
    expect(scoreChange(78, null)).toBeNull();
  });

  it("prices a thousand pages in credits", () => {
    expect(creditsPerThousand({ micros_per_unit: 200, free_per_month: 1000, credit_unit_micros: 1000 })).toBe(200);
  });
});
