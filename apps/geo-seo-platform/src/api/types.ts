// Shapes of the platform's commands (crates/octopuson-platform-app). Desktop and web answer alike.

export type Host = "desktop" | "web";
export type Via = "own" | "credit";
export type SourceMode = "auto" | "own" | "credit";
export type SecretState = "unset" | "saved" | "rebind" | "environment";

export type ErrorCode =
  | "invalid"
  | "not_found"
  | "busy"
  | "no_source"
  | "budget"
  | "auth"
  | "quota"
  | "rate_limited"
  | "unavailable"
  | "provider"
  | "unsupported"
  | "account"
  | "internal";

export interface AppInfo {
  name: string;
  version: string;
  host: Host;
  data_dir?: string;
  /** Desktop: the program assistants start with `mcp` (this program itself). */
  mcp_command?: string;
}

/** A personal access token of the web version's MCP server (never the token itself). */
export interface AccessToken {
  id: string;
  name: string;
  /** Unix seconds. */
  created_at: number;
  last_used_at: number | null;
}

/** A token just made: the only time it is shown. */
export interface MadeAccessToken {
  token: string;
  access_token: AccessToken;
}

export interface Settings {
  modes: Record<string, SourceMode>;
  budgets: Record<string, number>;
  proxy?: string;
}

export interface ProjectSpec {
  name: string;
  domain: string;
  country: string;
  language: string;
  competitors: string[];
  brands: string[];
}

export interface ProjectRow extends ProjectSpec {
  id: number;
  created_at: string;
  updated_at: string;
}

export interface KeyInfo {
  name: string;
  state: SecretState;
}

export interface ProviderInfo {
  id: string;
  name: string;
  modules: string[];
  keys: KeyInfo[];
  mode: SourceMode;
  via: Via | null;
  own_ready: boolean;
  credit_connector: string | null;
  credits_per_usd: number | null;
  budget_usd: number | null;
  spent_usd: number;
  signup_url: string;
}

export interface UsageRow {
  month: string;
  provider: string;
  via: Via;
  requests: number;
  cost_usd: number;
}

export interface Account {
  user_id?: string;
  email: string | null;
  name: string | null;
  balance_micros: number;
  held_micros: number;
  available_micros: number;
  credit_unit_micros: number;
  low_balance: boolean;
  portal_url: string;
}

export interface AccountStatus {
  signed_in: boolean;
  account: Account | null;
  error: string | null;
  portal_url: string;
  api: string;
  host: Host;
}

export type JobStatus = "running" | "done" | "partial" | "failed" | "cancelled";

export interface CallErrorBody {
  code: ErrorCode;
  message: string;
  retry_after?: number;
}

export interface Job {
  id: string;
  kind: string;
  label: string;
  project_id?: number;
  status: JobStatus;
  step: string;
  done: number;
  total: number;
  cost_usd: number;
  error?: CallErrorBody;
  result?: unknown;
  started_at: number;
  finished_at?: number;
  revision: number;
}

export interface Kept {
  fetched_at: string;
  fresh: boolean;
}

export interface Blocked {
  code: "no_source" | "budget";
  message: string;
}

export interface Estimate {
  usd: number;
  credits: number | null;
  via: Via | null;
  kept: Kept | null;
  blocked: Blocked | null;
}

export interface Fetched<T> {
  data: T;
  fetched_at: string;
  fresh: boolean;
  cached: boolean;
  spent_usd: number;
  via: Via;
}

export interface RankMetrics {
  keywords: number;
  traffic: number;
  traffic_value_usd: number;
  pos_1: number;
  pos_2_3: number;
  pos_4_10: number;
  pos_11_20: number;
  pos_21_100: number;
  new: number;
  up: number;
  down: number;
  lost: number;
}

export interface DomainOverview {
  target: string;
  organic: RankMetrics;
  paid: RankMetrics;
}

export interface Opened {
  url: string;
  opened: boolean;
}

/** The web version's session. */
export interface WebSession {
  signed_in: boolean;
  email: string | null;
  csrf: string;
  portal_url: string;
}

export type OverviewReport = "summary" | "history" | "keywords" | "pages" | "competitors" | "subdomains" | "compare";

export interface HistoryPoint {
  year: number;
  month: number;
  organic: RankMetrics;
  paid: RankMetrics;
}

export type Change = "new" | "up" | "down" | "same";

