import { expect, it } from "vitest";
import { exportSelection, type ExplorerExportView } from "./exports";

const view: ExplorerExportView = {
  id: "crawl-a", tabId: "titles", ready: true,
  selection: { request: { tab: "internal", search: "/shop", status_class: 2, indexability: "indexable", issue_id: "title-missing", sort: "address", descending: true, offset: 250, limit: 250 }, columns: ["address", "title"] },
};
it("rejects a loading, replaced or unmounted explorer instead of exporting stale filters", () => {
  expect(exportSelection(null, "crawl-a", "titles")).toBeNull();
  expect(exportSelection({ ...view, ready: false }, "crawl-a", "titles")).toBeNull();
  expect(exportSelection(view, "crawl-b", "titles")).toBeNull();
  expect(exportSelection(view, "crawl-a", "internal")).toBeNull();
});
it("captures an independent selection before a save dialog can change the active view", () => {
  const selection = exportSelection(view, "crawl-a", "titles")!;
  expect(selection.request.search).toBe("/shop");
  expect(selection.columns).toEqual(["address", "title"]);
  selection.request.search = "changed";
  selection.columns.push("status_code");
  expect(view.selection.request.search).toBe("/shop");
  expect(view.selection.columns).toHaveLength(2);
});
it("preserves an exact duplicate group independently while the save dialog is open", () => {
  const scoped = { ...view, selection: { ...view.selection, request: { ...view.selection.request, duplicate_scope: {kind:"description" as const,url:"https://example.com/?q='"} } } };
  const selection=exportSelection(scoped,"crawl-a","titles")!;
  expect(selection.request.duplicate_scope).toEqual(scoped.selection.request.duplicate_scope);
  selection.request.duplicate_scope!.url="changed";
  expect(scoped.selection.request.duplicate_scope.url).toBe("https://example.com/?q='");
});

it("captures the customized visible column order independently of later layout edits",()=>{
  const customized={...view,selection:{...view.selection,columns:["address","word_count","title"]}};
  const exported=exportSelection(customized,"crawl-a","titles")!;
  customized.selection.columns.reverse();
  expect(exported.columns).toEqual(["address","word_count","title"]);
});
