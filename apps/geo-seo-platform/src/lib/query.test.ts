import { describe, expect, it } from "vitest";
import { presetOn, togglePreset, type FilterPreset } from "./FilterBar";
import { askedQuery, EMPTY_QUERY, filterRows, isEmptyQuery, pageRange, parseValue, passes, withFilter, type Accessors, type QueryFilter } from "./query";

interface Row {
  keyword: string;
  volume: number | null;
  features: string[];
  isNew: boolean;
}

const rows: Row[] = [
  { keyword: "Spor Ayakkabı", volume: 1200, features: ["ai_overview"], isNew: true },
  { keyword: "ayakkabı bakımı", volume: 90, features: [], isNew: false },
  { keyword: "koşu bandı", volume: null, features: ["video"], isNew: false },
];

const read: Accessors<Row> = {
  keyword: (row) => row.keyword,
  volume: (row) => row.volume,
  features: (row) => row.features,
  new: (row) => row.isNew,
};

describe("list queries", () => {
  it("asks nothing for an empty query, so the question stays the same", () => {
    expect(isEmptyQuery(undefined)).toBe(true);
    expect(isEmptyQuery(EMPTY_QUERY)).toBe(true);
    expect(askedQuery(EMPTY_QUERY)).toEqual({});
    const query = { filters: [], order: [{ field: "volume", direction: "desc" as const }], offset: 0 };
    expect(askedQuery(query)).toEqual({ query });
    expect(askedQuery({ ...EMPTY_QUERY, offset: 100 })).toEqual({ query: { ...EMPTY_QUERY, offset: 100 } });
  });

  it("reads typed values in the shape their field takes", () => {
    expect(parseValue("number", "ge", " 1 000 ")).toBe(1000);
    expect(parseValue("number", "ge", "0,5")).toBe(0.5);
    expect(parseValue("number", "ge", "çok")).toBeNull();
    expect(parseValue("number", "ge", "")).toBeNull();
    expect(parseValue("text", "contains", "  ayakkabı ")).toBe("ayakkabı");
    expect(parseValue("text", "in", "a, b,\nc")).toEqual(["a", "b", "c"]);
    expect(parseValue("text", "regex", "(")).toBeNull();
    expect(parseValue("flag", "eq", "false")).toBe(false);
    expect(parseValue("flag", "eq", "true")).toBe(true);
  });

  it("replaces a filter on the same field and operator", () => {
    const first: QueryFilter = { field: "volume", op: "ge", value: 100 };
    const filters = withFilter(withFilter([], first), { field: "volume", op: "le", value: 500 });
    expect(withFilter(filters, { field: "volume", op: "ge", value: 200 })).toEqual([
      { field: "volume", op: "le", value: 500 },
      { field: "volume", op: "ge", value: 200 },
    ]);
  });

  it("runs filters here as DataForSEO would", () => {
    expect(filterRows(rows, [{ field: "keyword", op: "contains", value: "AYAKKABI" }], read).map((row) => row.keyword)).toEqual(["Spor Ayakkabı", "ayakkabı bakımı"]);
    expect(filterRows(rows, [{ field: "volume", op: "ge", value: 100 }], read)).toHaveLength(1);
    // Empty values pass no filter.
    expect(filterRows(rows, [{ field: "volume", op: "lt", value: 100 }], read)).toHaveLength(1);
    expect(filterRows(rows, [{ field: "features", op: "has", value: "ai_overview" }], read)).toHaveLength(1);
    expect(filterRows(rows, [{ field: "features", op: "has_not", value: "ai_overview" }], read)).toHaveLength(2);
    expect(filterRows(rows, [{ field: "new", op: "eq", value: true }], read)).toHaveLength(1);
    expect(filterRows(rows, [{ field: "keyword", op: "in", value: ["koşu bandı"] }], read)).toHaveLength(1);
    expect(filterRows(rows, [{ field: "keyword", op: "starts_with", value: "spor" }], read)).toHaveLength(1);
    expect(filterRows(rows, [{ field: "keyword", op: "regex", value: "^ayak" }], read)).toHaveLength(1);
    expect(passes(rows[0]!, { field: "unknown", op: "eq", value: 1 }, read)).toBe(true);
    expect(
      filterRows(
        rows,
        [
          { field: "keyword", op: "not_contains", value: "bakım" },
          { field: "volume", op: "gt", value: 0 },
        ],
        read,
      ),
    ).toHaveLength(1);
  });

  it("counts pages from one", () => {
    expect(pageRange(0, 100, 3052)).toEqual({ from: 1, to: 100 });
    expect(pageRange(3000, 100, 3052)).toEqual({ from: 3001, to: 3052 });
    expect(pageRange(0, 100, 0)).toEqual({ from: 0, to: 0 });
  });

  it("switches presets on and off", () => {
    const preset: FilterPreset = {
      id: "band",
      label: "4–10",
      filters: [
        { field: "position", op: "ge", value: 4 },
        { field: "position", op: "le", value: 10 },
      ],
    };
    const on = togglePreset(preset, [{ field: "volume", op: "ge", value: 10 }]);
    expect(on).toHaveLength(3);
    expect(presetOn(preset, on)).toBe(true);
    const off = togglePreset(preset, on);
    expect(off).toEqual([{ field: "volume", op: "ge", value: 10 }]);
    expect(presetOn(preset, off)).toBe(false);
  });
});
