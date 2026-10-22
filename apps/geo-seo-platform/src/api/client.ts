// The platform's commands, typed.

import { call } from "./transport";
import type {
  AccessToken,
  AccountStatus,
  AiEngine,
  AiEstimate,
  AiReport,
  AiRun,
  AiSchedule,
  AiSetup,
  AnswerDetail,
  GoogleLogin,
  GoogleProperties,
  GoogleStatus,
  GscReport,
  GscSchedule,
  GscSetup,
  MadeAccessToken,
  QueryPage,
  Branding,
  ReportDelivery,
  ReportRow,
  ReportSpec,
  ReportsSetup,
  SavedFile,
  Rival,
  AppInfo,
  AuditReport,
  AuditSchedule,
  AuditSetup,
  IssueDetail,
  Estimate,
  Fetched,
  Job,
  KeywordData,
  KeywordListRow,
  ListKeyword,
  Opened,
  ProjectRow,
  ProjectSpec,
  OverviewReport,
  ProviderInfo,
  KeywordDetail,
  NewKeyword,
  RankEstimate,
  RankReport,
  RankRun,
  RankSetup,
  Schedule,
  SearchEngine,
  Device,
  Settings,
  SourceMode,
  SyncAnswer,
  SyncStatus,
  Topic,
  UsageRow,
} from "./types";

export interface OverviewAsk {
  project_id: number;
  domain?: string;
  report?: OverviewReport;
  /** Rows of a table report (10–1000). */
  limit?: number;
  force?: boolean;
}

/** Modules whose reports are paid for: their estimate, kept-answer and fetch commands. */
export type PaidModule = "overview" | "keywords" | "backlinks" | "gap" | "ai_mentions";

const PAID: Record<PaidModule, [string, string, string]> = {
  overview: ["overview_estimate", "get_overview", "fetch_overview"],
  keywords: ["keywords_estimate", "get_keywords", "fetch_keywords"],
  backlinks: ["backlinks_estimate", "get_backlinks", "fetch_backlinks"],
  gap: ["gap_estimate", "get_gap", "fetch_gap"],
  ai_mentions: ["ai_mentions_estimate", "get_ai_mentions", "fetch_ai_mentions"],
};

/** A paid report's three commands. */
export const paid = {
  estimate: (module: PaidModule, ask: Record<string, unknown>) => call<Estimate>(PAID[module][0], ask),
  kept: <T>(module: PaidModule, ask: Record<string, unknown>) => call<Fetched<T> | null>(PAID[module][1], ask),
  fetch: <T>(module: PaidModule, ask: Record<string, unknown>) => call<Fetched<T>>(PAID[module][2], ask),
};

