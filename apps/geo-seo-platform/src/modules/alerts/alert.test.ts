import { describe, expect, it } from "vitest";
import type { AlertRules, AlertSettings } from "../../api/types";
import { MESSAGES } from "../../i18n";
import { ALERT_KINDS, alertTarget, alertValues, RULE_GROUPS, rulesOn, settingsToSave } from "./alert";

const RULES: AlertRules = { rank_drop: 5, top10_exit: true, visibility_drop: 10, gsc_drop: 20, health_drop: 5, audit_issues: true, ai_change: 10, local_drop: 10, reviews: true, rivals: true, links: false };
const SETTINGS: AlertSettings = { enabled: true, rules: RULES, email: "off", recipients: [], lang: "" };

describe("alerts", () => {
  it("writes numbers in the reader's language", () => {
    const detail = { drop: 31.5, before: 1200, clicks: 822, tracker: "Google TR" };
    expect(alertValues({ detail }, "tr-TR")).toEqual({ drop: "31,5", before: "1.200", clicks: "822", tracker: "Google TR" });
    expect(alertValues({ detail }, "en-GB").before).toBe("1,200");
  });

  it("opens where it happened, in its project", () => {
    expect(alertTarget({ page: "audit", params: { tab: "issues" }, project_id: 3 })).toEqual({ page: "audit", project: 3, params: { tab: "issues" } });
  });

  it("counts the rules that are on", () => {
    expect(rulesOn(RULES)).toBe(10);
    expect(rulesOn({ ...RULES, rank_drop: 0, links: true })).toBe(10);
    expect(RULE_GROUPS.flatMap((group) => group.rules.map((rule) => rule.key)).sort()).toEqual(Object.keys(RULES).sort());
  });

  it("checks the recipients before saving", () => {
    expect(settingsToSave(SETTINGS, "")).toEqual({ settings: SETTINGS });
    expect(settingsToSave({ ...SETTINGS, email: "daily" }, "")).toEqual({ problem: "alerts.recipientsMissing" });
    expect(settingsToSave({ ...SETTINGS, email: "daily" }, "Ekip@Example.com, x")).toEqual({ problem: "alerts.recipientsWrong", values: { list: "x" } });
    expect(settingsToSave({ ...SETTINGS, email: "instant" }, "a@b.co\nc@d.co")).toEqual({ settings: { ...SETTINGS, email: "instant", recipients: ["a@b.co", "c@d.co"] } });
  });

  it("has a title and text for every kind and a label for every rule in every language", () => {
    for (const lang of ["tr", "en", "de", "es"]) {
      for (const kind of ALERT_KINDS) {
        expect(MESSAGES[lang]?.[`alerts.title_${kind}`], `${lang} ${kind}`).toBeTruthy();
        expect(MESSAGES[lang]?.[`alerts.text_${kind}`], `${lang} ${kind}`).toBeTruthy();
      }
      for (const key of Object.keys(RULES)) expect(MESSAGES[lang]?.[`alerts.rule_${key}`], `${lang} ${key}`).toBeTruthy();
      for (const unit of ["places", "percent", "points"]) expect(MESSAGES[lang]?.[`alerts.unit_${unit}`]).toBeTruthy();
      for (const email of ["off", "instant", "daily"]) expect(MESSAGES[lang]?.[`alerts.emailHint_${email}`]).toBeTruthy();
    }
  });
});
