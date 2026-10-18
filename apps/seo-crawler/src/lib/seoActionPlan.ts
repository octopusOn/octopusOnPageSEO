import type { SeoScore } from "../api/types";
import { issueById, priorityOrder, type Issue } from "../catalog";

export type SeoAction = {
  rule: Issue;
  deduction: SeoScore["deductions"][number];
  rank: number;
};

/** Work from scored evidence, so ignored-only and external-only findings cannot
 * become recommendations. Caps make these contributions attribution, not gains. */
export function seoActionPlan(score?: SeoScore): SeoAction[] {
  if (!score || ![2,3,4,5].includes(score.version) || score.value === null || score.value === undefined) return [];
  const actions: Omit<SeoAction, "rank">[] = [];
  for (const deduction of score.deductions) {
    const rule = issueById.get(deduction.id);
    if (rule && Number.isFinite(deduction.points) && deduction.points > 0 && deduction.count > 0) {
      actions.push({ rule, deduction });
    }
  }
  return actions.sort((a, b) =>
    priorityOrder[a.rule.priority]! - priorityOrder[b.rule.priority]!
    || b.deduction.points - a.deduction.points
    || b.deduction.count - a.deduction.count
    || a.rule.id.localeCompare(b.rule.id, "en")
  ).map((action, index) => ({ ...action, rank: index + 1 }));
}
