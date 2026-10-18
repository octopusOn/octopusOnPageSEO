import type { DuplicateKind } from "../api/types";
import type { Key } from "../i18n";

export const duplicateLabels: Record<DuplicateKind, Key> = {
  title: "duplicateTitle", description: "duplicateDescription", h1: "duplicateH1", h2: "duplicateH2", content: "duplicateContent",
};
export function duplicateKindForIssue(id: string): DuplicateKind | undefined {
  return ({ "title-duplicate": "title", "description-duplicate": "description", "h1-duplicate": "h1", "h2-duplicate": "h2", "content-exact-duplicate": "content" } as Record<string, DuplicateKind>)[id];
}
