import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  RemediationRequest,
  RemediationDetail,
  CrawlConfig,
  CrawlMeta,
  CrawlStorage,
  Job,
  Overview,
  PageRequest,
  TablePage,
  TableExportRequest,
  UrlDetail,
  RedirectTrace,
  ContentMatch,
  DuplicateGroup,
  ComparisonRequest,
  ComparisonSummary,
  FindingComparisonRequest,
  FindingComparisonSummary,
  SitemapSelection,
  SitemapPlan,
  SitemapExport,
  SchemaTestRequest,
  SchemaTestResult,
} from "./types";

export const isNative = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export const isDemo = !isNative;
async function call<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!isNative) {
    const { demo } = await import("./demo");
    return demo(command, args) as Promise<T>;
  }
  return invoke<T>(command, args);
}
export const api = {
  schemaTest: (request:SchemaTestRequest) => call<SchemaTestResult>("schema_test",{request}),
  memory: () =>
    isNative ? call<number | null>("resource_usage") : Promise.resolve(null),
  crawls: () => call<CrawlMeta[]>("list_crawls"),
  storage: () => call<CrawlStorage[]>("crawl_storage"),
  jobs: () => call<Job[]>("list_jobs"),
  start: (config: CrawlConfig) => call<string>("start_crawl", { config }),
  control: (jobId: string, action: "pause" | "resume" | "stop") =>
    call<void>("control_crawl", { jobId, action }),
  remediation: (id:string,request:RemediationRequest) => call<RemediationDetail>("score_remediation",{id,request}),
  overview: (id: string) => call<Overview>("crawl_overview", { id }),
  page: (id: string, request: PageRequest) =>
    call<TablePage>("crawl_page", { id, request }),
  detail: (id: string, url: string) =>
    call<UrlDetail>("url_detail", { id, url }),
  redirects: (id: string, url: string) => call<RedirectTrace>("url_redirects", { id, url }),
  contentMatch: (id: string, url: string) => call<ContentMatch | null>("url_content_match", { id, url }),
  duplicates: (id: string, url: string) => call<DuplicateGroup[]>("url_duplicate_groups", { id, url }),
  remove: (id: string) => call<void>("delete_crawl", { id }),
  sitemapPlan: (id:string,selection:SitemapSelection) => call<SitemapPlan>("sitemap_plan",{id,selection}),
  async exportSitemap(id:string,selection:SitemapSelection,format:"xml"|"zip") {
    if(!isNative) {const { exportDemoSitemap }=await import("./demo");return exportDemoSitemap(id,selection,format);}
    const path=await save({defaultPath:format==="xml"?"sitemap.xml":"sitemaps.zip",filters:[{name:format.toUpperCase(),extensions:[format]}]});
    if(!path)return null;
    return call<SitemapExport>("export_sitemap",{id,selection,path,format});
  },
  comparisonSummary: (baseline:string,current:string,internalOnly:boolean,request?:ComparisonRequest) => call<ComparisonSummary>("comparison_summary",{baseline,current,internalOnly,request}),
  comparisonPage: (baseline:string,current:string,request:ComparisonRequest) => call<TablePage>("comparison_page",{baseline,current,request}),
  comparisonDetail: (baseline:string,current:string,url:string,internalOnly:boolean) => call<TablePage>("comparison_detail",{baseline,current,url,internalOnly}),
  findingComparisonSummary: (baseline:string,current:string,internalOnly:boolean,request?:FindingComparisonRequest)=>call<FindingComparisonSummary>("finding_comparison_summary",{baseline,current,internalOnly,request}),
  findingComparisonPage: (baseline:string,current:string,request:FindingComparisonRequest)=>call<TablePage>("finding_comparison_page",{baseline,current,request}),
  async exportFindingComparison(baseline:string,current:string,request:FindingComparisonRequest,format:"csv"|"xlsx"|"json") {
    if(!isNative){const {exportDemoFindingComparison}=await import("./demo");return exportDemoFindingComparison(baseline,current,request,format);}
    const path=await save({defaultPath:`finding-comparison.${format}`,filters:[{name:format.toUpperCase(),extensions:[format]}]});
    if(!path)return null;
    return call<number>("export_finding_comparison",{baseline,current,request,path});
  },
  async exportComparison(baseline:string,current:string,request:ComparisonRequest,format:"csv"|"xlsx"|"json") {
    if (!isNative) {
      const { exportDemoComparison } = await import("./demo");
      return exportDemoComparison(baseline,current,request,format);
    }
    const path=await save({defaultPath:`crawl-comparison.${format}`,filters:[{name:format.toUpperCase(),extensions:[format]}]});
    if (!path) return null;
    return call<number>("export_comparison",{baseline,current,request,path});
  },
  async export(id: string, tab: string, format: "csv" | "xlsx" | "json", selection?: TableExportRequest) {
    if (!isNative) {
      const { exportDemo } = await import("./demo");
      return exportDemo(id, tab, format, selection);
    }
    const path = await save({
      defaultPath: `${tab}${selection ? "-filtered" : ""}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return null;
    return call<number>("export_crawl", { id, tab, path, selection: selection ?? null });
  },
  async watch(callback: (job: Job) => void, onCloseBlocked?: () => void) {
    if (isNative) {
      const stopProgress = await listen<Job>("spider-progress", (event) => callback(event.payload));
      try {
        const stopClose = await listen("spider-close-blocked", () => onCloseBlocked?.());
        return () => { stopProgress(); stopClose(); };
      } catch (error) {
        stopProgress();
        throw error;
      }
    }
    const handler = (event: Event) =>
      callback((event as CustomEvent<Job>).detail);
    window.addEventListener("spider-demo-progress", handler);
    return () => window.removeEventListener("spider-demo-progress", handler);
  },
};
export function errorText(error: unknown) {
  return error && typeof error === "object" && "message" in error
    ? String(error.message)
    : String(error);
}
