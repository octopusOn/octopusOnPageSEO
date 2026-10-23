// A sortable, paged table for report rows. Filtering is the page's: it passes the rows to show.
// With a `tableId` it remembers which optional columns are shown and how many rows a page has;
// columns can explain themselves (`help`) and add up their rows (`total`).

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Columns3 } from "lucide-react";
import { Tip } from "./hint";

export interface Column<T> {
  /** Unique key, also the sort key. */
  key: string;
  label: string;
  /** The cell. */
  render: (row: T) => ReactNode;
  /** The value sorted by; not sortable without it. */
  sort?: (row: T) => number | string | null | undefined;
  /** Right-aligned numbers. */
  numeric?: boolean;
  /** CSS width. */
  width?: string;
  /** Language of the label when it differs from the page's, for product names in capitals. */
  lang?: string;
  /** What the column means, shown as a tip beside its label. */
  help?: string;
  /** Can be hidden from the column chooser (needs `tableId`). */
  optional?: boolean;
  /** Hidden until chosen. */
  defaultHidden?: boolean;
  /** The column's total over every row shown by the page's filters (a footer row). */
  total?: (rows: readonly T[]) => ReactNode;
}

export type SortDirection = "asc" | "desc";

export interface TableLabels {
  previous: string;
  next: string;
  page: (from: number, to: number, total: number) => string;
  sortBy: (label: string) => string;
  /** The column chooser's button. */
  columns?: string;
  /** The page size choice. */
  rowsPerPage?: string;
  /** The first cell of the totals row. */
  total?: string;
  /** What a column's tip button says. */
  help?: (label: string) => string;
}

/** Rows sorted by a value; empty values last whatever the direction. */
export function sortRows<T>(rows: readonly T[], value: (row: T) => number | string | null | undefined, direction: SortDirection): T[] {
  const sign = direction === "asc" ? 1 : -1;
  return [...rows].sort((left, right) => {
    const a = value(left);
    const b = value(right);
    const aEmpty = a === null || a === undefined || a === "";
    const bEmpty = b === null || b === undefined || b === "";
    if (aEmpty || bEmpty) return aEmpty === bEmpty ? 0 : aEmpty ? 1 : -1;
    if (typeof a === "number" && typeof b === "number") return (a - b) * sign;
    return String(a).localeCompare(String(b)) * sign;
  });
}

/** What a table remembers. */
interface Kept {
  shown: string[];
  hidden: string[];
  pageSize: number | null;
}

const storageKey = (id: string) => `ui.table.v1.${id}`;

function readKept(id: string | undefined): Kept {
  const empty: Kept = { shown: [], hidden: [], pageSize: null };
  if (!id) return empty;
  try {
    const value = JSON.parse(localStorage.getItem(storageKey(id)) ?? "null") as Partial<Kept> | null;
    if (!value || typeof value !== "object") return empty;
    const list = (input: unknown) => (Array.isArray(input) ? input.filter((entry): entry is string => typeof entry === "string") : []);
    return { shown: list(value.shown), hidden: list(value.hidden), pageSize: typeof value.pageSize === "number" ? value.pageSize : null };
  } catch {
    return empty;
  }
}

function writeKept(id: string | undefined, kept: Kept) {
  if (!id) return;
  try {
    localStorage.setItem(storageKey(id), JSON.stringify(kept));
  } catch {
    // Not remembered without storage.
  }
}

/** Whether an optional column is shown: the user's choice, else its default. */
export function columnShown<T>(column: Column<T>, kept: Pick<Kept, "shown" | "hidden">): boolean {
  if (!column.optional) return true;
  if (kept.hidden.includes(column.key)) return false;
  if (kept.shown.includes(column.key)) return true;
  return !column.defaultHidden;
}

function ColumnChooser<T>({ columns, kept, label, onChange }: { columns: Column<T>[]; kept: Kept; label: string; onChange: (kept: Kept) => void }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return undefined;
    const away = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) setOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
    };
  }, [open]);
  const toggle = (column: Column<T>) => {
    const on = columnShown(column, kept);
    const shown = kept.shown.filter((key) => key !== column.key);
    const hidden = kept.hidden.filter((key) => key !== column.key);
    onChange(on ? { ...kept, shown, hidden: [...hidden, column.key] } : { ...kept, hidden, shown: [...shown, column.key] });
  };
  return (
    <div className="ui-columns" ref={box}>
      <button type="button" className="oc-btn small ghost" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Columns3 size={14} aria-hidden="true" />
        {label}
      </button>
      {open && (
        <div className="oc-menu ui-columns-menu" role="group" aria-label={label}>
          {columns
            .filter((column) => column.optional)
            .map((column) => (
              <label key={column.key} className="oc-check">
                <input type="checkbox" checked={columnShown(column, kept)} onChange={() => toggle(column)} />
                <span>{column.label}</span>
              </label>
            ))}
        </div>
      )}
    </div>
  );
}

