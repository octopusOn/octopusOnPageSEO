// A sortable, paged table for report rows. Filtering is the page's: it passes the rows to show.

import { useMemo, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight } from "lucide-react";

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
}

export type SortDirection = "asc" | "desc";

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

export function DataTable<T>({
  rows,
  columns,
  rowKey,
  initialSort,
  pageSize = 50,
  empty,
  labels,
}: {
  rows: readonly T[];
  columns: Column<T>[];
  rowKey: (row: T) => string;
  initialSort?: { key: string; direction: SortDirection };
  pageSize?: number;
  empty: ReactNode;
  labels: { previous: string; next: string; page: (from: number, to: number, total: number) => string; sortBy: (label: string) => string };
}) {
  const [sort, setSort] = useState(initialSort ?? null);
  const [page, setPage] = useState(0);
  const sorted = useMemo(() => {
    if (!sort) return [...rows];
    const column = columns.find((entry) => entry.key === sort.key);
    return column?.sort ? sortRows(rows, column.sort, sort.direction) : [...rows];
  }, [rows, columns, sort]);
  const pages = Math.max(1, Math.ceil(sorted.length / pageSize));
  const current = Math.min(page, pages - 1);
  const shown = sorted.slice(current * pageSize, (current + 1) * pageSize);
  const toggle = (column: Column<T>) => {
    if (!column.sort) return;
    setPage(0);
    setSort((previous) =>
      previous?.key === column.key
        ? { key: column.key, direction: previous.direction === "desc" ? "asc" : "desc" }
        : { key: column.key, direction: column.numeric ? "desc" : "asc" },
    );
  };
  if (rows.length === 0) return <div className="ui-table-empty">{empty}</div>;
  return (
    <div className="ui-table">
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              {columns.map((column) => {
                const direction = sort?.key === column.key ? sort.direction : null;
                return (
                  <th
                    key={column.key}
                    className={column.numeric ? "number" : undefined}
                    style={column.width ? { width: column.width } : undefined}
                    lang={column.lang}
                    aria-sort={direction ? (direction === "asc" ? "ascending" : "descending") : undefined}
                  >
                    {column.sort ? (
                      <button type="button" className="ui-sort" onClick={() => toggle(column)} title={labels.sortBy(column.label)}>
                        {column.label}
                        {direction && (direction === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
                      </button>
                    ) : (
                      column.label
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {shown.map((row) => (
              <tr key={rowKey(row)}>
                {columns.map((column) => (
                  <td key={column.key} className={column.numeric ? "number" : undefined}>
                    {column.render(row)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {pages > 1 && (
        <div className="ui-pager">
          <span>{labels.page(current * pageSize + 1, Math.min(sorted.length, (current + 1) * pageSize), sorted.length)}</span>
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