export interface RankedKeyword {
  keyword: string;
  position: number | null;
  absolute_position: number | null;
  previous_position: number | null;
  change: Change;
  url: string | null;
  volume: number | null;
  difficulty: number | null;
  cpc: number | null;
  competition: number | null;
  intent: string | null;
  traffic: number;
  traffic_value_usd: number;
  serp_features: string[];
}

export interface RankedKeywords {
  total: number;
  keywords: RankedKeyword[];
}

export interface PageRow {
  url: string;
  organic: RankMetrics;
}

export interface CompetitorRow {
  domain: string;
  shared_keywords: number;
  average_position: number | null;
  shared_traffic: number;
  keywords: number;
  traffic: number;
  traffic_value_usd: number;
}

export interface SubdomainRow {
  subdomain: string;
  organic: RankMetrics;
}

export interface TrafficRow {
  target: string;
  organic_traffic: number;
  organic_keywords: number;
  paid_traffic: number;
  paid_keywords: number;
}

/** What each Site Explorer report holds. */
export interface ReportData {
  summary: DomainOverview;
  history: HistoryPoint[];
  keywords: RankedKeywords;
  pages: PageRow[];
  competitors: CompetitorRow[];
  subdomains: SubdomainRow[];
  compare: TrafficRow[];
}

export type KeywordReport = "overview" | "ideas" | "related" | "suggestions" | "serp" | "wordstat";

export interface Monthly {
  year: number;
  month: number;
  volume: number;
}

export interface KeywordData {
  keyword: string;
  volume: number | null;
  cpc: number | null;
  competition: number | null;
  competition_level: string | null;
  difficulty: number | null;
  intent: string | null;
  other_intents: string[];
  monthly: Monthly[];
  trend: { monthly: number | null; quarterly: number | null; yearly: number | null };
  serp_features: string[];
  results: number | null;
  top_backlinks: number | null;
  top_referring_domains: number | null;
}

export interface KeywordList {
  total: number;
  seed: KeywordData | null;
  keywords: KeywordData[];
}

export interface SerpItem {
  kind: string;
  rank: number | null;
  absolute: number | null;
  domain: string | null;
  url: string | null;
  title: string | null;
  description: string | null;
}

export interface SerpPage {
  keyword: string;
  results: number | null;
  features: string[];
  organic: SerpItem[];
  other: SerpItem[];
  check_url: string | null;
}

export interface KeywordListRow {
  id: number;
  project_id: number;
  name: string;
  keywords: number;
  created_at: string;
}

export interface ListKeyword {
  keyword: string;
  data: Partial<KeywordData> & { keyword: string };
  added_at: string;
}

export interface Topic {
  pillar: string;
  volume: number;
  keywords: { keyword: string; volume: number | null; score: number }[];
}

export type BacklinkReport = "summary" | "backlinks" | "referring_domains" | "anchors" | "pages" | "new_lost" | "bulk";
export type LinkStatus = "live" | "new" | "lost" | "broken";

export interface Share {
  name: string;
  count: number;
}

export interface BacklinkSummary {
  target: string;
  rank: number | null;
  backlinks: number;
  referring_domains: number;
  referring_domains_nofollow: number;
  referring_main_domains: number;
  referring_ips: number;
  referring_subnets: number;
  referring_pages: number;
  broken_backlinks: number;
  broken_pages: number;
  spam_score: number | null;
  first_seen: string | null;
  attributes: Share[];
  link_types: Share[];
  top_level_domains: Share[];
  countries: Share[];
  platforms: Share[];
}

export interface BacklinkRow {
  url_from: string;
  domain_from: string | null;
  title: string | null;
  url_to: string | null;
  url_to_status: number | null;
  anchor: string | null;
  dofollow: boolean;
  kind: string | null;
  page_rank: number | null;
  domain_rank: number | null;
  spam_score: number | null;
  first_seen: string | null;
  last_seen: string | null;
  is_new: boolean;
  is_lost: boolean;
  is_broken: boolean;
  country: string | null;
}

export interface ReferringDomainRow {
  domain: string;
  rank: number | null;
  backlinks: number;
  nofollow_only: boolean;
  spam_score: number | null;
  first_seen: string | null;
}

export interface AnchorRow {
  anchor: string;
  backlinks: number;
  referring_domains: number;
  referring_domains_nofollow: number;
  first_seen: string | null;
}

