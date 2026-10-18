import type { LinkClass, LinkScope, PageRequest } from "../api/types";

export const isLinkEvidenceIssue = (id: string) => ["links-to-broken", "links-to-broken-external", "links-to-redirects"].includes(id);

export const inspectorLinkTabs = {
  internalInlinks: { direction: "incoming", classification: "internal" },
  externalInlinks: { direction: "incoming", classification: "external" },
  internalOutlinks: { direction: "outgoing", classification: "internal" },
  externalOutlinks: { direction: "outgoing", classification: "external" },
} as const;
export type InspectorLinkTab = keyof typeof inspectorLinkTabs;

export function linkPreviewRequest(url: string, direction: LinkScope["direction"], classification: LinkClass): PageRequest {
  return { tab: "links", link_scope: { url, direction }, link_class: classification, limit: 250, offset: 0, search: "", status_class: 0, indexability: "", issue_id: "", sort: direction === "incoming" ? "source" : "destination", descending: false };
}
