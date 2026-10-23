// What the Reports page computes on its own: recipients typed as text, the order of a report's
// parts, share-link addresses and logo checks.

import type { ReportColors, ReportDesign, ReportLanguage, ReportSpec, ReportWidget } from "../../api/types";
import { mix } from "../../lib/color";

export const LANGUAGES: ReportLanguage[] = ["tr", "en", "de", "es"];
export const DAYS = [7, 28, 90] as const;
export const MAX_RECIPIENTS = 5;
/** Largest logo file accepted (its `data:` address grows by a third). */
export const MAX_LOGO_BYTES = 300 * 1024;
export const LOGO_TYPES = ["image/png", "image/jpeg", "image/webp", "image/svg+xml"];

/** Addresses separated by commas, semicolons, spaces or lines; lower case, once each. */
export function parseRecipients(text: string): string[] {
  const seen = new Set<string>();
  for (const part of text.split(/[\s,;]+/)) {
    const address = part.trim().toLowerCase();
    if (address) seen.add(address);
  }
  return [...seen];
}

/** Whether an address looks deliverable (the program checks it again). */
export function validEmail(address: string): boolean {
  return /^[^\s@<>,;"()[\]\\]+@[^\s@<>,;"()[\]\\]+\.[^\s@<>,;"()[\]\\.]+$/.test(address) && address.length <= 254;
}

/** The parts with `widget` moved one place up (-1) or down (+1). */
export function moveWidget(widgets: readonly ReportWidget[], widget: ReportWidget, step: -1 | 1): ReportWidget[] {
  const index = widgets.indexOf(widget);
  const target = index + step;
  if (index < 0 || target < 0 || target >= widgets.length) return [...widgets];
  const next = [...widgets];
  [next[index], next[target]] = [next[target]!, next[index]!];
  return next;
}

/** The parts in the order chosen, then the others not chosen, for the editor's list. */
export function orderedChoices(all: readonly ReportWidget[], chosen: readonly ReportWidget[]): ReportWidget[] {
  return [...chosen.filter((widget) => all.includes(widget)), ...all.filter((widget) => !chosen.includes(widget))];
}

/** A read-only link's address. */
export function shareUrl(base: string | null, token: string, origin: string): string {
  return `${(base ?? origin).replace(/\/+$/, "")}/r/${token}`;
}

/** Why a logo file cannot be used; `null` when it can. */
export function logoProblem(file: { type: string; size: number }): "type" | "size" | null {
  if (!LOGO_TYPES.includes(file.type)) return "type";
  return file.size > MAX_LOGO_BYTES ? "size" : null;
}

/** Parts a new report leaves out: they show data bought on other pages, or need snapshots. */
const ADDED_BY_HAND: readonly ReportWidget[] = ["backlinks", "comparison", "comparison_previous", "organic", "link_history", "gap", "keyword_lists"];

/** A new report's starting point in the project's language. */
export function blankReport(language: string, widgets: readonly ReportWidget[]): ReportSpec {
  return {
    name: "",
    language: (LANGUAGES as string[]).includes(language) ? (language as ReportLanguage) : "en",
    days: 28,
    widgets: widgets.filter((widget) => !ADDED_BY_HAND.includes(widget)),
    schedule: "manual",
    recipients: [],
    design: "clean",
    colors: { ...NO_COLORS },
  };
}

/** Colours a report can have of its own, in the editor's order. */
export const COLOR_KEYS = ["accent", "accent2", "cover", "heading", "table"] as const;
export type ColorKey = (typeof COLOR_KEYS)[number];

/** No colour of its own: the design's everywhere. */
export const NO_COLORS: ReportColors = { accent: null, accent2: null, cover: null, heading: null, table: null };

/** Each design's accent until the agency or the report sets one. */
const DESIGN_ACCENT: Record<ReportDesign, string> = { clean: "#1e6fd9", ocean: "#3354d1", accent: "#1e6fd9", elegant: "#9a7b4f" };

/** The colours a report shows: its own, else the agency's accent, else its design's. */
export function shownColors(design: ReportDesign, colors: ReportColors, brandAccent: string | null): Record<ColorKey, string> {
  const accent = colors.accent ?? brandAccent ?? DESIGN_ACCENT[design];
  const accent2 = colors.accent2 ?? (design === "ocean" ? "#19b5c8" : design === "elegant" ? accent : mix(accent, "#1fb6c9", 0.55));
  const covers: Record<ReportDesign, string> = { clean: "#ffffff", ocean: "#182559", accent: mix(accent, "#070b1a", 0.68), elegant: "#fbfaf7" };
  const tables: Record<ReportDesign, string> = { clean: mix(accent, "#ffffff", 0.09), ocean: "#12224a", accent, elegant: "#ffffff" };
  return { accent, accent2, cover: colors.cover ?? covers[design], heading: colors.heading ?? "#0f1726", table: colors.table ?? tables[design] };
}

/** How many colours are the report's own. */
export function ownColors(colors: ReportColors): number {
  return COLOR_KEYS.filter((key) => colors[key] !== null).length;
}
