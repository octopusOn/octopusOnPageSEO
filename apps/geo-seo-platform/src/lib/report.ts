// Filtering and summing report rows, and exporting them as CSV.

import { currentMode } from "../api/transport";
import type { RankedKeyword } from "../api/types";

export type PositionBand = "all" | "top3" | "top10" | "top20" | "beyond";

export interface KeywordFilter {
  query: string;
  position: PositionBand;
  intent: string;
  minVolume: number;
}

export const NO_FILTER: KeywordFilter = { query: "", position: "all", intent: "", minVolume: 0 };

function inBand(position: number | null, band: PositionBand): boolean {
  if (band === "all") return true;
  if (position === null) return false;
  switch (band) {
    case "top3":
      return position <= 3;
    case "top10":
      return position <= 10;
    case "top20":
      return position <= 20;
    case "beyond":
      return position > 20;
  }
}

/**
 * Text folded for searching: any case, without accents, and the Turkish dotless and dotted i
 * both as `i` ("AYAKKABI" and "ayakkabı" and "ayakkabi" all match).
 */
export function fold(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/ı/g, "i");
}

/** The keywords a filter keeps; the query matches the keyword or its address, folded. */
export function filterKeywords(rows: readonly RankedKeyword[], filter: KeywordFilter): RankedKeyword[] {
  const query = fold(filter.query.trim());
  return rows.filter(
    (row) =>
      (!query || fold(row.keyword).includes(query) || fold(row.url ?? "").includes(query)) &&
      inBand(row.position, filter.position) &&
      (!filter.intent || row.intent === filter.intent) &&
      (row.volume ?? 0) >= filter.minVolume,
  );
}

/** How many keywords show each result-page element, most common first. */
export function featureCounts(rows: readonly RankedKeyword[]): [string, number][] {
  const counts = new Map<string, number>();
  for (const row of rows) for (const feature of new Set(row.serp_features)) counts.set(feature, (counts.get(feature) ?? 0) + 1);
  return [...counts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
}

function cell(value: string | number | null | undefined): string {
  const text = value === null || value === undefined ? "" : String(value);
  // Cells starting like a formula are quoted so spreadsheets do not run them.
  const safe = /^[=+\-@\t\r]/.test(text) ? `'${text}` : text;
  return /[",\n\r;]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}

/** CSV text with a header row, opened correctly by Excel (byte order mark). */
export function toCsv(header: string[], rows: (string | number | null | undefined)[][]): string {
  return `﻿${[header, ...rows].map((row) => row.map(cell).join(",")).join("\r\n")}\r\n`;
}

/** Offers text as a file to save. */
export function download(name: string, text: string, type = "text/csv;charset=utf-8"): void {
  if (currentMode() === "desktop") {
    void saveOnDesktop(name, bytesToBase64(new TextEncoder().encode(text)));
    return;
  }
  saveBlob(name, new Blob([text], { type }));
}

/** Saves a file the program sent as base64. */
export function downloadBase64(name: string, data: string, type: string): void {
  if (currentMode() === "desktop") {
    void saveOnDesktop(name, data);
    return;
  }
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  saveBlob(name, new Blob([bytes], { type }));
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let index = 0; index < bytes.length; index += 0x8000) binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  return btoa(binary);
}

/** The desktop window cannot download: the person picks where the file goes. */
async function saveOnDesktop(name: string, data: string): Promise<void> {
  const [{ save }, { invoke }] = await Promise.all([import("@tauri-apps/plugin-dialog"), import("@tauri-apps/api/core")]);
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  const path = await save({ defaultPath: name, filters: extension ? [{ name: extension.toUpperCase(), extensions: [extension] }] : [] });
  if (path) await invoke("save_file", { path, data });
}

function saveBlob(name: string, blob: Blob): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
