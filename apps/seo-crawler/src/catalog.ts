import catalog from "../../../crates/octopuson-audit/issues.json";
export type Issue = (typeof catalog.issues)[number];
export const issues = catalog.issues;
export const categories = catalog.categories;
export const issueById = new Map(issues.map((issue) => [issue.id, issue]));
export const priorityOrder: Record<string, number> = {
  high: 0,
  medium: 1,
  low: 2,
};
