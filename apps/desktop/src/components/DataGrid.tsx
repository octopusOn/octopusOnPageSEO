// Virtualized result table: renders only visible rows, loads further pages on demand and
// sorts either through the caller (server-side) or locally.

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp } from "lucide-react";

import type { Cell, Column } from "../api/types";
import { formatCell } from "../lib/format";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/en";
import { Badge } from "./ui";

const ROW_HEIGHT = 34;

export interface SortState {
  column: string;
  direction: "asc" | "desc";
}

interface DataGridProps {
  columns: Column[];
  rows: Cell[][];
  hasMore?: boolean;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  /** Server-side sorting; without it the grid sorts the loaded rows itself. */
  sort?: SortState | null;
  onSort?: (sort: SortState) => void;
  footer?: ReactNode;
}

function columnWidth(column: Column): number {
  switch (column.name) {
    case "url":
    case "path":
    case "referer":
    case "ua":
      return 440;
    case "ip":
      return 150;
    case "bot":
    case "bot_category":
    case "directory":
      return 200;
    case "verification":
    case "presence":
    case "trend":
    case "status_codes":
      return 140;
    default:
      break;
  }
  switch (column.kind) {
    case "timestamp":
      return 180;
    case "date":
      return 120;
    case "integer":
    case "float":
      return 120;
    default:
      return 220;
  }
}

const numeric = (column: Column) => column.kind === "integer" || column.kind === "float";

function compareCells(a: Cell, b: Cell): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b));
}

export function DataGrid({ columns, rows, hasMore, loadingMore, onLoadMore, sort, onSort, footer }: DataGridProps) {
  const { t, locale } = useI18n();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [localSort, setLocalSort] = useState<SortState | null>(null);
  const activeSort = onSort ? sort ?? null : localSort;

  const sortedRows = useMemo(() => {
    if (onSort || !localSort) return rows;
    const index = columns.findIndex((c) => c.name === localSort.column);
    if (index < 0) return rows;
    const factor = localSort.direction === "asc" ? 1 : -1;
    return [...rows].sort((x, y) => factor * compareCells(x[index] ?? null, y[index] ?? null));
  }, [rows, columns, localSort, onSort]);

  const virtualizer = useVirtualizer({
    count: sortedRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 20,
  });
  const items = virtualizer.getVirtualItems();
  const lastIndex = items.length > 0 ? items[items.length - 1]!.index : 0;

  useEffect(() => {
    if (hasMore && !loadingMore && onLoadMore && lastIndex >= sortedRows.length - 40) onLoadMore();
  }, [hasMore, loadingMore, onLoadMore, lastIndex, sortedRows.length]);

  const widths = useMemo(() => columns.map(columnWidth), [columns]);
  const totalWidth = widths.reduce((sum, width) => sum + width, 0);

  const toggleSort = (column: Column) => {
    const direction: "asc" | "desc" =
      activeSort?.column === column.name
        ? activeSort.direction === "asc"
          ? "desc"
          : "asc"
        : numeric(column) || column.kind === "timestamp"
          ? "desc"
          : "asc";
    const next = { column: column.name, direction };
    if (onSort) onSort(next);
    else setLocalSort(next);
  };

  return (
    <div className="grid">
      <div className="grid-scroll" ref={scrollRef}>
        <div className="grid-head" style={{ width: totalWidth }}>
          {columns.map((column, index) => (
            <div
              key={column.name}
              className={`cell ${numeric(column) ? "num" : ""}`}
              style={{ width: widths[index] }}
              onClick={() => toggleSort(column)}
              title={column.name}
            >
              {column.name}
              {activeSort?.column === column.name &&
                (activeSort.direction === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
            </div>
          ))}
        </div>
        <div style={{ height: virtualizer.getTotalSize(), position: "relative", width: totalWidth }}>
          {items.map((item) => {
            const row = sortedRows[item.index] ?? [];
            return (
              <div
                key={item.key}
                className="grid-row"
                style={{ transform: `translateY(${item.start}px)`, height: ROW_HEIGHT, width: totalWidth }}
              >
                {columns.map((column, index) => (
                  <div
                    key={column.name}
                    className={`cell ${numeric(column) ? "num" : ""}`}
                    style={{ width: widths[index] }}
                    title={typeof row[index] === "string" ? (row[index] as string) : undefined}
                  >
                    <CellView column={column} value={row[index] ?? null} locale={locale} t={t} />
                  </div>
                ))}
              </div>
            );
          })}
        </div>
        {sortedRows.length === 0 && <div className="empty">{t("common.noRows")}</div>}
      </div>
      <div className="grid-foot">
        <span>
          {hasMore
            ? t("common.moreRows", { count: sortedRows.length.toLocaleString(locale) })
            : t("common.rows", { count: sortedRows.length.toLocaleString(locale) })}
        </span>
        {loadingMore && <span className="spinner" />}
        <span className="grow" />
        {footer}
      </div>
    </div>
  );
}

function CellView({
  column,
  value,
  locale,
  t,
}: {
  column: Column;
  value: Cell;
  locale: string;
  t: ReturnType<typeof useI18n>["t"];
}) {
  if (value !== null && typeof value === "string") {
    switch (column.name) {
      case "verification": {
        const tone = value === "verified" ? "success" : value === "spoofed" ? "danger" : "warning";
        const label =
          value === "verified" || value === "spoofed" || value === "unverified" ? t(`verdict.${value}`) : value;
        return <Badge tone={tone}>{label}</Badge>;
      }
      case "presence": {
        const tone = value === "both" ? "success" : value === "not-in-logs" ? "warning" : "danger";
        const key = PRESENCE_LABELS[value];
        return <Badge tone={tone}>{key ? t(key) : value}</Badge>;
      }
      case "trend": {
        const tone = value === "new" || value === "up" ? "success" : value === "same" ? undefined : "danger";
        const key = TREND_LABELS[value];
        return <Badge tone={tone}>{key ? t(key) : value}</Badge>;
      }
      case "status_class":
        return <Badge tone={statusTone(Number(value[0]))}>{value}</Badge>;
      default:
        break;
    }
  }
  if (column.name === "status" && typeof value === "number") {
    return <Badge tone={statusTone(Math.floor(value / 100))}>{value}</Badge>;
  }
  return <>{formatCell(value, column.kind, locale)}</>;
}

const PRESENCE_LABELS: Record<string, MessageKey> = {
  both: "presence.both",
  "not-in-logs": "presence.not-in-logs",
  "not-in-list": "presence.not-in-list",
};

const TREND_LABELS: Record<string, MessageKey> = {
  new: "trend.new",
  lost: "trend.lost",
  up: "trend.up",
  down: "trend.down",
  same: "trend.same",
};

function statusTone(statusClass: number): "success" | "accent" | "warning" | "danger" {
  if (statusClass === 2) return "success";
  if (statusClass === 3) return "accent";
  if (statusClass === 4) return "warning";
  return "danger";
}
