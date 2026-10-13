// The simple query form: rows of field / operator / value(s) plus grouping options. The
// query text stays the source of truth: the form generates text, and text is read back into
// the form when it can be shown as one (see `formFromQuery`). Generated text follows the
// quoting rules of the query language's printer (crates/seonexus-query/src/dsl/printer.rs);
// `filterForm.fixtures.json` is checked against the Rust parser as well.

import type { AstExpr, AstLiteral, AstPredicate, FieldKind, QueryAst } from "../api/types";

export type FilterOp =
  | "eq"
  | "neq"
  | "contains"
  | "not_contains"
  | "starts"
  | "ends"
  | "pattern"
  | "regex"
  | "range"
  | "class"
  | "last_days"
  | "is";

export interface FilterRow {
  field: string;
  op: FilterOp;
  /** Value of single-value operators (contains, starts, range bounds aside, …). */
  value: string;
  /** Values of `eq` / `neq` (any of them matches). */
  values: string[];
  /** Lower bound of `range` (inclusive), empty for none. */
  min: string;
  /** Upper bound of `range` (inclusive), empty for none. */
  max: string;
}

export interface FilterForm {
  /** `and`: every row must hold; `or`: any row. */
  mode: "and" | "or";
  rows: FilterRow[];
  group: string[];
  metrics: string[];
  sort: { column: string; direction: "asc" | "desc" }[];
  /** Row limit as typed; empty for none. */
  limit: string;
}

export const EMPTY_FORM: FilterForm = { mode: "and", rows: [], group: [], metrics: [], sort: [], limit: "" };

/** Text fields usually picked from a list of exact values rather than typed in part. */
const EXACT_FIRST = new Set(["bot", "ua", "method", "host", "protocol", "ip"]);

/** Operators offered per field; the first one is the default. */
export function operatorsFor(field: string, kind: FieldKind): FilterOp[] {
  if (field === "bot_category" || field === "verification") return ["eq", "neq"];
  switch (kind) {
    case "text":
      return EXACT_FIRST.has(field)
        ? ["eq", "neq", "contains", "not_contains", "starts", "ends", "pattern", "regex"]
        : ["contains", "eq", "starts", "ends", "not_contains", "neq", "pattern", "regex"];
    case "integer":
      return field === "status" ? ["eq", "class", "range", "neq"] : ["range", "eq", "neq"];
    case "date":
      return ["range", "last_days", "eq"];
    case "boolean":
      return ["is"];
  }
}

export function newRow(field: string, kind: FieldKind): FilterRow {
  const op = operatorsFor(field, kind)[0]!;
  return { field, op, value: op === "is" ? "true" : op === "last_days" ? "30" : "", values: [], min: "", max: "" };
}

// ---------------------------------------------------------------- form → text

const isBareChar = (c: string) => !/\s/.test(c) && !'(),"'.includes(c);

/** A text value as the printer writes it: bare when possible, quoted otherwise. */
export function quoteValue(value: string): string {
  const bare = value.length > 0 && [...value].every(isBareChar) && !/^[~><]/.test(value);
  if (bare) return value;
  return `"${value.replace(/[\\"]/g, (c) => `\\${c}`)}"`;
}

/** Whether `text` is a whole number, the only value integer fields accept. */
export const isInteger = (text: string) => /^\d+$/.test(text.trim());
const isDate = (text: string) => /^\d{4}-\d{2}-\d{2}$/.test(text.trim());

