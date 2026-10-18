import { describe, expect, it } from "vitest";

import { composeQuery, currentSort, dateValue, tokenize, withSort } from "./queryBuilder";

describe("query builder", () => {
  it("composes quick filters before the query text", () => {
    expect(composeQuery({ preset: "last30d" }, "googlebot-verified", "group:url")).toBe(
      "date:last30d bot:googlebot* verification:verified group:url",
    );
    expect(composeQuery({ preset: "all" }, "all", "  ")).toBe("");
  });

  it("renders custom date ranges", () => {
    expect(dateValue({ preset: "custom", from: "2026-09-01", to: "2026-09-30" })).toBe(
      "2026-09-01..2026-09-30",
    );
    expect(dateValue({ preset: "custom", from: "2026-09-01", to: "2026-09-01" })).toBe("2026-09-01");
    expect(dateValue({ preset: "custom" })).toBeNull();
  });

  it("keeps quoted values together", () => {
    expect(tokenize('url:"a b" status:404  ua:"x \\" y"')).toEqual(['url:"a b"', "status:404", 'ua:"x \\" y"']);
  });

  it("replaces the sort directive", () => {
    const text = "status:4xx group:url sort:hits:desc limit:10";
    expect(withSort(text, "url", "asc")).toBe("status:4xx group:url limit:10 sort:url:asc");
    expect(currentSort(text)).toEqual({ column: "hits", direction: "desc" });
    expect(currentSort("group:url")).toBeNull();
  });
});