export function DataTable<T>({
  rows,
  columns,
  rowKey,
  initialSort,
  pageSize = 50,
  pageSizes,
  tableId,
  empty,
  labels,
}: {
  rows: readonly T[];
  columns: Column<T>[];
  rowKey: (row: T) => string;
  initialSort?: { key: string; direction: SortDirection };
  pageSize?: number;
  /** Page sizes to choose from; none shows no choice. */
  pageSizes?: number[];
  /** Remembers shown columns and the page size under this id. */
  tableId?: string;
  empty: ReactNode;
  labels: TableLabels;
}) {
  const [sort, setSort] = useState(initialSort ?? null);
  const [page, setPage] = useState(0);
  const [kept, setKeptState] = useState<Kept>(() => readKept(tableId));
  const setKept = (next: Kept) => {
    setKeptState(next);
    writeKept(tableId, next);
  };
  const visible = columns.filter((column) => columnShown(column, kept));
  const size = kept.pageSize && (pageSizes ?? []).includes(kept.pageSize) ? kept.pageSize : pageSize;
  const sorted = useMemo(() => {
    if (!sort) return [...rows];
    const column = columns.find((entry) => entry.key === sort.key);
    return column?.sort ? sortRows(rows, column.sort, sort.direction) : [...rows];
  }, [rows, columns, sort]);
  const pages = Math.max(1, Math.ceil(sorted.length / size));
  const current = Math.min(page, pages - 1);
  const shown = sorted.slice(current * size, (current + 1) * size);
  const toggle = (column: Column<T>) => {
    if (!column.sort) return;
    setPage(0);
    setSort((previous) =>
      previous?.key === column.key
        ? { key: column.key, direction: previous.direction === "desc" ? "asc" : "desc" }
        : { key: column.key, direction: column.numeric ? "desc" : "asc" },
    );
  };
  const chooser = Boolean(tableId) && columns.some((column) => column.optional);
  const totals = visible.some((column) => column.total);
  if (rows.length === 0) return <div className="ui-table-empty">{empty}</div>;
  return (
    <div className="ui-table">
      {chooser && (
        <div className="ui-table-tools">
          <ColumnChooser columns={columns} kept={kept} label={labels.columns ?? "Columns"} onChange={setKept} />
        </div>
      )}
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              {visible.map((column) => {
                const direction = sort?.key === column.key ? sort.direction : null;
                return (
                  <th
                    key={column.key}
                    className={column.numeric ? "number" : undefined}
                    style={column.width ? { width: column.width } : undefined}
                    lang={column.lang}
                    aria-sort={direction ? (direction === "asc" ? "ascending" : "descending") : undefined}
                  >
                    <span className="ui-th">
                      {column.sort ? (
                        <button type="button" className="ui-sort" onClick={() => toggle(column)} title={labels.sortBy(column.label)}>
                          {column.label}
                          {direction && (direction === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
                        </button>
                      ) : (
                        column.label
                      )}
                      {column.help && <Tip text={column.help} label={labels.help ? labels.help(column.label) : column.label} />}
                    </span>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {shown.map((row) => (
              <tr key={rowKey(row)}>
                {visible.map((column) => (
                  <td key={column.key} className={column.numeric ? "number" : undefined}>
                    {column.render(row)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
          {totals && (
            <tfoot>
              <tr className="ui-totals">
                {visible.map((column, index) => (
                  <td key={column.key} className={column.numeric ? "number" : undefined}>
                    {column.total ? column.total(sorted) : index === 0 ? (labels.total ?? "Total") : null}
                  </td>
                ))}
              </tr>
            </tfoot>
          )}
        </table>
      </div>
      {(pages > 1 || (pageSizes && sorted.length > Math.min(...pageSizes))) && (
        <div className="ui-pager">
          {pageSizes && pageSizes.length > 1 && (
            <label className="ui-page-size">
              <span>{labels.rowsPerPage ?? "Rows"}</span>
              <select
                className="oc-select"
                value={size}
                onChange={(event) => {
                  setPage(0);
                  setKept({ ...kept, pageSize: Number(event.target.value) });
                }}
              >
                {pageSizes.map((choice) => (
                  <option key={choice} value={choice}>
                    {choice}
                  </option>
                ))}
              </select>
            </label>
          )}
          <span>{labels.page(current * size + 1, Math.min(sorted.length, (current + 1) * size), sorted.length)}</span>
          <button type="button" className="oc-btn small icon ghost" disabled={current === 0} onClick={() => setPage(current - 1)} aria-label={labels.previous}>
            <ChevronLeft size={15} />
          </button>
          <button type="button" className="oc-btn small icon ghost" disabled={current >= pages - 1} onClick={() => setPage(current + 1)} aria-label={labels.next}>
            <ChevronRight size={15} />
          </button>
        </div>
      )}
    </div>
  );
}