export interface LinkedPageRow {
  url: string;
  title: string | null;
  status: number | null;
  rank: number | null;
  backlinks: number;
  referring_domains: number;
  broken_backlinks: number;
}

export interface NewLostPoint {
  date: string;
  new_backlinks: number;
  lost_backlinks: number;
  new_referring_domains: number;
  lost_referring_domains: number;
}

export interface RankRow {
  target: string;
  rank: number | null;
}

export type GapReport = "missing" | "shared" | "links" | "bulk";

export interface Ranking {
  domain: string;
  position: number | null;
  url: string | null;
  traffic: number;
}

export interface GapKeyword {
  keyword: string;
  volume: number | null;
  difficulty: number | null;
  cpc: number | null;
  intent: string | null;
  own: Ranking | null;
  competitors: Ranking[];
  weak: boolean;
}

export interface KeywordGap {
  own: string;
  competitors: string[];
  keywords: GapKeyword[];
}

export interface LinkGapRow {
  domain: string;
  rank: number | null;
  backlinks: Record<string, number>;
}

export interface BulkRow {
  target: string;
  organic_traffic: number;
  organic_keywords: number;
  paid_traffic: number;
  rank: number | null;
}

// ----- Rank Tracker -----

export type Schedule = "daily" | "weekly" | "manual";
export type SearchEngine = "google" | "bing" | "yandex";
export type Device = "desktop" | "mobile";

export interface RankSettings {
  schedule: Schedule;
  /** Results looked at, 10–100. */
  depth: number;
  ai_overview: boolean;
  last_run_at: string | null;
}

export interface TrackerRow {
  id: number;
  project_id: number;
  engine: SearchEngine;
  country: string;
  language: string;
  device: Device;
  location: string | null;
  created_at: string;
}

export interface RankSetup {
  settings: RankSettings;
  trackers: TrackerRow[];
  keywords: number;
  missing_volumes: number;
  next_check_at: string | null;
}

export interface NewKeyword {
  keyword: string;
  volume?: number | null;
}

export interface RankEstimate {
  checks: number;
  run: Estimate;
  runs_per_month: number;
  month_usd: number;
  month_credits: number | null;
}

export interface KeywordRank {
  keyword_id: number;
  keyword: string;
  tags: string[];
  volume: number | null;
  position: number | null;
  previous: number | null;
  change: number | null;
  best: number | null;
  url: string | null;
  cannibalised: boolean;
  checked_at: string | null;
  /** [YYYY-MM-DD, position] by day, oldest first. */
  history: [string, number | null][];
  error: string | null;
  pages: [number, string][];
  features: string[];
  ai_overview: boolean;
  ai_cited: boolean;
  snippet: boolean;
}

export interface RankSummary {
  visibility: number | null;
  visibility_change: number | null;
  average_position: number | null;
  top3: number;
  top10: number;
  ranked: number;
  checked: number;
  improved: number;
  declined: number;
  lost: number;
  ai_overviews: number;
  ai_cited: number;
  snippets: number;
  cannibalised: number;
}

export interface VisibilityPoint {
  date: string;
  visibility: number;
  top3: number;
  top10: number;
  ranked: number;
}

export interface CompetitorShare {
  domain: string;
  visibility: number;
  keywords: number;
  /** Keywords it ranks for in the first three results. */
  top3: number;
  /** Keywords it ranks for in the first ten results. */
  top10: number;
  average_position: number | null;
  own: boolean;
}

export interface SharePoint {
  date: string;
  shares: Record<string, number>;
}

