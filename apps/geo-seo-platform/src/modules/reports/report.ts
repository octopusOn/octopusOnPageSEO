// What the Reports page computes on its own: recipients typed as text, the order of a report's
// parts, share-link addresses and logo checks.

import type { ReportLanguage, ReportSpec, ReportWidget } from "../../api/types";

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

/** A new report's starting point in the project's language. */
export function blankReport(language: string, widgets: readonly ReportWidget[]): ReportSpec {
  return {
    name: "",
    language: (LANGUAGES as string[]).includes(language) ? (language as ReportLanguage) : "en",
    days: 28,
    widgets: widgets.filter((widget) => widget !== "backlinks"),
    schedule: "manual",
    recipients: [],
  };
}
