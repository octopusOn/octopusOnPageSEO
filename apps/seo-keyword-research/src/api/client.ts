import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AddOutcome,
  AppInfo,
  BrowserStatus,
  CloudStatus,
  ClusterPage,
  ClusterQuery,
  ClusterRequest,
  ClusteringRow,
  DiscoverEstimate,
  DiscoverRequest,
  EnrichEstimate,
  EnrichRequest,
  Facets,
  GoogleStatus,
  ImportOutcome,
  ImportPreview,
  Job,
  KeywordPage,
  KeywordQuery,
  KeywordStatus,
  MemberRow,
  ProjectRow,
  ProjectSpec,
  Property,
  ProviderInfo,
  ProviderUsage,
  RunRow,
  SearchConsoleRequest,
  SecretState,
  SerpEstimate,
  SerpRecord,
  SerpRequest,
  Settings,
  SourceMode,
} from "./types";

export type ExportFormat = "csv" | "xlsx" | "json";

export const isNative = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export const isDemo = !isNative;

async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isNative) {
    const { demo } = await import("./demo");
    return demo(command, args) as Promise<T>;
  }
  return invoke<T>(command, args);
}

export const api = {
  info: () => call<AppInfo>("app_info"),
  settings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),

  projects: () => call<ProjectRow[]>("list_projects"),
  createProject: (spec: ProjectSpec) => call<ProjectRow>("create_project", { spec }),
  updateProject: (id: number, spec: ProjectSpec) => call<ProjectRow>("update_project", { id, spec }),
  deleteProject: (id: number) => call<void>("delete_project", { id }),
  runs: (projectId: number | null) => call<RunRow[]>("list_runs", { projectId }),

  addKeywords: (projectId: number, keywords: string[]) =>
    call<AddOutcome>("add_keywords", { projectId, keywords }),
  previewImport: (path: string) => call<ImportPreview>("preview_import", { path }),
  importFile: (projectId: number, path: string) => call<ImportOutcome>("import_file", { projectId, path }),
  keywords: (projectId: number, query: KeywordQuery) =>
    call<KeywordPage>("list_keywords", { projectId, query }),
  setStatus: (projectId: number, ids: number[], status: KeywordStatus) =>
    call<number>("set_status", { projectId, ids, status }),
  tag: (projectId: number, ids: number[], add: string[], remove: string[]) =>
    call<void>("tag_keywords", { projectId, ids, add, remove }),
  facets: (projectId: number) => call<Facets>("keyword_facets", { projectId }),
  keywordSerp: (keywordId: number, engine: string, device = "desktop") =>
    call<SerpRecord | null>("keyword_serp", { keywordId, engine, device }),

  estimateDiscovery: (projectId: number, request: DiscoverRequest) =>
    call<DiscoverEstimate>("estimate_discovery", { projectId, request }),
  discover: (projectId: number, request: DiscoverRequest) =>
    call<string>("start_discovery", { projectId, request }),
  estimateEnrichment: (projectId: number, request: EnrichRequest) =>
    call<EnrichEstimate>("estimate_enrichment", { projectId, request }),
  enrich: (projectId: number, request: EnrichRequest) =>
    call<string>("start_enrichment", { projectId, request }),
  estimateSerps: (projectId: number, request: SerpRequest) =>
    call<SerpEstimate>("estimate_serps", { projectId, request }),
  fetchSerps: (projectId: number, request: SerpRequest) =>
    call<string>("start_serps", { projectId, request }),

  cluster: (projectId: number, request: ClusterRequest) =>
    call<string>("start_cluster", { projectId, request }),
  clusterings: (projectId: number) => call<ClusteringRow[]>("list_clusterings", { projectId }),
  clusters: (clusteringId: number, query: ClusterQuery) =>
    call<ClusterPage>("list_clusters", { clusteringId, query }),
  members: (clusteringId: number, number: number) =>
    call<MemberRow[]>("cluster_members", { clusteringId, number }),
  place: (projectId: number, keywordIds: number[], pillarId: number) =>
    call<ClusteringRow>("place_keywords", { projectId, keywordIds, pillarId }),
  unplace: (projectId: number, keywordIds: number[]) =>
    call<void>("unplace_keywords", { projectId, keywordIds }),
  remap: (projectId: number) => call<ClusteringRow>("remap", { projectId }),

  providers: () => call<ProviderInfo[]>("list_providers"),
  setKey: (provider: string, name: string, value: string) =>
    call<void>("set_key", { provider, name, value }),
  removeKey: (provider: string, name: string) => call<void>("remove_key", { provider, name }),
  setBudget: (provider: string, budgetUsd: number | null) =>
    call<void>("set_budget", { provider, budgetUsd }),
  setEnabled: (provider: string, enabled: boolean) => call<void>("set_enabled", { provider, enabled }),
  usage: () => call<ProviderUsage[]>("usage"),
  secretState: (owner: string, name: string) => call<SecretState>("secret_state", { owner, name }),
  browserStatus: () => call<BrowserStatus>("browser_status"),
  resetBrowserProfile: () => call<void>("reset_browser_profile"),

  cloudStatus: () => call<CloudStatus>("cloud_status"),
  accountLogin: () => call<string>("start_account_login"),
  accountLogout: () => call<void>("account_logout"),
  setSourceMode: (provider: string, mode: SourceMode) => call<void>("set_source_mode", { provider, mode }),
  openAccountPage: (page: "credits" | "devices" | "usage") => call<void>("open_account_page", { page }),

  googleStatus: () => call<GoogleStatus>("google_status"),
  setGoogleClient: (clientId: string, clientSecret: string) =>
    call<void>("set_google_client", { clientId, clientSecret }),
  setAdsAccount: (customerId: string | null, loginCustomerId: string | null) =>
    call<void>("set_ads_account", { customerId, loginCustomerId }),
  googleLogin: (scopes: { search_console: boolean; ads: boolean }) =>
    call<string>("start_google_login", { scopes }),
  googleLogout: () => call<void>("google_logout"),
  properties: () => call<Property[]>("search_console_properties"),
  importSearchConsole: (projectId: number, request: SearchConsoleRequest) =>
    call<string>("start_search_console", { projectId, request }),

  jobs: () => call<Job[]>("list_jobs"),
  cancel: (id: string) => call<void>("cancel_job", { id }),
  clearJobs: () => call<void>("clear_jobs"),

  /** Asks where to save, then writes the file; `null` when the person cancelled. */
  async exportKeywords(projectId: number, query: KeywordQuery, name: string, format: ExportFormat) {
    const path = await exportPath(name, format);
    return path === null ? null : call<number>("export_keywords", { projectId, query, path });
  },
  async exportClusters(projectId: number, clusteringId: number | null, name: string, format: ExportFormat) {
    const path = await exportPath(name, format);
    return path === null ? null : call<number>("export_clusters", { projectId, clusteringId, path });
  },
  /** Asks for a keyword file to import; `null` when the person cancelled. */
  async pickImport(): Promise<string | null> {
    if (!isNative) return "ahrefs-keywords.csv";
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Keywords", extensions: ["csv", "tsv", "txt", "xlsx", "zip"] }],
    });
    return typeof picked === "string" ? picked : null;
  },

  /** Calls `callback` with every job update; returns the function that stops listening. */
  async watch(callback: (job: Job) => void, onCloseBlocked?: () => void) {
    if (isNative) {
      const stopProgress = await listen<Job>("keywords-progress", (event) => callback(event.payload));
      try {
        const stopClose = await listen("keywords-close-blocked", () => onCloseBlocked?.());
        return () => {
          stopProgress();
          stopClose();
        };
      } catch (error) {
        stopProgress();
        throw error;
      }
    }
    const handler = (event: Event) => callback((event as CustomEvent<Job>).detail);
    window.addEventListener("keywords-demo-progress", handler);
    return () => window.removeEventListener("keywords-demo-progress", handler);
  },
};

/** A file name without characters that file systems reject. */
export function fileName(text: string) {
  return text.trim().replace(/[\\/:*?"<>|]+/g, " ").trim().replace(/\s+/g, "-").slice(0, 80) || "export";
}

async function exportPath(name: string, format: ExportFormat): Promise<string | null> {
  if (!isNative) return `${fileName(name)}.${format}`;
  return save({
    defaultPath: `${fileName(name)}.${format}`,
    filters: [{ name: format.toUpperCase(), extensions: [format] }],
  });
}

export function errorText(error: unknown) {
  return error && typeof error === "object" && "message" in error
    ? String(error.message)
    : String(error);
}
