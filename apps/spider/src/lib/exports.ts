import type { TableExportRequest } from "../api/types";

export type ExplorerExportView = {
  id: string;
  tabId: string;
  selection: TableExportRequest;
  ready: boolean;
};

/** Export only a settled view belonging to the currently selected crawl and tab. */
export function exportSelection(view: ExplorerExportView | null, id: string, tabId: string): TableExportRequest | null {
  if (!view?.ready || view.id !== id || view.tabId !== tabId || !view.selection.columns.length) return null;
  return structuredClone(view.selection);
}