export interface RankRun {
  id: number;
  project_id: number;
  scheduled: boolean;
  status: "running" | "done" | "partial" | "failed" | "cancelled" | "interrupted";
  total: number;
  checked: number;
  failed: number;
  cost_usd: number;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface RankReport {
  tracker: TrackerRow | null;
  rows: KeywordRank[];
  summary: RankSummary;
  visibility: VisibilityPoint[];
  competitors: CompetitorShare[];
  share_history: SharePoint[];
  last_run: RankRun | null;
  since: string;
}

export interface CheckResult {
  position: number;
  domain: string;
  url: string;
  title?: string;
}

export interface CheckPoint {
  checked_at: string;
  position: number | null;
  url: string | null;
  pages: [number, string][];
  features: string[];
  ai_overview: boolean;
  ai_cited: boolean;
  snippet: boolean;
  error: string | null;
}

export interface TrackedKeyword {
  id: number;
  project_id: number;
  keyword: string;
  tags: string[];
  volume: number | null;
  added_at: string;
}

export interface KeywordDetail {
  keyword: TrackedKeyword;
  tracker: TrackerRow;
  checks: CheckPoint[];
  results: CheckResult[];
}

// ----- Site Audit -----

export type AuditSchedule = "weekly" | "monthly" | "manual";
export type AuditStatus = "running" | "done" | "stopped" | "failed" | "interrupted";

export interface AuditSettings {
  schedule: AuditSchedule;
  max_pages: number;
  last_run_at: string | null;
}

export interface AuditRow {
  id: number;
  project_id: number;
  crawl_id: string | null;
  scheduled: boolean;
  status: AuditStatus;
  start_url: string;
  pages: number;
  health: number | null;
  geo: number | null;
  issues: number;
  metered_pages: number;
  charged_micros: number;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface MeterInfo {
  micros_per_unit: number;
  free_per_month: number;
  credit_unit_micros: number;
}

export interface AuditSetup {
  settings: AuditSettings;
  max_pages_limit: number;
  next_audit_at: string | null;
  meter: MeterInfo | null;
  audits: AuditRow[];
}

export type IssueKind = "issue" | "warning" | "opportunity";
export type IssuePriority = "high" | "medium" | "low";

export interface IssueReport {
  id: string;
  title: string;
  kind: IssueKind;
  priority: IssuePriority;
  count: number;
  previous: number | null;
}

export interface CategoryReport {
  id: string;
  name: string;
  issues: IssueReport[];
}

export interface AuditOverview {
  health: number | null;
  internal: number;
  external: number;
  indexable: number;
  html: number;
  blocked: number;
  errors: number;
  redirects: number;
  average_response_ms: number;
  affected_urls: number;
}

export type GeoStatus = "good" | "warning" | "poor";

export interface GeoSignal {
  id: string;
  weight: number;
  value: number;
  status: GeoStatus;
  passed: number;
  total: number;
  examples: string[];
}

export interface GeoCrawler {
  id: string;
  name: string;
  operator: string;
  purpose: "search" | "assistant" | "training";
  allowed: boolean;
  rule: string | null;
}

export interface GeoReadiness {
  score: number | null;
  pages: number;
  signals: GeoSignal[];
  crawlers: GeoCrawler[];
  robots: string | null;
  schema_types: [string, number][];
}

export interface AuditReport {
  audit: AuditRow;
  previous: AuditRow | null;
  overview: AuditOverview | null;
  categories: CategoryReport[];
  geo: GeoReadiness | null;
}

export interface IssueDetail {
  id: string;
  title: string;
  description: string;
  fix: string;
  kind: IssueKind;
  priority: IssuePriority;
  total: number;
  offset: number;
  columns: string[];
  rows: string[][];
}

// AI visibility (`ai_*` commands).

export type AiEngine = "chatgpt" | "claude" | "gemini" | "perplexity" | "google_ai_mode";
export type AiSchedule = "weekly" | "monthly" | "manual";
/** How an assistant is asked: the user's own key for it, DataForSEO on the user's key, or credits. */
export type AiVia = "own" | "dataforseo" | "credit";

export interface Rival {
  domain: string;
  names: string[];
}

export interface AiSettings {
  schedule: AiSchedule;
  engines: AiEngine[];
  /** Model chosen per assistant; the default when missing. */
  models: Partial<Record<AiEngine, string>>;
  /** Rival brands; the project's competitors when empty. */
  rivals: Rival[];
  last_run_at: string | null;
}

export interface AiPrompt {
  id: number;
  project_id: number;
  prompt: string;
  tags: string[];
  added_at: string;
}

export interface EngineInfo {
  engine: AiEngine;
  enabled: boolean;
  via: AiVia | null;
  model: string | null;
  /** The source whose own key asks it directly (none for Google AI Mode). */
  provider: string | null;
}

/** A brand watched in the answers; `key` is its domain. */
export interface Brand {
  key: string;
  names: string[];
  own: boolean;
}

export interface AiSetup {
  settings: AiSettings;
  prompts: AiPrompt[];
  engines: EngineInfo[];
  brands: Brand[];
  next_run_at: string | null;
}

export interface EngineCost {
  engine: AiEngine;
  via: AiVia;
  model: string;
  answers: number;
  usd: number;
  credits: number | null;
}

export interface AiEstimate {
  engines: EngineCost[];
  answers: number;
  usd: number;
  credits: number;
  runs_per_month: number;
  month_usd: number;
  month_credits: number;
  unavailable: AiEngine[];
}

export interface AiRun {
  id: number;
  project_id: number;
  scheduled: boolean;
  status: "running" | "done" | "partial" | "failed" | "cancelled" | "interrupted";
  total: number;
  checked: number;
  failed: number;
  cost_usd: number;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface Visibility {
  answers: number;
  mentioned: number;
  cited: number;
  /** 0–100. */
  mention_rate: number;
  citation_rate: number;
  average_position: number | null;
  /** -1 to 1. */
  sentiment: number | null;
  share_of_voice: number;
}

export interface EngineVisibility extends Visibility {
  engine: AiEngine;
}

export interface BrandShare {
  key: string;
  own: boolean;
  mentions: number;
  citations: number;
  share: number;
  average_position: number | null;
}

export interface AiRunPoint {
  run_id: number;
  date: string;
  mention_rate: number;
  citation_rate: number;
  share_of_voice: number;
  engines: Partial<Record<AiEngine, number>>;
}

export interface AiCell {
  mentioned: boolean;
  /** Its place in the answer: the number of the list item naming it, else among the brands named. */
  position: number | null;
  cited: boolean;
  sentiment: number | null;
  error: string | null;
  checked_at: string;
  /** The best-placed rival the answer names. */
  rival: { key: string; position: number } | null;
}

export interface AiPromptRow {
  prompt_id: number;
  prompt: string;
  tags: string[];
  results: Partial<Record<AiEngine, AiCell>>;
}

export interface SourceCount {
  key: string;
  title: string | null;
  own: boolean;
  answers: number;
}

export interface AiReport {
  summary: Visibility;
  engines: EngineVisibility[];
  brands: BrandShare[];
  history: AiRunPoint[];
  prompts: AiPromptRow[];
  top_domains: SourceCount[];
  own_pages: SourceCount[];
  last_run: AiRun | null;
  since: string;
}

export interface AnswerBrand {
  key: string;
  own: boolean;
  position: number | null;
  cited: boolean;
  sentiment: number | null;
}

export interface AnswerSource {
  url: string;
  title?: string;
  domain: string;
}

export interface AiAnswer {
  run_id: number;
  prompt_id: number;
  engine: AiEngine;
  via: AiVia;
  model: string | null;
  checked_at: string;
  text: string;
  mentioned: boolean;
  position: number | null;
  cited: boolean;
  sentiment: number | null;
  brands: AnswerBrand[];
  citations: AnswerSource[];
  error: string | null;
}

export interface AnswerDetail {
  prompt: AiPrompt;
  engine: AiEngine;
  brands: Brand[];
  /** Newest first. */
  answers: AiAnswer[];
}

export type MentionsReport = "summary" | "queries" | "compare";

export interface MentionGroup {
  key: string;
  mentions: number;
  ai_search_volume: number;
}

export interface MentionMetrics {
  target: string;
  mentions: number;
  ai_search_volume: number;
  platforms: MentionGroup[];
  sources: MentionGroup[];
  brands: MentionGroup[];
}

export interface MentionRow {
  platform: string;
  question: string;
  answer: string;
  sources: { domain: string; url: string | null; title: string | null }[];
  ai_search_volume: number | null;
  last_response_at: string | null;
}

// Search Console and Google Analytics (`google_*` and `gsc_*` commands).

export interface GoogleStatus {
  /** A client is there to sign in with. */
  configured: boolean;
  /** The desktop user's own OAuth client id. */
  own_client_id: string | null;
  /** The program's built-in client is used. */
  built_in: boolean;
  connected: boolean;
  search_console: boolean;
  analytics: boolean;
}

/** A desktop sign-in runs as a job; the web opens Google's page. */
export interface GoogleLogin {
  job_id: string | null;
  url: string | null;
}

export interface GscSite {
  site_url: string;
  permission: string;
}

export interface Ga4Property {
  id: string;
  name: string;
  account: string;
}

export interface GoogleProperties {
  search_console: GscSite[];
  analytics: Ga4Property[];
  analytics_error: string | null;
  /** The property covering the project's domain. */
  suggested: string | null;
}

export type GscSchedule = "daily" | "weekly" | "manual";

export interface GscSettings {
  property: string | null;
  ga4_property: string | null;
  ga4_name: string | null;
  days: number;
  schedule: GscSchedule;
  last_sync_at: string | null;
}

export interface GscSync {
  id: number;
  project_id: number;
  scheduled: boolean;
  status: "running" | "done" | "partial" | "failed" | "cancelled" | "interrupted";
  period_start: string | null;
  period_end: string | null;
  rows: number;
  currency: string | null;
  limited: boolean;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface GscSetup {
  settings: GscSettings;
  google: GoogleStatus;
  syncs: GscSync[];
  next_sync_at: string | null;
}

export interface SearchTotals {
  clicks: number;
  impressions: number;
  /** 0–100. */
  ctr: number;
  position: number | null;
}

export interface TrafficTotals {
  sessions: number;
  engaged_sessions: number;
  key_events: number;
  revenue: number;
}

export interface GscDay {
  date: string;
  period: "now" | "before";
  clicks: number;
  impressions: number;
  position: number;
  sessions: number | null;
  key_events: number | null;
}

export interface GscQuery {
  query: string;
  clicks: number;
  impressions: number;
  ctr: number;
  position: number;
  clicks_change: number | null;
  /** Places gained (positive) or lost. */
  position_change: number | null;
  pages: number;
  tracked: boolean;
  near_top: boolean;
  low_ctr: boolean;
}

export interface PageAudit {
  status: number | null;
  indexability: string;
  findings: number;
}

export interface GscPage {
  page: string;
  clicks: number;
  impressions: number;
  ctr: number;
  position: number;
  clicks_change: number | null;
  sessions: number | null;
  key_events: number | null;
  revenue: number | null;
  audit: PageAudit | null;
}

export interface SearchRow {
  key: string;
  clicks: number;
  impressions: number;
  position: number;
}

export interface QueryPage {
  query: string;
  page: string;
  clicks: number;
  impressions: number;
  position: number;
}

export interface Cannibalised {
  query: string;
  impressions: number;
  pages: { page: string; clicks: number; impressions: number; position: number }[];
}

export interface GscReport {
  sync: GscSync;
  period: [string, string];
  previous: [string, string];
  now: SearchTotals;
  before: SearchTotals;
  traffic_now: TrafficTotals | null;
  traffic_before: TrafficTotals | null;
  currency: string | null;
  daily: GscDay[];
  queries: GscQuery[];
  pages: GscPage[];
  devices: SearchRow[];
  countries: SearchRow[];
  cannibalised: Cannibalised[];
  audit: { audit_id: number; finished_at: string | null; problem_pages: number } | null;
  /** Tracked keywords with Search Console numbers, tracked keywords in all. */
  tracked: [number, number];
}

// Reports (`report*` commands).

export type ReportWidget = "summary" | "rankings" | "competitors" | "search_console" | "site_audit" | "ai_visibility" | "backlinks";
export type ReportSchedule = "manual" | "weekly" | "monthly";
export type ReportLanguage = "tr" | "en" | "de" | "es";

export interface Branding {
  company: string | null;
  /** A `data:image/…;base64,` address. */
  logo: string | null;
  /** `#rrggbb`. */
  accent: string | null;
  website: string | null;
  footer: string | null;
  hide_octopuson: boolean;
}

export interface ReportSpec {
  name: string;
  language: ReportLanguage;
  days: number;
  widgets: ReportWidget[];
  schedule: ReportSchedule;
  recipients: string[];
}

export interface ReportRow extends ReportSpec {
  id: number;
  project_id: number;
  share_token: string | null;
  share_views: number;
  last_sent_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ReportsSetup {
  reports: ReportRow[];
  branding: Branding;
  widgets: ReportWidget[];
  can_print: boolean;
  can_share: boolean;
  can_email: boolean;
  public_url: string | null;
}

export interface ReportDelivery {
  id: number;
  report_id: number;
  project_id: number;
  scheduled: boolean;
  status: "sent" | "failed";
  recipients: number;
  error: string | null;
  sent_at: string;
}

/** A file to save: base64 bytes. */
export interface SavedFile {
  name: string;
  content_type: string;
  data: string;
}
