import type { CrawlConfig, CrawlMeta, PageRequest, TablePage } from "../api/types";

/** Read every failed rendering URL from a finished, immutable crawl. */
export async function prepareRenderingRetry(
  meta: CrawlMeta,
  read: (request: PageRequest) => Promise<TablePage>,
  signal?: AbortSignal,
): Promise<CrawlConfig | null> {
  if (meta.status === "running") throw new Error("The source crawl is still running.");
  const urls: string[] = [];
  let total: number | undefined;
  do {
    signal?.throwIfAborted();
    const page = await read({
      tab: "rendering", issue_id: "render-error", sort: "address", descending: false,
      search: "", status_class: 0, indexability: "", offset: urls.length, limit: 1000,
    });
    signal?.throwIfAborted();
    total ??= page.total;
    const address = page.columns.findIndex((column) => column.name === "address");
    if (page.total !== total || address < 0 || (!page.rows.length && urls.length < total)) {
      throw new Error("Rendering results changed or could not be read completely. Reload the crawl.");
    }
    for (const row of page.rows) {
      const url = row[address];
      if (typeof url !== "string" || !/^https?:\/\//i.test(url)) {
        throw new Error("A rendering result has an invalid URL.");
      }
      urls.push(url);
    }
    if (urls.length > total) throw new Error("Rendering result count is inconsistent.");
  } while (urls.length < total);
  if (!urls.length) return null;
  if (new Set(urls).size !== urls.length) throw new Error("Rendering results contain duplicate URLs.");
  const config = structuredClone(meta.config);
  config.start_urls = urls;
  config.mode = "list";
  config.rendering.mode = "javascript";
  config.limits.max_urls = 0;
  config.limits.max_depth = null;
  return config;
}
