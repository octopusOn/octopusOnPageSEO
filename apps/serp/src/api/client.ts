import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  AnalysisSummary,
  AnalysisView,
  AnalyzeRequest,
  AppInfo,
  CloudStatus,
  DomainFacts,
  Engine,
  Job,
  KeywordAnalysis,
  KeywordRow,
  ParsedPage,
  ParserInfo,
  ProfileOverride,
  Project,
  ProjectSpec,
  ProviderEstimate,
  ProviderInfo,
  ProviderSpec,
  ProviderUsage,
  RankReport,
  SearchRequest,
  SerpSnapshot,
  Settings,
  SnapshotSummary,
  SourceMode,
  StoredPage,
  TrackEstimate,
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
  cloudStatus: () => call<CloudStatus>("cloud_status"),
  accountLogin: () => call<string>("start_account_login"),
  accountLogout: () => call<void>("account_logout"),
  setSourceMode: (provider: string, mode: SourceMode) => call<void>("set_source_mode", { provider, mode }),
  openAccountPage: (page: "credits" | "devices" | "usage") => call<void>("open_account_page", { page }),
  settings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),
  providers: () => call<ProviderInfo[]>("list_providers"),
  setSecret: (provider: string, name: string, value: string) =>
    call<void>("set_secret", { provider, name, value }),
  removeSecret: (provider: string, name: string) =>
    call<void>("remove_secret", { provider, name }),
  estimate: (request: SearchRequest) => call<ProviderEstimate[]>("estimate_search", { request }),
  analyze: (request: AnalyzeRequest) => call<string>("start_analysis", { request }),
  search: (request: SearchRequest) => call<string>("start_search", { request }),
  track: (projectId: number) => call<string>("start_track", { projectId }),
  jobs: () => call<Job[]>("list_jobs"),
  cancel: (id: string) => call<void>("cancel_job", { id }),
  clearJobs: () => call<void>("clear_jobs"),
  analyses: (keyword?: string) => call<AnalysisSummary[]>("list_analyses", { keyword: keyword ?? null }),
  analysis: (id: number) => call<AnalysisView>("get_analysis", { id }),
  rescore: (id: number) => call<KeywordAnalysis>("rescore_analysis", { id }),
  snapshots: (filter: { keyword?: string; engine?: Engine; limit?: number } = {}) =>
    call<SnapshotSummary[]>("list_snapshots", { filter }),
  snapshot: (id: number) => call<SerpSnapshot>("get_snapshot", { id }),
  deleteSnapshot: (id: number) => call<void>("delete_snapshot", { id }),
  usage: () => call<ProviderUsage[]>("usage"),
  domains: (hosts: string[]) => call<DomainFacts[]>("domain_facts", { hosts }),
  updateTranco: (accepted: boolean) => call<number>("update_tranco", { accepted }),
  closeBrowser: () => call<void>("close_browser"),
  resetBrowser: () => call<void>("reset_browser_profile"),
  projects: () => call<Project[]>("list_projects"),
  createProject: (spec: ProjectSpec) => call<Project>("create_project", { spec }),
  updateProject: (project: Project) => call<void>("update_project", { project }),
  deleteProject: (id: number) => call<void>("delete_project", { id }),
  addKeywords: (projectId: number, keywords: string[], tags: string[]) =>
    call<number>("add_keywords", { projectId, keywords, tags }),
  keywords: (projectId: number) => call<KeywordRow[]>("list_keywords", { projectId }),
  removeKeyword: (projectId: number, keywordId: number) =>
    call<void>("remove_keyword", { projectId, keywordId }),
  setCompetitors: (projectId: number, domains: string[]) =>
    call<void>("set_competitors", { projectId, domains }),
  estimateTrack: (projectId: number) => call<TrackEstimate>("estimate_track", { projectId }),
  rankReport: (projectId: number) => call<RankReport>("rank_report", { projectId }),
  storedPages: (snapshotId: number) => call<StoredPage[]>("stored_pages", { snapshotId }),
  testParser: (snapshotId: number, patch?: ProfileOverride, page = 1) =>
    call<ParsedPage>("test_parser", { snapshotId, page, patch: patch ?? null }),
  reparse: (snapshotId: number) => call<SerpSnapshot>("reparse_snapshot", { snapshotId }),
  parsers: () => call<ParserInfo[]>("list_parsers"),
  saveParserOverride: (patch: ProfileOverride) => call<void>("save_parser_override", { patch }),
  removeParserOverride: (engine: Engine) => call<void>("remove_parser_override", { engine }),
  customSpecs: () => call<ProviderSpec[]>("custom_specs"),
  saveSpec: (spec: ProviderSpec) => call<void>("save_spec", { spec }),
  removeSpec: (id: string) => call<void>("remove_spec", { id }),
  importSpecs: (json: string) => call<string[]>("import_specs", { json }),
  exportSpecs: (ids: string[]) => call<string>("export_specs", { ids }),
  testSpec: (spec: ProviderSpec, engine: Engine, body: string, url?: string) =>
    call<ParsedPage>("test_spec", { spec, engine, body, url: url ?? null }),
  /** Asks where to save, then writes the file; `null` when the person cancelled. */
  async exportRankReport(projectId: number, name: string, format: ExportFormat) {
    const path = await exportPath(name, format);
    return path === null ? null : call<number>("export_rank_report", { projectId, path });
  },
  async exportAnalysis(id: number, name: string, format: ExportFormat) {
    const path = await exportPath(name, format);
    return path === null ? null : call<number>("export_analysis", { id, path });
  },

  /** Calls `callback` with every job update; returns the function that stops listening. */
  async watch(callback: (job: Job) => void, onCloseBlocked?: () => void) {
    if (isNative) {
      const stopProgress = await listen<Job>("serp-progress", (event) => callback(event.payload));
      try {
        const stopClose = await listen("serp-close-blocked", () => onCloseBlocked?.());
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
    window.addEventListener("serp-demo-progress", handler);
    return () => window.removeEventListener("serp-demo-progress", handler);
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
