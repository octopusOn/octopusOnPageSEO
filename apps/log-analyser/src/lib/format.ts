// Locale-aware formatting of numbers, dates and result cells.

import type { Cell, ColumnKind } from "../api/types";

export function formatNumber(value: number, locale: string, digits = 0): string {
  return new Intl.NumberFormat(locale, {
    maximumFractionDigits: digits,
    minimumFractionDigits: 0,
  }).format(value);
}

export function formatPercent(ratio: number, locale: string): string {
  return new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 1 }).format(ratio);
}

export function formatBytes(bytes: number, locale: string): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${formatNumber(value, locale, unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function formatDateTime(iso: string | null | undefined, locale: string): string {
  if (!iso) return "–";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

export function formatDuration(ms: number, locale: string): string {
  if (ms < 1000) return `${formatNumber(ms, locale)} ms`;
  const seconds = ms / 1000;
  if (seconds < 120) return `${formatNumber(seconds, locale, 1)} s`;
  return `${formatNumber(seconds / 60, locale, 1)} min`;
}

/** Display text of a result cell. */
export function formatCell(cell: Cell, kind: ColumnKind, locale: string): string {
  if (cell === null) return "–";
  switch (kind) {
    case "integer":
      return typeof cell === "number" ? formatNumber(cell, locale) : String(cell);
    case "float":
      return typeof cell === "number" ? formatNumber(cell, locale, 2) : String(cell);
    case "timestamp":
      return typeof cell === "string" ? formatDateTime(cell, locale) : String(cell);
    case "boolean":
      return cell ? "✓" : "✗";
    default:
      return String(cell);
  }
}
