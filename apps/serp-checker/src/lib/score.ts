import type { Band, Breakdown, ScoreComponent } from "../api/types";
import type { Key } from "../i18n";
import { hasKey } from "../i18n";

/** Status tone of a quality score, where higher is better. */
export function tone(score: number | null | undefined): "good" | "warning" | "critical" | "" {
  if (score === null || score === undefined || Number.isNaN(score)) return "";
  if (score >= 70) return "good";
  if (score >= 40) return "warning";
  return "critical";
}

/** Status tone of a difficulty, where higher is harder. */
export function difficultyTone(score: number): "good" | "warning" | "critical" {
  if (score < 30) return "good";
  if (score < 70) return "warning";
  return "critical";
}

export const bandKey = (band: Band) => `band_${band.replace(/-/g, "_")}` as Key;

/** Text keys of a component's reason: its label and, if there is one, the explanation. */
export function reasonKeys(component: ScoreComponent): { label?: Key; help?: Key } {
  const base = component.reason.replace(/\.unknown$/, "");
  const label = `reason.${component.reason}`;
  const help = `reason.${base}.help`;
  return {
    label: hasKey(label) ? label : undefined,
    help: hasKey(help) ? help : undefined,
  };
}

/** Components with known values first, by the points they give or take. */
export function ordered(breakdown: Breakdown) {
  return [...breakdown.components].sort(
    (a, b) =>
      Number(b.normalized !== null && b.normalized !== undefined) -
        Number(a.normalized !== null && a.normalized !== undefined) ||
      Math.abs(b.contribution) - Math.abs(a.contribution),
  );
}

/** The registrable domain of a URL's host, for matching the user's site. */
export function sameSite(domain: string, target: string | null | undefined) {
  if (!target) return false;
  const clean = target.toLowerCase().replace(/^https?:\/\//, "").replace(/^www\./, "").split("/")[0] ?? "";
  return domain === clean || domain.endsWith(`.${clean}`) || clean.endsWith(`.${domain}`);
}