/** Query text of one row; empty when the row is incomplete or invalid. */
export function rowToText(row: FilterRow, kind: FieldKind): string {
  const field = row.field;
  const value = row.value.trim();
  const ordered = kind === "integer" || kind === "date";
  const valid = kind === "integer" ? isInteger : kind === "date" ? isDate : () => true;
  switch (row.op) {
    case "eq":
    case "neq": {
      const items = row.values.map((item) => item.trim()).filter((item) => item.length > 0);
      if (items.length === 0 || (ordered && !items.every(valid))) return "";
      let text: string;
      if (!ordered && items.some((item) => item.includes("*"))) {
        // `*` patterns cannot share a value list, so every value becomes its own condition.
        const parts = items.map((item) => `${field}:${quoteValue(item)}`);
        text = parts.length > 1 ? `(${parts.join(" OR ")})` : parts[0]!;
      } else {
        text = `${field}:${items.map((item) => (ordered ? item : quoteValue(item))).join(",")}`;
      }
      return row.op === "neq" ? `-${text}` : text;
    }
    case "contains":
    case "not_contains":
      if (!value) return "";
      return `${row.op === "not_contains" ? "-" : ""}${field}:${quoteValue(`*${value}*`)}`;
    case "starts":
      return value ? `${field}:${quoteValue(`${value}*`)}` : "";
    case "ends":
      return value ? `${field}:${quoteValue(`*${value}`)}` : "";
    case "pattern":
      return value.includes("*") ? `${field}:${quoteValue(value)}` : "";
    case "regex":
      return value ? `${field}:~${quoteValue(value)}` : "";
    case "range": {
      const min = row.min.trim();
      const max = row.max.trim();
      if ((min && !valid(min)) || (max && !valid(max))) return "";
      if (min && max) return `${field}:${min}..${max}`;
      if (min) return `${field}:>=${min}`;
      if (max) return `${field}:<=${max}`;
      return "";
    }
    case "class":
      return /^[1-5]$/.test(value) ? `${field}:${value}xx` : "";
    case "last_days":
      return isInteger(value) && Number(value) > 0 ? `${field}:last${Number(value)}d` : "";
    case "is":
      return value === "true" || value === "false" ? `${field}:${value}` : "";
  }
}

/** Query text of the whole form. */
export function formToText(form: FilterForm, kinds: Record<string, FieldKind>): string {
  const conditions = form.rows
    .map((row) => rowToText(row, kinds[row.field] ?? "text"))
    .filter((text) => text.length > 0);
  const parts: string[] = [];
  if (conditions.length > 0) {
    parts.push(
      form.mode === "or" && conditions.length > 1 ? `(${conditions.join(" OR ")})` : conditions.join(" "),
    );
  }
  if (form.group.length > 0) parts.push(`group:${form.group.join(",")}`);
  if (form.metrics.length > 0) parts.push(`metric:${form.metrics.join(",")}`);
  if (form.sort.length > 0) parts.push(`sort:${form.sort.map((key) => `${key.column}:${key.direction}`).join(",")}`);
  if (isInteger(form.limit) && Number(form.limit) > 0) parts.push(`limit:${Number(form.limit)}`);
  return parts.join(" ");
}

// ---------------------------------------------------------------- syntax tree → form

function literalText(literal: AstLiteral): string {
  if ("text" in literal) return literal.text;
  if ("integer" in literal) return String(literal.integer);
  if ("date" in literal) return literal.date;
  return String(literal.boolean);
}

function addDays(date: string, days: number): string {
  const value = new Date(`${date}T00:00:00Z`);
  value.setUTCDate(value.getUTCDate() + days);
  return value.toISOString().slice(0, 10);
}

/** Shifts an exclusive bound to the inclusive one the form shows (`>9` → `>=10`). */
function inclusive(literal: AstLiteral, step: 1 | -1): string {
  if ("integer" in literal) return String(literal.integer + step);
  if ("date" in literal) return addDays(literal.date, step);
  return literalText(literal);
}

function globRow(field: string, pattern: string): FilterRow {
  const row = (op: FilterOp, value: string): FilterRow => ({ field, op, value, values: [], min: "", max: "" });
  const inner = pattern.slice(1, -1);
  if (pattern.length >= 3 && pattern.startsWith("*") && pattern.endsWith("*") && !inner.includes("*")) {
    return row("contains", inner);
  }
  const head = pattern.slice(0, -1);
  if (pattern.endsWith("*") && head.length > 0 && !head.includes("*")) return row("starts", head);
  const tail = pattern.slice(1);
  if (pattern.startsWith("*") && tail.length > 0 && !tail.includes("*")) return row("ends", tail);
  return row("pattern", pattern);
}