export const api = {
  info: () => call<AppInfo>("app_info"),
  settings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),
  syncStatus: () => call<SyncStatus>("sync_status"),
  setSync: (enabled: boolean) => call<SyncStatus>("set_sync", { enabled }),
  syncNow: () => call<SyncStatus>("sync_now"),
  syncChoose: (choice: SyncAnswer) => call<SyncStatus>("sync_choose", { choice }),
  reportProblem: (message: string, page: string, error: string) => call<null>("report_problem", { message, page, error }),
  accessTokens: () => call<AccessToken[]>("list_access_tokens"),
  createAccessToken: (name: string) => call<MadeAccessToken>("create_access_token", { name }),
  revokeAccessToken: (id: string) => call<null>("revoke_access_token", { id }),
  openUrl: (url: string) => call<Opened>("open_url", { url }),

  projects: () => call<ProjectRow[]>("list_projects"),
  project: (id: number) => call<ProjectRow>("get_project", { id }),
  createProject: (spec: ProjectSpec) => call<ProjectRow>("create_project", { spec }),
  updateProject: (id: number, spec: ProjectSpec) => call<ProjectRow>("update_project", { id, spec }),
  deleteProject: (id: number) => call<null>("delete_project", { id }),

  providers: () => call<ProviderInfo[]>("list_providers"),
  setKey: (provider: string, name: string, value: string) => call<null>("set_key", { provider, name, value }),
  removeKey: (provider: string, name: string) => call<null>("remove_key", { provider, name }),
  setMode: (provider: string, mode: SourceMode) => call<null>("set_source_mode", { provider, mode }),
  setBudget: (provider: string, usd: number | null) => call<null>("set_budget", { provider, usd }),
  usage: () => call<UsageRow[]>("usage"),

  account: () => call<AccountStatus>("account_status"),
  connect: () => call<string>("start_account_login"),
  disconnect: () => call<null>("account_logout"),
  accountPage: (page: "credits" | "usage" | "devices") => call<Opened>("account_page", { page }),

  jobs: () => call<Job[]>("list_jobs"),
  cancelJob: (id: string) => call<boolean>("cancel_job", { id }),
  clearJobs: () => call<null>("clear_jobs"),

  keywordAutocomplete: (projectId: number, seed: string, engine: "google" | "yandex") =>
    call<string[]>("keyword_autocomplete", { project_id: projectId, seed, engine }),
  keywordLists: (projectId: number) => call<KeywordListRow[]>("keyword_lists", { project_id: projectId }),
  createKeywordList: (projectId: number, name: string) => call<KeywordListRow>("create_keyword_list", { project_id: projectId, name }),
  renameKeywordList: (listId: number, name: string) => call<KeywordListRow>("rename_keyword_list", { list_id: listId, name }),
  deleteKeywordList: (listId: number) => call<null>("delete_keyword_list", { list_id: listId }),
  listKeywords: (listId: number) => call<ListKeyword[]>("list_keywords", { list_id: listId }),
  addToKeywordList: (listId: number, keywords: KeywordData[]) => call<number>("add_to_keyword_list", { list_id: listId, keywords }),
  removeFromKeywordList: (listId: number, keywords: string[]) => call<number>("remove_from_keyword_list", { list_id: listId, keywords }),
  clusterKeywordList: (listId: number, threshold?: number) => call<Topic[]>("cluster_keyword_list", { list_id: listId, ...(threshold ? { threshold } : {}) }),

  rankSetup: (projectId: number) => call<RankSetup>("rank_setup", { project_id: projectId }),
  saveRankSettings: (projectId: number, settings: { schedule: Schedule; depth: number; ai_overview: boolean }) =>
    call<RankSetup>("save_rank_settings", { project_id: projectId, ...settings }),
  addRankTracker: (projectId: number, tracker: { engine: SearchEngine; device: Device; country?: string; language?: string; location?: string }) =>
    call<RankSetup>("add_rank_tracker", { project_id: projectId, ...tracker }),
  removeRankTracker: (projectId: number, trackerId: number) => call<RankSetup>("remove_rank_tracker", { project_id: projectId, tracker_id: trackerId }),
  trackKeywords: (projectId: number, keywords: NewKeyword[], tags: string[] = []) =>
    call<{ added: number; total: number }>("track_keywords", { project_id: projectId, keywords, tags }),
  untrackKeywords: (projectId: number, keywordIds: number[]) => call<number>("untrack_keywords", { project_id: projectId, keyword_ids: keywordIds }),
  setKeywordTags: (projectId: number, keywordIds: number[], tags: string[]) => call<null>("set_keyword_tags", { project_id: projectId, keyword_ids: keywordIds, tags }),
  rankEstimate: (projectId: number) => call<RankEstimate>("rank_estimate", { project_id: projectId }),
  startRankCheck: (projectId: number) => call<string>("start_rank_check", { project_id: projectId }),
  rankRuns: (projectId: number) => call<RankRun[]>("rank_runs", { project_id: projectId }),
  rankReport: (projectId: number, trackerId: number | null, days: number) =>
    call<RankReport>("rank_report", { project_id: projectId, days, ...(trackerId === null ? {} : { tracker_id: trackerId }) }),
  rankKeyword: (projectId: number, keywordId: number, trackerId: number) =>
    call<KeywordDetail>("rank_keyword", { project_id: projectId, keyword_id: keywordId, tracker_id: trackerId }),
  rankVolumesEstimate: (projectId: number) => call<Estimate>("rank_volumes_estimate", { project_id: projectId }),
  fetchRankVolumes: (projectId: number) => call<{ updated: number; spent_usd: number }>("fetch_rank_volumes", { project_id: projectId }),

  auditSetup: (projectId: number) => call<AuditSetup>("audit_setup", { project_id: projectId }),
  saveAuditSettings: (projectId: number, schedule: AuditSchedule, maxPages: number) =>
    call<AuditSetup>("save_audit_settings", { project_id: projectId, schedule, max_pages: maxPages }),
  startAudit: (projectId: number) => call<string>("start_audit", { project_id: projectId }),
  auditReport: (projectId: number, auditId: number | null, lang: string) =>
    call<AuditReport | null>("audit_report", { project_id: projectId, lang, ...(auditId === null ? {} : { audit_id: auditId }) }),
  auditIssue: (projectId: number, auditId: number, issueId: string, offset: number, lang: string) =>
    call<IssueDetail>("audit_issue", { project_id: projectId, audit_id: auditId, issue_id: issueId, offset, lang }),
  deleteAudit: (projectId: number, auditId: number) => call<null>("delete_audit", { project_id: projectId, audit_id: auditId }),

  aiSetup: (projectId: number) => call<AiSetup>("ai_setup", { project_id: projectId }),
  saveAiSettings: (projectId: number, settings: { schedule: AiSchedule; engines: AiEngine[]; models: Partial<Record<AiEngine, string>>; rivals: Rival[] }) =>
    call<AiSetup>("save_ai_settings", { project_id: projectId, ...settings }),
  addAiPrompts: (projectId: number, prompts: string[], tags: string[] = []) => call<{ added: number; total: number }>("add_ai_prompts", { project_id: projectId, prompts, tags }),
  removeAiPrompts: (projectId: number, promptIds: number[]) => call<number>("remove_ai_prompts", { project_id: projectId, prompt_ids: promptIds }),
  suggestAiPrompts: (projectId: number) => call<string[]>("suggest_ai_prompts", { project_id: projectId }),
  aiEstimate: (projectId: number) => call<AiEstimate>("ai_estimate", { project_id: projectId }),
  startAiRun: (projectId: number) => call<string>("start_ai_run", { project_id: projectId }),
  aiRuns: (projectId: number) => call<AiRun[]>("ai_runs", { project_id: projectId }),
  aiReport: (projectId: number, days: number) => call<AiReport>("ai_report", { project_id: projectId, days }),
  aiAnswer: (projectId: number, promptId: number, engine: AiEngine) => call<AnswerDetail>("ai_answer", { project_id: projectId, prompt_id: promptId, engine }),

  googleStatus: () => call<GoogleStatus>("google_status"),
  setGoogleClient: (clientId: string, clientSecret: string) => call<GoogleStatus>("set_google_client", { client_id: clientId, client_secret: clientSecret }),
  startGoogleLogin: (analytics: boolean) => call<GoogleLogin>("start_google_login", { analytics }),
  googleLogout: () => call<GoogleStatus>("google_logout"),
  googleProperties: (projectId: number) => call<GoogleProperties>("google_properties", { project_id: projectId }),
  gscSetup: (projectId: number) => call<GscSetup>("gsc_setup", { project_id: projectId }),
  saveGscSettings: (projectId: number, settings: { property: string | null; ga4_property: string | null; ga4_name: string | null; days: number; schedule: GscSchedule }) =>
    call<GscSetup>("save_gsc_settings", { project_id: projectId, ...settings }),
  startGscSync: (projectId: number) => call<string>("start_gsc_sync", { project_id: projectId }),
  gscReport: (projectId: number) => call<GscReport | null>("gsc_report", { project_id: projectId }),
  gscQuery: (projectId: number, query: string) => call<QueryPage[]>("gsc_query", { project_id: projectId, query }),

  reportsSetup: (projectId: number) => call<ReportsSetup>("reports_setup", { project_id: projectId }),
  saveReportBranding: (branding: Branding) => call<Branding>("save_report_branding", { branding }),
  saveReport: (projectId: number, reportId: number | null, spec: ReportSpec) =>
    call<ReportRow>("save_report", { project_id: projectId, ...(reportId === null ? {} : { report_id: reportId }), ...spec }),
  deleteReport: (projectId: number, reportId: number) => call<null>("delete_report", { project_id: projectId, report_id: reportId }),
  reportHtml: (projectId: number, reportId: number) => call<{ name: string; html: string }>("report_html", { project_id: projectId, report_id: reportId }),
  reportPdf: (projectId: number, reportId: number) => call<SavedFile>("report_pdf", { project_id: projectId, report_id: reportId }),
  reportXlsx: (projectId: number, reportId: number) => call<SavedFile>("report_xlsx", { project_id: projectId, report_id: reportId }),
  shareReport: (projectId: number, reportId: number) => call<ReportRow>("share_report", { project_id: projectId, report_id: reportId }),
  unshareReport: (projectId: number, reportId: number) => call<ReportRow>("unshare_report", { project_id: projectId, report_id: reportId }),
  reportDeliveries: (projectId: number, reportId: number) => call<ReportDelivery[]>("report_deliveries", { project_id: projectId, report_id: reportId }),
  sendReport: (projectId: number, reportId: number) => call<string>("send_report", { project_id: projectId, report_id: reportId }),
};
