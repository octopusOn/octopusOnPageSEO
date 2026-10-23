// The sample report in each design, drawn by the program's own renderer
// (crates/octopuson-platform-app/src/report/demo.rs writes these files); the font is added once
// for all four, and a report's own colours are set the way the renderer sets them.

import { INK, inkOn, isColor, WHITE } from "../lib/color";
import type { ReportColors, ReportDesign } from "./types";
import accent from "./demo-reports/accent.html?raw";
import clean from "./demo-reports/clean.html?raw";
import elegant from "./demo-reports/elegant.html?raw";
import fonts from "./demo-reports/fonts.css?raw";
import ocean from "./demo-reports/ocean.html?raw";

const PAGES: Record<ReportDesign, string> = { clean, ocean, accent, elegant };

/** The report's root element, whose classes and variables carry the colours. */
const ROOT = /<div class="report design-([a-z]+)" style="--accent:(#[0-9a-f]{6});[^"]*">/;

/** The sample report page in `design`, with `colors` of its own. */
export function demoReportHtml(design: ReportDesign, colors?: Partial<ReportColors>): string {
  const page = (PAGES[design] ?? clean).replace("<style>", `<style>${fonts}`);
  const own = (value: string | null | undefined) => (value && isColor(value) ? value.toLowerCase() : null);
  return page.replace(ROOT, (root, name: string, designAccent: string) => {
    const mine = { accent: own(colors?.accent), accent2: own(colors?.accent2), cover: own(colors?.cover), heading: own(colors?.heading), table: own(colors?.table) };
    if (!Object.values(mine).some(Boolean)) return root;
    const accent = mine.accent ?? designAccent;
    const coverInk = mine.cover ? inkOn(mine.cover) : name === "ocean" ? WHITE : name === "accent" ? inkOn(accent) : INK;
    let classes = `report design-${name}`;
    let style = `--accent:${accent};--on-accent:${inkOn(accent)};--cover-ink:${coverInk}`;
    if (mine.cover) {
      classes += ` custom-cover cover-${coverInk === WHITE ? "dark" : "light"}`;
      style += `;--cover-bg:${mine.cover}`;
    }
    if (mine.table) {
      classes += " custom-table";
      style += `;--table-head:${mine.table};--table-ink:${inkOn(mine.table)}`;
    }
    if (mine.accent2) style += `;--accent-2:${mine.accent2}`;
    if (mine.heading) style += `;--heading:${mine.heading}`;
    return `<div class="${classes}" style="${style}">`;
  });
}