function predicateRow(field: string, predicate: AstPredicate): FilterRow {
  const row = (op: FilterOp, value = "", min = "", max = ""): FilterRow => ({ field, op, value, values: [], min, max });
  if ("one_of" in predicate) {
    const [first] = predicate.one_of;
    if (first && "boolean" in first) return row("is", String(first.boolean));
    return { ...row("eq"), values: predicate.one_of.map(literalText) };
  }
  if ("glob" in predicate) return globRow(field, predicate.glob);
  if ("regex" in predicate) return row("regex", predicate.regex);
  if ("between" in predicate) return row("range", "", literalText(predicate.between[0]), literalText(predicate.between[1]));
  if ("status_class" in predicate) return row("class", String(predicate.status_class));
  if ("last_days" in predicate) return row("last_days", String(predicate.last_days));
  const [op, literal] = predicate.compare;
  switch (op) {
    case "ge":
      return row("range", "", literalText(literal));
    case "gt":
      return row("range", "", inclusive(literal, 1));
    case "le":
      return row("range", "", "", literalText(literal));
    case "lt":
      return row("range", "", "", inclusive(literal, -1));
  }
}

/** Values of a condition, or of an OR of conditions, on a single text field. */
function fieldValues(expr: AstExpr): { field: string; values: string[] } | null {
  if ("condition" in expr) {
    const { field, predicate } = expr.condition;
    if ("glob" in predicate) return { field, values: [predicate.glob] };
    if ("one_of" in predicate && !predicate.one_of.some((literal) => "boolean" in literal)) {
      return { field, values: predicate.one_of.map(literalText) };
    }
    return null;
  }
  if ("or" in expr) {
    const parts = expr.or.map(fieldValues);
    const field = parts[0]?.field;
    if (!field || parts.some((part) => !part || part.field !== field)) return null;
    return { field, values: parts.flatMap((part) => part?.values ?? []) };
  }
  return null;
}

function exprRow(expr: AstExpr): FilterRow | null {
  const listRow = (op: "eq" | "neq", found: { field: string; values: string[] }): FilterRow => ({
    field: found.field,
    op,
    value: "",
    values: found.values,
    min: "",
    max: "",
  });
  if ("condition" in expr) return predicateRow(expr.condition.field, expr.condition.predicate);
  if ("or" in expr) {
    const found = fieldValues(expr);
    return found ? listRow("eq", found) : null;
  }
  if ("not" in expr) {
    if ("condition" in expr.not) {
      const row = predicateRow(expr.not.condition.field, expr.not.condition.predicate);
      if (row.op === "eq") return { ...row, op: "neq" };
      if (row.op === "contains") return { ...row, op: "not_contains" };
    }
    const found = fieldValues(expr.not);
    return found ? listRow("neq", found) : null;
  }
  return null;
}

/**
 * The form showing `query`, or `null` when the query cannot be shown as a form (nested
 * groups, negated ranges, …) and must be edited as text.
 */
export function formFromQuery(query: QueryAst): FilterForm | null {
  let mode: FilterForm["mode"] = "and";
  let operands: AstExpr[] = [];
  if (query.filter) {
    if ("and" in query.filter) operands = query.filter.and;
    // `(bot:googlebot* OR bot:bingbot)` is one value list, as the form wrote it.
    else if ("or" in query.filter && fieldValues(query.filter)) operands = [query.filter];
    else if ("or" in query.filter) {
      mode = "or";
      operands = query.filter.or;
    } else operands = [query.filter];
  }
  const rows: FilterRow[] = [];
  for (const operand of operands) {
    const row = exprRow(operand);
    if (!row) return null;
    rows.push(row);
  }
  return {
    mode,
    rows,
    group: [...query.group],
    metrics: [...query.metrics],
    sort: query.sort.map((key) => ({ ...key })),
    limit: query.limit === null ? "" : String(query.limit),
  };
}
