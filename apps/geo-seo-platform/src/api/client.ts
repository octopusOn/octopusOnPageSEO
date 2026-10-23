// The platform's commands, typed.

import { call } from "./transport";
import type {
  AccessToken,
  AdCopy,
  AccountStatus,
  AiEngine,
  AiEstimate,
  AiReport,
  AlertList,
  AlertSettings,
  AlertSetup,
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
  GscCompare,
  GscExport,
  LinksReport,
  HreflangReport,
  PagesAsk,
  PagesView,
  CrawledPageDetail,
  FindingsFile,
  VitalsSetup,
  VitalsReport,
  MadeAccessToken,
  QueryPage,
  Branding,
  ReportDelivery,
  ReportRow,
  ReportSpec,
  ReportsSetup,
  SavedFile,
  SerpGroups,
  Rival,
  AppInfo,
  AuditReport,
  AuditSchedule,
  AuditSetup,
  CategoryName,
  ClusterEstimate,
  DisavowFile,
  IssueDetail,
  Estimate,
  Fetched,
  Job,
  KeywordData,
  KeywordListRow,
  LinkDecision,
  LinkDecisionKind,
  LinkProspect,
  ListKeyword,
  MetricPoint,
  Business,
  BriefSummary,
  OpportunityList,
  OpportunityState,
  ProjectDashboard,
  BriefView,
  CheckerRow,
  DraftedReply,
  ScoredContent,
  Writer,
  WriterEngine,
  WritingEstimate,
  GridEstimate,
  LocalGrid,
  LocalRun,
  LocalSchedule,
  LocalSetup,
  Opened,
  Outbound,
  PlacesReport,
  ReviewsView,
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
  SnapshotComparison,
  SnapshotEstimate,
  SnapshotProgress,
  SnapshotRow,
  SnapshotSchedule,
  SnapshotSettings,
  SnapshotSetup,
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
export type PaidModule = "overview" | "keywords" | "backlinks" | "gap" | "ai_mentions" | "research" | "local";

const PAID: Record<PaidModule, [string, string, string]> = {
  overview: ["overview_estimate", "get_overview", "fetch_overview"],
  keywords: ["keywords_estimate", "get_keywords", "fetch_keywords"],
  backlinks: ["backlinks_estimate", "get_backlinks", "fetch_backlinks"],
  gap: ["gap_estimate", "get_gap", "fetch_gap"],
  ai_mentions: ["ai_mentions_estimate", "get_ai_mentions", "fetch_ai_mentions"],
  research: ["research_estimate", "get_research", "fetch_research"],
  local: ["local_estimate", "get_local", "fetch_local"],
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

  keywordAutocomplete: (projectId: number, seed: string, engine: "google" | "yandex", market?: string) =>
    call<string[]>("keyword_autocomplete", { project_id: projectId, seed, engine, ...(market ? { market } : {}) }),
  keywordLists: (projectId: number) => call<KeywordListRow[]>("keyword_lists", { project_id: projectId }),
  createKeywordList: (projectId: number, name: string) => call<KeywordListRow>("create_keyword_list", { project_id: projectId, name }),
  renameKeywordList: (listId: number, name: string) => call<KeywordListRow>("rename_keyword_list", { list_id: listId, name }),
  deleteKeywordList: (listId: number) => call<null>("delete_keyword_list", { list_id: listId }),
  listKeywords: (listId: number) => call<ListKeyword[]>("list_keywords", { list_id: listId }),
  addToKeywordList: (listId: number, keywords: KeywordData[]) => call<number>("add_to_keyword_list", { list_id: listId, keywords }),
  removeFromKeywordList: (listId: number, keywords: string[]) => call<number>("remove_from_keyword_list", { list_id: listId, keywords }),
  clusterKeywordList: (listId: number, threshold?: number) => call<Topic[]>("cluster_keyword_list", { list_id: listId, ...(threshold ? { threshold } : {}) }),
  serpClusterEstimate: (listId: number) => call<ClusterEstimate>("serp_cluster_estimate", { list_id: listId }),
  startSerpClustering: (listId: number) => call<string>("start_serp_clustering", { list_id: listId }),
  serpClusters: (listId: number, threshold: number, mode: "soft" | "hard") => call<SerpGroups>("serp_clusters", { list_id: listId, threshold, mode }),

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
  rankReport: (projectId: number, trackerId: number | null, days: number, compare: string | null = null) =>
    call<RankReport>("rank_report", { project_id: projectId, days, ...(trackerId === null ? {} : { tracker_id: trackerId }), ...(compare ? { compare } : {}) }),
  rankPlaces: (projectId: number, days: number) => call<PlacesReport>("rank_places", { project_id: projectId, days }),

  localSetup: (projectId: number) => call<LocalSetup>("local_setup", { project_id: projectId }),
  saveLocalSettings: (projectId: number, settings: { keywords: string[]; grid: number; radius_km: number; schedule: LocalSchedule }) =>
    call<LocalSetup>("save_local_settings", { project_id: projectId, ...settings }),
  chooseBusiness: (projectId: number, business: Business) => call<LocalSetup>("choose_business", { project_id: projectId, business }),
  localGridEstimate: (projectId: number) => call<GridEstimate>("local_grid_estimate", { project_id: projectId }),
  startLocalGrid: (projectId: number) => call<string>("start_local_grid", { project_id: projectId }),
  localRuns: (projectId: number) => call<LocalRun[]>("local_runs", { project_id: projectId }),
  localGrid: (projectId: number, runId: number | null) => call<LocalGrid>("local_grid", { project_id: projectId, ...(runId === null ? {} : { run_id: runId }) }),
  localReviewsEstimate: (projectId: number, depth: number) => call<Estimate>("local_reviews_estimate", { project_id: projectId, depth }),
  fetchLocalReviews: (projectId: number, depth: number) => call<{ waiting: number; spent_usd: number }>("fetch_local_reviews", { project_id: projectId, depth }),
  localReviews: (projectId: number) => call<ReviewsView>("local_reviews", { project_id: projectId }),
  opportunities: (projectId: number, lang: string, all = false) => call<OpportunityList>("opportunities", { project_id: projectId, lang, all }),
  setOpportunity: (projectId: number, key: string, state: OpportunityState, days?: number) =>
    call<null>("set_opportunity", { project_id: projectId, key, state, ...(days === undefined ? {} : { days }) }),
  projectDashboard: (projectId: number, lang: string) => call<ProjectDashboard>("project_dashboard", { project_id: projectId, lang }),
  alertSetup: (projectId: number) => call<AlertSetup>("alert_setup", { project_id: projectId }),
  saveAlertSettings: (projectId: number, settings: AlertSettings) => call<AlertSettings>("save_alert_settings", { project_id: projectId, settings }),
  /** Every project's alerts when `projectId` is null. */
  listAlerts: (projectId: number | null, limit = 50, unread = false) => call<AlertList>("list_alerts", { ...(projectId === null ? {} : { project_id: projectId }), limit, unread }),
  /** Every unread alert (of the project) when `ids` is absent. */
  markAlertsRead: (projectId: number | null, ids?: number[]) => call<number>("mark_alerts_read", { ...(projectId === null ? {} : { project_id: projectId }), ...(ids ? { ids } : {}) }),
  checkAlerts: (projectId: number) => call<number>("check_alerts", { project_id: projectId }),
  startLinkWatch: (projectId: number) => call<string>("start_link_watch", { project_id: projectId }),
  contentBriefEstimate: (ask: BriefAsk) => call<Estimate>("content_brief_estimate", { ...ask }),
  startContentBrief: (ask: BriefAsk) => call<{ job_id: string; brief_id: number }>("start_content_brief", { ...ask }),
  contentBriefs: (projectId: number) => call<BriefSummary[]>("content_briefs", { project_id: projectId }),
  contentBrief: (briefId: number) => call<BriefView>("content_brief", { brief_id: briefId }),
  deleteContentBrief: (briefId: number) => call<null>("delete_content_brief", { brief_id: briefId }),
  scoreContent: (ask: { brief_id: number; url?: string; title?: string; description?: string; body?: string; save?: boolean }) => call<ScoredContent>("score_content", ask),
  contentPages: (projectId: number) => call<CheckerRow[]>("content_pages", { project_id: projectId }),
  contentWritingEstimate: (briefId: number, task: WritingTask, scoreId?: number) =>
    call<WritingEstimate>("content_writing_estimate", { brief_id: briefId, task, ...(scoreId === undefined ? {} : { score_id: scoreId }) }),
  startContentWriting: (briefId: number, task: WritingTask, engine: WriterEngine, scoreId?: number) =>
    call<string>("start_content_writing", { brief_id: briefId, task, engine, ...(scoreId === undefined ? {} : { score_id: scoreId }) }),
  replyEstimate: (projectId: number) => call<Writer[]>("reply_estimate", { project_id: projectId }),
  draftReviewReply: (projectId: number, reviewId: string, engine: WriterEngine) =>
    call<DraftedReply>("draft_review_reply", { project_id: projectId, review_id: reviewId, engine }),
  rankKeyword: (projectId: number, keywordId: number, trackerId: number) =>
    call<KeywordDetail>("rank_keyword", { project_id: projectId, keyword_id: keywordId, tracker_id: trackerId }),
  rankVolumesEstimate: (projectId: number) => call<Estimate>("rank_volumes_estimate", { project_id: projectId }),
  fetchRankVolumes: (projectId: number) => call<{ updated: number; spent_usd: number }>("fetch_rank_volumes", { project_id: projectId }),

  auditSetup: (projectId: number) => call<AuditSetup>("audit_setup", { project_id: projectId }),
  saveAuditSettings: (projectId: number, schedule: AuditSchedule, maxPages: number, checkOutbound: boolean) =>
    call<AuditSetup>("save_audit_settings", { project_id: projectId, schedule, max_pages: maxPages, check_outbound: checkOutbound }),
  auditOutbound: (projectId: number, statusClass: number, offset = 0) => call<Outbound | null>("audit_outbound", { project_id: projectId, status_class: statusClass, offset }),
  linkDecisions: (projectId: number) => call<LinkDecision[]>("link_decisions", { project_id: projectId }),
  decideLinks: (projectId: number, domains: string[], decision: LinkDecisionKind | null, note?: string) =>
    call<LinkDecision[]>("decide_links", { project_id: projectId, domains, ...(decision ? { decision } : {}), ...(note ? { note } : {}) }),
  disavowFile: (projectId: number) => call<DisavowFile>("disavow_file", { project_id: projectId }),
  linkProspects: (projectId: number) => call<LinkProspect[]>("link_prospects", { project_id: projectId }),
  saveLinkProspect: (projectId: number, prospect: LinkProspect) => call<LinkProspect>("save_link_prospect", { project_id: projectId, prospect }),
  removeLinkProspect: (projectId: number, domain: string) => call<null>("remove_link_prospect", { project_id: projectId, domain }),
  startAudit: (projectId: number) => call<string>("start_audit", { project_id: projectId }),
  auditLinks: (projectId: number, auditId: number | null) => call<LinksReport | null>("audit_links", { project_id: projectId, ...(auditId === null ? {} : { audit_id: auditId }) }),
  auditHreflang: (projectId: number, auditId: number | null, lang: string, offset = 0) =>
    call<HreflangReport | null>("audit_hreflang", { project_id: projectId, lang, offset, ...(auditId === null ? {} : { audit_id: auditId }) }),
  auditPages: (projectId: number, auditId: number | null, ask: PagesAsk) => call<PagesView | null>("audit_pages", { project_id: projectId, ...ask, ...(auditId === null ? {} : { audit_id: auditId }) }),
  auditPage: (projectId: number, auditId: number, url: string, lang: string) => call<CrawledPageDetail>("audit_page", { project_id: projectId, audit_id: auditId, url, lang }),
  auditFindingsCsv: (projectId: number, auditId: number | null, lang: string) =>
    call<FindingsFile>("audit_findings_csv", { project_id: projectId, lang, ...(auditId === null ? {} : { audit_id: auditId }) }),
  vitalsSetup: (projectId: number) => call<VitalsSetup>("vitals_setup", { project_id: projectId }),
  startVitals: (projectId: number, pages: string[], desktop: boolean) => call<string>("start_vitals", { project_id: projectId, pages, desktop }),
  vitalsReport: (projectId: number) => call<VitalsReport>("vitals_report", { project_id: projectId }),
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
  saveGscSettings: (projectId: number, settings: { property: string | null; ga4_property: string | null; ga4_name: string | null; days: number; schedule: GscSchedule; compare: GscCompare }) =>
    call<GscSetup>("save_gsc_settings", { project_id: projectId, ...settings }),
  startGscSync: (projectId: number) => call<string>("start_gsc_sync", { project_id: projectId }),
  gscReport: (projectId: number) => call<GscReport | null>("gsc_report", { project_id: projectId }),
  gscExport: (projectId: number, kind: "queries" | "pages" | "query_pages") => call<GscExport>("gsc_export", { project_id: projectId, kind }),
  gscQuery: (projectId: number, query: string) => call<QueryPage[]>("gsc_query", { project_id: projectId, query }),

  reportsSetup: (projectId: number) => call<ReportsSetup>("reports_setup", { project_id: projectId }),
  saveReportBranding: (branding: Branding) => call<Branding>("save_report_branding", { branding }),
  saveReport: (projectId: number, reportId: number | null, spec: ReportSpec) =>
    call<ReportRow>("save_report", { project_id: projectId, ...(reportId === null ? {} : { report_id: reportId }), ...spec }),
  deleteReport: (projectId: number, reportId: number) => call<null>("delete_report", { project_id: projectId, report_id: reportId }),
  reportHtml: (projectId: number, reportId: number) => call<{ name: string; html: string }>("report_html", { project_id: projectId, report_id: reportId }),
  reportPreview: (projectId: number, reportId: number | null, spec: ReportSpec) =>
    call<{ name: string; html: string }>("report_preview", { project_id: projectId, ...(reportId === null ? {} : { report_id: reportId }), ...spec }),
  reportPdf: (projectId: number, reportId: number) => call<SavedFile>("report_pdf", { project_id: projectId, report_id: reportId }),
  reportXlsx: (projectId: number, reportId: number) => call<SavedFile>("report_xlsx", { project_id: projectId, report_id: reportId }),
  shareReport: (projectId: number, reportId: number) => call<ReportRow>("share_report", { project_id: projectId, report_id: reportId }),
  unshareReport: (projectId: number, reportId: number) => call<ReportRow>("unshare_report", { project_id: projectId, report_id: reportId }),
  reportDeliveries: (projectId: number, reportId: number) => call<ReportDelivery[]>("report_deliveries", { project_id: projectId, report_id: reportId }),
  sendReport: (projectId: number, reportId: number) => call<string>("send_report", { project_id: projectId, report_id: reportId }),

  categoryNames: (projectId: number) => call<CategoryName[] | null>("get_category_names", { project_id: projectId }),
  fetchCategoryNames: (projectId: number) => call<CategoryName[]>("fetch_category_names", { project_id: projectId }),
  listAds: (projectId: number, domain?: string) => call<AdCopy[]>("list_ads", { project_id: projectId, ...(domain ? { domain } : {}) }),
  metricHistory: (projectId: number, kind: "domain" | "backlinks", ask: { target?: string; market?: string } = {}) =>
    call<MetricPoint[]>("metric_history", { project_id: projectId, kind, ...ask }),
  snapshotSetup: (projectId: number) => call<SnapshotSetup>("snapshot_setup", { project_id: projectId }),
  saveSnapshotSettings: (projectId: number, settings: { schedule: SnapshotSchedule; competitors: string[] }) =>
    call<SnapshotSettings>("save_snapshot_settings", { project_id: projectId, settings }),
  snapshotEstimate: (projectId: number, competitors?: string[]) =>
    call<SnapshotEstimate>("snapshot_estimate", { project_id: projectId, ...(competitors ? { competitors } : {}) }),
  startSnapshot: (projectId: number, ask: { competitors?: string[]; label?: string }) => call<string>("start_snapshot", { project_id: projectId, ...ask }),
  snapshots: (projectId: number) => call<SnapshotRow[]>("list_snapshots", { project_id: projectId }),
  deleteSnapshot: (projectId: number, snapshotId: number) => call<null>("delete_snapshot", { project_id: projectId, snapshot_id: snapshotId }),
  compareSnapshots: (projectId: number, ask: SnapshotPair) => call<SnapshotComparison | null>("compare_snapshots", { project_id: projectId, ...ask }),
  snapshotXlsx: (projectId: number, ask: SnapshotPair) => call<SavedFile>("snapshot_xlsx", { project_id: projectId, ...ask }),
  snapshotProgress: () => call<SnapshotProgress[]>("snapshot_progress"),
};

/** A brief to make: a keyword in the project's market or another, and a page of the project to
 * score against it once made (the page checker). */
export interface BriefAsk {
  project_id: number;
  keyword: string;
  market?: string;
  mobile?: boolean;
  page?: string;
}

/** What an assistant writes from a brief. */
export type WritingTask = "brief" | "improve";

/** Which two snapshots to compare: by default the first finished against the latest. */
export interface SnapshotPair {
  from?: number;
  to?: number;
  /** With `from` absent: the snapshot before `to` instead of the first. */
  previous?: boolean;
  /** Language of the summary. */
  lang?: string;
}
