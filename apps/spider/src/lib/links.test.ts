import { expect, it } from "vitest";
import { inspectorLinkTabs, isLinkEvidenceIssue, linkPreviewRequest } from "./links";

it("keeps incoming and outgoing internal/external previews separate, with exact URL identity", () => {
  const url = "https://example.com/?q='<script>";
  for (const tab of Object.values(inspectorLinkTabs)) {
    const request = linkPreviewRequest(url, tab.direction, tab.classification);
    expect(request.link_scope).toEqual({ url, direction: tab.direction });
    expect(request.link_class).toBe(tab.classification);
    expect(request.sort).toBe(tab.direction === "incoming" ? "source" : "destination");
    expect(request.limit).toBe(250);
    expect(request.issue_id).toBe("");
  }
  expect(new Set(Object.values(inspectorLinkTabs).map(tab => `${tab.direction}/${tab.classification}`)).size).toBe(4);
});

it("routes only relation findings to their link evidence", () => {
  for (const id of ["links-to-broken", "links-to-broken-external", "links-to-redirects"]) expect(isLinkEvidenceIssue(id)).toBe(true);
  for (const id of ["internal-client-error", "image-broken", "title-missing", ""]) expect(isLinkEvidenceIssue(id)).toBe(false);
});
