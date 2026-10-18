import type { ColumnRole } from "../api/types";
import type { Key } from "../i18n";

/** Non-empty trimmed lines of a text field. */
export function lines(text: string): string[] {
  return text
    .split(/\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

/** The i18n key of an import column role. */
export function roleKey(role: ColumnRole | null): Key {
  if (role === null) return "leftOut";
  if (typeof role === "object") return "role.month";
  return `role.${role}` as Key;
}

/** A sparkline path of monthly searches in a box of `width` × `height`. */
export function sparkline(values: number[], width = 64, height = 18): string {
  if (values.length < 2) return "";
  const max = Math.max(...values, 1);
  const step = width / (values.length - 1);
  return values
    .map((value, index) => `${index === 0 ? "M" : "L"}${(index * step).toFixed(1)},${(height - (value / max) * height).toFixed(1)}`)
    .join(" ");
}

/** Difficulty band (as the SERP Checker names them). */
export function difficultyBand(value: number | null): "none" | "easy" | "medium" | "hard" | "extreme" {
  if (value === null) return "none";
  if (value < 30) return "easy";
  if (value < 50) return "medium";
  if (value < 85) return "hard";
  return "extreme";
}
