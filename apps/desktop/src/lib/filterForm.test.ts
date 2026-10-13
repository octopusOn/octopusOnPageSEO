import { describe, expect, it } from "vitest";

import type { FieldKind, QueryAst } from "../api/types";
import fixtures from "./filterForm.fixtures.json";
import { EMPTY_FORM, formFromQuery, formToText, newRow, quoteValue, rowToText, type FilterForm } from "./filterForm";

const KINDS: Record<string, FieldKind> = {
  date: "date",
  status: "integer",
  bytes: "integer",
  response_ms: "integer",
  params: "boolean",
};

const cases = fixtures as unknown as { name: string; text: string; form: FilterForm; query: QueryAst }[];

describe("filter form", () => {
  for (const fixture of cases) {
    it(`generates and reads back: ${fixture.name}`, () => {
      expect(formToText(fixture.form, KINDS)).toBe(fixture.text);
      expect(formFromQuery(fixture.query)).toEqual(fixture.form);
    });
  }

  it("quotes like the query printer", () => {
    expect(quoteValue("/blog/*")).toBe("/blog/*");
    expect(quoteValue("a b")).toBe('"a b"');
    expect(quoteValue(">x")).toBe('">x"');
    expect(quoteValue('say "hi" \\ ok')).toBe('"say \\"hi\\" \\\\ ok"');
    expect(quoteValue("")).toBe('""');
  });

  it("skips incomplete or invalid rows", () => {
    const row = (op: string, value = "", min = "", max = "", values: string[] = []) =>
      ({ field: "status", op, value, values, min, max }) as never;
    expect(rowToText(row("range", "", "4x"), "integer")).toBe("");
    expect(rowToText(row("range"), "integer")).toBe("");
    expect(rowToText(row("eq", "", "", "", ["200", "abc"]), "integer")).toBe("");
    expect(rowToText(row("eq", "", "", "", ["200", " ", "301"]), "integer")).toBe("status:200,301");
    expect(rowToText(row("class", "9"), "integer")).toBe("");
    expect(formToText({ ...EMPTY_FORM, rows: [newRow("url", "text")], limit: "0" }, KINDS)).toBe("");
  });

  it("shows exclusive comparisons as inclusive bounds", () => {
    const form = formFromQuery({
      filter: { and: [
        { condition: { field: "bytes", predicate: { compare: ["gt", { integer: 9 }] } } },
        { condition: { field: "date", predicate: { compare: ["lt", { date: "2026-10-01" }] } } },
      ] },
      group: [],
      metrics: [],
      sort: [],
      limit: null,
    });
    expect(form?.rows.map((r) => [r.min, r.max])).toEqual([["10", ""], ["", "2026-09-30"]]);
  });

  it("refuses queries a form cannot show", () => {
    const nested: QueryAst = {
      filter: { and: [
        { or: [
          { condition: { field: "status", predicate: { one_of: [{ integer: 404 }] } } },
          { condition: { field: "bot", predicate: { glob: "googlebot*" } } },
        ] },
      ] },
      group: [],
      metrics: [],
      sort: [],
      limit: null,
    };
    expect(formFromQuery(nested)).toBeNull();
    const negatedRange: QueryAst = {
      ...nested,
      filter: { not: { condition: { field: "bytes", predicate: { between: [{ integer: 1 }, { integer: 2 }] } } } },
    };
    expect(formFromQuery(negatedRange)).toBeNull();
  });
});
