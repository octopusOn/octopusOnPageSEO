// Alerts in words and the settings form's own reckoning: each kind's title and text with its
// numbers in the reader's language, where it opens, and the rules grouped as the form shows them.

import type { AlertKind, AlertRow, AlertRules, AlertSettings } from "../../api/types";
import type { PageId, Route } from "../../lib/route";
import { MAX_RECIPIENTS, parseRecipients, validEmail } from "../reports/report";

export const ALERT_KINDS: AlertKind[] = ["rank_drop", "visibility_drop", "gsc_drop", "audit_health", "audit_issues", "ai_drop", "ai_gain", "local_drop", "review", "rival_growth", "links"];

/** The detail's values as the texts show them: numbers in the reader's locale. */
export function alertValues(alert: Pick<AlertRow, "detail">, locale: string): Record<string, string> {
  const format = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  const values: Record<string, string> = {};
  for (const [name, value] of Object.entries(alert.detail)) values[name] = typeof value === "number" ? format.format(value) : String(value);
  return values;
}

/** Where an alert opens: its page and tab, in its project. */
export function alertTarget(alert: Pick<AlertRow, "page" | "params" | "project_id">): Partial<Route> {
  return { page: alert.page, project: alert.project_id, params: alert.params };
}

/** A rule with a threshold (0 turns it off) or one only turned on and off. */
export type RuleField = { key: keyof AlertRules; kind: "number"; unit: "places" | "percent" | "points"; fallback: number } | { key: keyof AlertRules; kind: "toggle" };

/** The rules by the module they watch, in the order the form shows them. */
export const RULE_GROUPS: { page: PageId; rules: RuleField[] }[] = [
  {
    page: "rank",
    rules: [
      { key: "rank_drop", kind: "number", unit: "places", fallback: 5 },
      { key: "top10_exit", kind: "toggle" },
      { key: "visibility_drop", kind: "number", unit: "percent", fallback: 10 },
    ],
  },
  { page: "gsc", rules: [{ key: "gsc_drop", kind: "number", unit: "percent", fallback: 20 }] },
  {
    page: "audit",
    rules: [
      { key: "health_drop", kind: "number", unit: "points", fallback: 5 },
      { key: "audit_issues", kind: "toggle" },
    ],
  },
  { page: "ai", rules: [{ key: "ai_change", kind: "number", unit: "points", fallback: 10 }] },
  {
    page: "local",
    rules: [
      { key: "local_drop", kind: "number", unit: "points", fallback: 10 },
      { key: "reviews", kind: "toggle" },
    ],
  },
  { page: "gap", rules: [{ key: "rivals", kind: "toggle" }] },
  { page: "backlinks", rules: [{ key: "links", kind: "toggle" }] },
];

/** How many rules are on. */
export function rulesOn(rules: AlertRules): number {
  return RULE_GROUPS.flatMap((group) => group.rules).filter((rule) => {
    const value = rules[rule.key];
    return typeof value === "number" ? value > 0 : value;
  }).length;
}

/** The settings with the recipients typed as text; or why they cannot be saved (an i18n key with its values). */
export function settingsToSave(settings: AlertSettings, recipientsText: string): { settings: AlertSettings } | { problem: string; values?: Record<string, string | number> } {
  const recipients = parseRecipients(recipientsText);
  const wrong = recipients.filter((address) => !validEmail(address));
  if (wrong.length > 0) return { problem: "alerts.recipientsWrong", values: { list: wrong.join(", ") } };
  if (recipients.length > MAX_RECIPIENTS) return { problem: "alerts.recipientsMany", values: { n: MAX_RECIPIENTS } };
  if (settings.email !== "off" && recipients.length === 0) return { problem: "alerts.recipientsMissing" };
  return { settings: { ...settings, recipients } };
}
