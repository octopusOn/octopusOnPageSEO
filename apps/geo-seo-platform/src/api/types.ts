// Shapes of the platform's commands (crates/octopuson-platform-app). Desktop and web answer alike.

import type { PageId } from "../lib/route";

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

/** Why a sync waits for the person: both sides hold projects, another account, or the web's
 * sync records started over. */
export interface SyncChoice {
  reason: "first" | "account" | "reset";
  here: number;
  web: number;
}

/** Where syncing this desktop with the account's web workspace stands. */
export interface SyncStatus {
  host: Host;
  enabled: boolean;
  signed_in: boolean;
  /** Syncing: scheduled work runs on the web. */
  active: boolean;
  syncing: boolean;
  choice?: SyncChoice;
  /** Unix milliseconds. */
  last_at?: number;
  last_error?: string;
  unsent: number;
  parked: number;
  url: string;
}

export type SyncAnswer = "merge" | "upload" | "download" | "pause";

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
  /** Always ask providers anew: no discounted answers kept by the gateway. */
  fresh_only?: boolean;
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
  /** On credits, when the gateway keeps an answer for every account: its discounted price. */
  shared?: { credits: number; full_credits: number } | null;
}

export interface Fetched<T> {
  data: T;
  fetched_at: string;
  fresh: boolean;
  cached: boolean;
  spent_usd: number;
  via: Via;
  /** What the gateway's discount on a kept answer saved now, US dollars. */
  saved_usd?: number;
  /** Every row the source holds for a list question, of which `data` is one page. */
  total?: number;
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

export type OverviewReport = "summary" | "history" | "keywords" | "pages" | "competitors" | "subdomains" | "compare" | "countries";

/** Which of a domain's keywords a list holds: organic now, organic lost, or ads. */
export type KeywordSet = "organic" | "lost" | "paid";

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
  /** The ad shown, for paid keywords. */
  ad?: AdText;
}

export interface AdText {
  title: string;
  description: string | null;
  breadcrumb: string | null;
}

/** An ad text a domain was seen showing, from fetched paid keywords. */
export interface AdCopy {
  domain: string;
  title: string;
  description: string | null;
  breadcrumb: string | null;
  url: string | null;
  keywords: string[];
  first_seen: string;
  last_seen: string;
}

/** A domain's rankings in one country. */
export interface CountryRow {
  country: string | null;
  location_code: number;
  language: string | null;
  organic: RankMetrics;
  paid: RankMetrics;
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
  countries: CountryRow[];
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

/** One result of a result page and its weight. */
export interface ResultWeight {
  rank: number;
  url: string;
  page_rank: number | null;
  referring_domains: number | null;
  traffic: number | null;
  keywords: number | null;
}

/** What a keyword's first result ranks for: the parent topic and the traffic potential. */
export interface ParentTopic {
  url: string;
  parent: RankedKeyword | null;
  reach: { traffic: number; keywords: number };
  keywords: RankedKeyword[];
}

/** A keyword's numbers in one market. */
export interface MarketVolume {
  market: string;
  data: KeywordData | null;
}

/** A result page as it was on a day. */
export interface PastSerp {
  date: string;
  results_count: number | null;
  features: string[];
  results: { rank: number; domain: string | null; url: string; title: string | null }[];
}

/** What each research report holds. */
export interface ResearchData {
  history: Monthly[];
  serp_history: PastSerp[];
  serp_metrics: ResultWeight[];
  parent: ParentTopic;
  countries: MarketVolume[];
}

export type ResearchReport = keyof ResearchData;

/** What grouping a list by result pages would fetch. */
export interface ClusterEstimate {
  keywords: number;
  kept: number;
  estimate: Estimate | null;
}

export interface SerpGroupMember {
  keyword: string;
  volume: number | null;
  difficulty: number | null;
  intent: string | null;
  shared: number | null;
}

/** Keywords one page can target, as the result pages show. */
export interface SerpGroup {
  pillar: string;
  volume: number;
  difficulty: number | null;
  intent: string | null;
  pages: string[];
  keywords: SerpGroupMember[];
}

export interface SerpGroups {
  groups: SerpGroup[];
  without_page: string[];
}

export interface Topic {
  pillar: string;
  volume: number;
  keywords: { keyword: string; volume: number | null; score: number }[];
}

export type BacklinkReport = "summary" | "backlinks" | "referring_domains" | "anchors" | "pages" | "new_lost" | "bulk" | "history" | "audit";

/** A domain's link profile on the first day of a month. */
export interface LinkHistoryPoint {
  month: string;
  rank: number | null;
  backlinks: number;
  referring_domains: number;
  referring_main_domains: number;
  new_backlinks: number;
  lost_backlinks: number;
  new_referring_domains: number;
  lost_referring_domains: number;
}
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
  ip?: string | null;
  tld?: string | null;
  links_from_domain?: number | null;
  page_external_links?: number | null;
  redirect_target?: string | null;
}

export type LinkRiskLevel = "low" | "medium" | "high";

/** A linking site judged by the toxic-link rules. */
export interface LinkRisk {
  domain: string;
  url_from: string;
  url_to: string | null;
  anchor: string | null;
  dofollow: boolean;
  spam_score: number | null;
  domain_rank: number | null;
  network: string | null;
  tld: string | null;
  links: number | null;
  country: string | null;
  first_seen: string | null;
  flags: string[];
  score: number;
  risk: LinkRiskLevel;
}

export type LinkDecisionKind = "keep" | "watch" | "disavow";

export interface LinkDecision {
  domain: string;
  decision: LinkDecisionKind;
  note: string | null;
  decided_at: string;
}

export type ProspectSource = "gap" | "lost" | "broken" | "competitor_page" | "manual";
export type ProspectStatus = "prospect" | "contacted" | "won" | "lost";

/** A site that could link to the project, with its outreach state. */
export interface LinkProspect {
  domain: string;
  url?: string | null;
  source: ProspectSource;
  status: ProspectStatus;
  note?: string | null;
  added_at?: string;
  updated_at?: string;
}

export interface DisavowFile {
  file_name: string;
  text: string;
  domains: number;
}

/** Links from the site to other sites in an audit's crawl. */
export interface Outbound {
  audit_id: number;
  checked: boolean;
  total: number;
  offset: number;
  columns: string[];
  rows: string[][];
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

/** Semrush's keyword gap groups. */
export type GapGroup = "shared" | "missing" | "weak" | "strong" | "untapped" | "unique";

/** A keyword with where the site (first) and each competitor rank for it. */
export interface MatrixKeyword {
  keyword: string;
  volume: number | null;
  difficulty: number | null;
  cpc: number | null;
  intent: string | null;
  rankings: (Ranking | null)[];
  groups: GapGroup[];
}

/** The keywords of a site and its competitors side by side. */
export interface GapMatrix {
  own: string;
  competitors: string[];
  rows_per_domain: number;
  keywords_per_domain: number[];
  counts: Record<GapGroup, number>;
  keywords: MatrixKeyword[];
}

/** A keyword a competitor's page ranks for and the site's page does not. */
export interface PageGapRow {
  keyword: string;
  volume: number | null;
  difficulty: number | null;
  cpc: number | null;
  intent: string | null;
  position: number | null;
  traffic: number;
}

/** A domain's organic traffic month by month. */
export interface TrafficHistory {
  target: string;
  months: { year: number; month: number; traffic: number; keywords: number }[];
}

/** A category a domain ranks in. */
export interface DomainCategory {
  code: number;
  path: number[];
  traffic: number;
  keywords: number;
}

/** A domain getting traffic in a category. */
export interface CategoryPlayer {
  domain: string;
  traffic: number;
  keywords: number;
  traffic_change: number | null;
}

export interface CategoryName {
  code: number;
  name: string;
  parent: number | null;
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
  /** A run started by hand: live result pages. */
  run: Estimate;
  /** Scheduled runs go through the standard queue (cheaper, answers within minutes). */
  queued: boolean;
  /** List price of one scheduled run. */
  scheduled_usd: number;
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
  /** Elements of the latest result page that show the site. */
  owned: string[];
  /** Estimated monthly visits: searches × the click share of the position. */
  traffic: number | null;
  /** The listed competitors' positions on the latest result page. */
  rivals: Record<string, number | null>;
  /** The position on the compared day, when one was asked for. */
  compared?: { position: number | null; checked_at: string };
}

export interface FeatureCount {
  feature: string;
  /** Keywords whose latest page shows it. */
  shown: number;
  /** Of those, keywords where it shows the site. */
  owned: number;
}

export interface LandingPage {
  url: string;
  keywords: number;
  best_position: number;
  traffic: number;
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
  /** Estimated monthly visits from the tracked keywords. */
  traffic: number;
  features: FeatureCount[];
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
  /** Checks still waiting in the provider's queue. */
  waiting: number;
}

export interface RankReport {
  tracker: TrackerRow | null;
  rows: KeywordRank[];
  summary: RankSummary;
  visibility: VisibilityPoint[];
  competitors: CompetitorShare[];
  /** Estimated monthly visits of the site and each competitor. */
  traffic: Record<string, number>;
  landing_pages: LandingPage[];
  share_history: SharePoint[];
  last_run: RankRun | null;
  since: string;
  /** The day compared with (yyyy-mm-dd). */
  compare: string | null;
}

export interface PlaceRank {
  tracker_id: number;
  position: number | null;
  change: number | null;
  url: string | null;
  checked_at: string | null;
  error: string | null;
}

export interface KeywordPlaces {
  keyword_id: number;
  keyword: string;
  tags: string[];
  volume: number | null;
  /** In the order of the places. */
  places: PlaceRank[];
}

export interface PlaceSummary {
  tracker_id: number;
  visibility: number | null;
  average_position: number | null;
  top10: number;
  ranked: number;
}

export interface PlacesReport {
  trackers: TrackerRow[];
  rows: KeywordPlaces[];
  summaries: PlaceSummary[];
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
  /** Links to other sites are checked once each (each counts as a page). */
  check_outbound?: boolean;
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
  /** Each brand's share of the run's brand mentions: the own one and the most named rivals. */
  brands: Record<string, number>;
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
  /** Pages of any site the latest answers cite most. */
  top_pages: SourceCount[];
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
  /** The period is compared with as many days before it, or the same days a year before. */
  compare: GscCompare;
}

export type GscCompare = "previous" | "year";

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
  /** It names the brand. */
  brand: boolean;
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

/** Searches naming the brand and the others, in both periods. */
export interface BrandSplit {
  terms: string[];
  brand: SearchTotals;
  other: SearchTotals;
  brand_before: SearchTotals;
  other_before: SearchTotals;
}

/** Every row a sync read, for a CSV file. */
export interface GscExport {
  columns: string[];
  rows: (string | number | boolean | null)[][];
}

export interface GscReport {
  sync: GscSync;
  period: [string, string];
  previous: [string, string];
  compare: GscCompare;
  brand: BrandSplit;
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

export type ReportWidget = "summary" | "rankings" | "competitors" | "search_console" | "site_audit" | "ai_visibility" | "backlinks" | "comparison" | "comparison_previous" | "local" | "organic" | "link_history" | "gap" | "keyword_lists" | "opportunities";
export type ReportSchedule = "manual" | "weekly" | "monthly" | "snapshot";
export type ReportLanguage = "tr" | "en" | "de" | "es";
/** How a report looks: white, deep sea (the program's own), the agency's colour, elegant. */
export type ReportDesign = "clean" | "ocean" | "accent" | "elegant";

/** A report's own colours, `#rrggbb`, each over its design's; `null` keeps the design's. */
export interface ReportColors {
  /** Section numbers, highlights, charts, bars and the contents' numbers. */
  accent: string | null;
  /** The second colour of gradients and chart lines. */
  accent2: string | null;
  /** The cover's background. */
  cover: string | null;
  /** Section, table and chart titles. */
  heading: string | null;
  /** Table heads' background. */
  table: string | null;
}

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
  design: ReportDesign;
  colors: ReportColors;
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
  /** Designs, the default first. */
  designs: ReportDesign[];
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

// Snapshots (`snapshot*` commands).

export type SnapshotSchedule = "manual" | "weekly" | "monthly";
export type SnapshotStatus = "running" | "done" | "partial" | "failed" | "interrupted";

export interface SnapshotSettings {
  schedule: SnapshotSchedule;
  /** Competitors kept too (some of the project's). */
  competitors: string[];
  last_run_at: string | null;
}

/** A finished snapshot's main numbers. */
export interface SnapshotHeadline {
  traffic: number | null;
  keywords: number | null;
  top10: number | null;
  oa: number | null;
  referring_domains: number | null;
  visibility: number | null;
  health: number | null;
  mention_rate: number | null;
  clicks: number | null;
  /** Share of local voice (snapshots from version 2). */
  solv?: number | null;
  /** High-risk linking sites not disavowed (snapshots from version 2). */
  risky_links?: number | null;
  /** Parts that could not be taken. */
  missing: number;
}

export interface SnapshotRow {
  id: number;
  project_id: number;
  label: string | null;
  kind: "manual" | "scheduled";
  status: SnapshotStatus;
  taken_at: string;
  finished_at: string | null;
  cost_usd: number;
  /** Why parts are missing, or why it failed. */
  error: string | null;
  headline: SnapshotHeadline | null;
}

/** A target's headline numbers on a day they were fetched (metric history). */
export interface MetricPoint {
  target: string;
  kind: "domain" | "backlinks";
  /** `country-language` of domain numbers; empty for links. */
  market: string;
  /** `YYYY-MM-DD`. */
  day: string;
  metrics: Record<string, unknown>;
  taken_at: string;
}

export interface SnapshotSetup {
  settings: SnapshotSettings;
  /** The project's competitors to choose from. */
  competitors: string[];
  /** Newest first. */
  snapshots: SnapshotRow[];
  /** The first finished snapshot: where the work began. */
  baseline_id: number | null;
  /** When the next one is taken on its own, seconds since the Unix epoch. */
  next_at: number | null;
}

export interface SnapshotEstimate {
  /** Provider list price of the parts to pay for. */
  usd: number;
  credits: number | null;
  /** Credits without the shared-cache discount. */
  full_credits: number | null;
  via: Via | null;
  /** Paid parts kept fresh by the modules: free. */
  kept: number;
  /** Paid parts to pay for. */
  paid: number;
  blocked: Blocked | null;
}

export interface SnapshotSide {
  id: number;
  label: string | null;
  taken_at: string;
}

export type KpiGroup = "organic" | "links" | "rank" | "audit" | "ai" | "gsc" | "local";

export interface SnapshotKpi {
  key: string;
  group: KpiGroup;
  before: number | null;
  after: number | null;
  change: number | null;
  /** The change in percent of before. */
  percent: number | null;
  higher_better: boolean;
}

export interface KeywordMove {
  keyword: string;
  before: number | null;
  after: number | null;
  /** Places gained (positive) or lost. */
  change: number | null;
  traffic_before: number;
  traffic_after: number;
  url: string | null;
}

export interface KeywordMoves {
  gained: number;
  lost: number;
  improved: number;
  declined: number;
  entered_top10: number;
  left_top10: number;
  new_keywords: KeywordMove[];
  lost_keywords: KeywordMove[];
  improved_keywords: KeywordMove[];
  declined_keywords: KeywordMove[];
}

export interface PageMove {
  url: string;
  before: number;
  after: number;
  change: number;
}

export interface LinkingDomain {
  domain: string;
  rank: number | null;
  backlinks: number;
}

export interface DomainMoves {
  gained: number;
  lost: number;
  gained_domains: LinkingDomain[];
  lost_domains: LinkingDomain[];
}

export interface TrackedMove {
  keyword: string;
  before: number | null;
  after: number | null;
  change: number | null;
}

export interface IssueMove {
  id: string;
  title: string;
  before: number;
  after: number;
}

export interface CellMove {
  prompt: string;
  engine: AiEngine;
  before: number | null;
  after: number | null;
}

export interface BrandMove {
  key: string;
  own: boolean;
  before: number;
  after: number;
}

export interface SiteKpis {
  domain: string;
  own: boolean;
  kpis: SnapshotKpi[];
}

export interface SummarySentence {
  tone: "good" | "bad" | "note";
  text: string;
}

/** Two snapshots compared, with the summary. */
export interface SnapshotComparison {
  from: SnapshotSide;
  to: SnapshotSide;
  days: number;
  kpis: SnapshotKpi[];
  keywords: KeywordMoves | null;
  pages: { up: PageMove[]; down: PageMove[] } | null;
  domains: DomainMoves | null;
  tracked: { improved: number; declined: number; moves: TrackedMove[] } | null;
  audit: { fixed: IssueMove[]; new_issues: IssueMove[]; better: IssueMove[]; worse: IssueMove[] } | null;
  ai: { gained: CellMove[]; lost: CellMove[]; brands: BrandMove[] } | null;
  /** Map keywords in both grids, the largest moves first. */
  local?: { improved: number; declined: number; keywords: LocalMove[] } | null;
  /** High-risk linking sites found since, and those gone or disavowed. */
  link_audit?: { new_risky: string[]; cleared: string[] } | null;
  sites: SiteKpis[];
  summary: SummarySentence[];
}

export interface LocalMove {
  keyword: string;
  arp_before: number | null;
  arp_after: number | null;
  solv_before: number;
  solv_after: number;
}

/** How far a project has come since its first snapshot. */
export interface SnapshotProgress {
  project_id: number;
  from: SnapshotSide;
  to: SnapshotSide;
  days: number;
  kpis: SnapshotKpi[];
}

// ----- Local SEO -----

export type LocalSchedule = "weekly" | "monthly" | "manual";

/** A business on Google: its ids, place and rating. */
export interface Business {
  title: string;
  cid: string | null;
  place_id: string | null;
  category: string | null;
  address: string | null;
  domain: string | null;
  url: string | null;
  phone: string | null;
  lat: number;
  lng: number;
  rating: number | null;
  votes: number | null;
  is_claimed: boolean | null;
}

export interface DayHours {
  /** `monday` … `sunday`. */
  day: string;
  /** `08:00–22:30` spans; empty when closed. */
  hours: string[];
}

/** A business's Google profile. */
export interface BusinessProfile {
  business: Business;
  description: string | null;
  additional_categories: string[];
  /** Reviews by stars, `"1"` … `"5"`. */
  rating_distribution: Record<string, number>;
  total_photos: number | null;
  main_image: string | null;
  price_level: string | null;
  hours: DayHours[];
  status: string | null;
  attributes: string[];
  topics: [string, number][];
  check_url: string | null;
}

export interface AroundBusiness extends Business {
  own: boolean;
  distance_km: number;
}

export interface LocalCompetitors {
  businesses: AroundBusiness[];
  total: number | null;
}

export interface GridPoint {
  row: number;
  col: number;
  lat: number;
  lng: number;
}

export interface LocalSetup {
  business: Business | null;
  keywords: string[];
  grid: number;
  radius_km: number;
  schedule: LocalSchedule;
  last_run_at: string | null;
  points: GridPoint[];
  zoom: number;
  next_run_at: string | null;
  waiting_reviews: number;
}

export interface GridEstimate {
  points: number;
  checks: number;
  /** A run started by hand: live pages. */
  run: Estimate;
  queued: boolean;
  scheduled_usd: number;
  runs_per_month: number;
  month_usd: number;
  month_credits: number | null;
}

export interface LocalRun {
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
  grid: number;
  radius_km: number;
  lat: number;
  lng: number;
  waiting: number;
}

export interface MapPlace {
  position: number;
  title: string;
  cid: string | null;
  rating: number | null;
  votes: number | null;
}

export interface GridCell extends GridPoint {
  /** The business's place; `null` when not among the first 20. */
  position: number | null;
  top: MapPlace[];
  error: string | null;
}

export interface GridSummary {
  points: number;
  found: number;
  top3: number;
  /** Average rank where the business shows. */
  arp: number | null;
  /** Average rank over every point, 21 where it does not show. */
  atrp: number | null;
  /** Share of local voice, percent. */
  solv: number;
}

export interface KeywordGrid {
  keyword: string;
  summary: GridSummary;
  cells: GridCell[];
}

export interface LocalRival {
  title: string;
  cid: string | null;
  top3: number;
  average: number;
  rating: number | null;
  votes: number | null;
}

export interface LocalTrendPoint {
  run_id: number;
  started_at: string;
  keyword: string;
  arp: number | null;
  atrp: number | null;
  solv: number;
}

export interface LocalGrid {
  run: LocalRun | null;
  keywords: KeywordGrid[];
  summary: GridSummary;
  rivals: LocalRival[];
  trend: LocalTrendPoint[];
  runs: LocalRun[];
}

export interface LocalReview {
  review_id: string;
  rating: number | null;
  text: string | null;
  author: string | null;
  author_url: string | null;
  url: string | null;
  published_at: string | null;
  owner_answer: string | null;
  answered_at: string | null;
  photos: number;
  local_guide: boolean;
  first_seen: string;
}

export interface ReviewSummary {
  count: number;
  average: number | null;
  unanswered: number;
  unanswered_negative: number;
  recent: number;
  /** Reviews by stars, 1 to 5. */
  stars: [number, number, number, number, number];
}

export interface RatingDay {
  day: string;
  rating: number | null;
  votes: number | null;
}

export interface ReviewsView {
  reviews: LocalReview[];
  summary: ReviewSummary;
  ratings: RatingDay[];
  waiting: number;
}

// ----- Content briefs ----------------------------------------------------------------------

/** Low, median and high quartile of what the ranking pages have. */
export interface Spread {
  low: number;
  median: number;
  high: number;
}

/** How the ranking pages' titles are written. */
export interface TitlePattern {
  median_chars: number;
  median_px: number;
  /** Shares, 0–1. */
  keyword_first: number;
  with_year: number;
  with_number: number;
}

/** A term most ranking pages use. */
export interface BriefTerm {
  term: string;
  pages: number;
  /** 0–1. */
  share: number;
  /** Uses on a page using it. */
  average: number;
}

export interface BriefCompetitor {
  position: number;
  url: string;
  domain: string;
  title: string | null;
  words: number;
  h2: number;
  /** Content score against the others; `null` when it could not be read. */
  score: number | null;
  status: number | null;
}

export interface InternalLink {
  url: string;
  title: string;
  /** 0–1. */
  relevance: number;
}

/** What to write for a keyword, from the pages ranking for it. */
export interface ContentBrief {
  keyword: string;
  intent: string;
  words: Spread | null;
  h2: Spread | null;
  title: TitlePattern;
  headings: string[];
  outline: string[];
  questions: string[];
  ideas: string[];
  schema: string[];
  short_answer: boolean;
  terms: BriefTerm[];
  competitors: BriefCompetitor[];
  internal_links: InternalLink[];
}

export type BriefStatus = "running" | "done" | "failed" | "interrupted";

/** A brief in the list. */
export interface BriefSummary {
  id: number;
  keyword: string;
  country: string;
  language: string;
  mobile: boolean;
  status: BriefStatus;
  error: string | null;
  created_at: string;
  updated_at: string;
  words: number | null;
  pages: number;
  written: boolean;
  cost_usd: number;
}

/** A part of a score. */
export interface ScorePart {
  id: string;
  weight: number;
  value: number | null;
  /** 0–1; `null` when it could not be measured. */
  normalized: number | null;
  contribution: number;
  reason: string;
  params: Record<string, unknown>;
}

export interface ScoreBreakdown {
  score: number;
  model: string;
  confidence: number;
  components: ScorePart[];
}

/** A brief term and how often a page uses it. */
export interface TermUse {
  term: string;
  count: number;
  average: number;
  share: number;
}

/** How a page or draft measures up to a brief. */
export interface ContentScore {
  breakdown: ScoreBreakdown;
  words: number;
  h2: number;
  title: string | null;
  title_chars: number;
  description_chars: number;
  keyword_in_title: boolean;
  readability: number | null;
  terms: TermUse[];
  missing: TermUse[];
  unanswered: string[];
  status: number | null;
}

/** A page or draft scored and kept. */
export interface ContentScoreRow {
  id: number;
  brief_id: number;
  project_id: number;
  kind: "url" | "draft";
  url: string | null;
  title: string | null;
  description: string | null;
  body: string | null;
  score: number;
  result: ContentScore;
  suggestions: string | null;
  scored_at: string;
}

/** A brief with the pages and drafts scored against it. */
export interface BriefView {
  id: number;
  project_id: number;
  keyword: string;
  country: string;
  language: string;
  mobile: boolean;
  status: BriefStatus;
  brief: ContentBrief | null;
  ai_text: string | null;
  ai_model: string | null;
  cost_usd: number;
  error: string | null;
  created_at: string;
  updated_at: string;
  scores: ContentScoreRow[];
}

/** A page or draft scored now. */
export type ScoredContent = ContentScore & { id: number | null };

/** A page of the project worth checking. */
export interface CheckerRow {
  url: string;
  keyword: string;
  source: "rank" | "gsc";
  position: number | null;
  clicks: number | null;
  volume: number | null;
  score: number | null;
  scored_for: string | null;
  scored_at: string | null;
}

export type WriterEngine = "claude" | "chatgpt" | "gemini";

/** An assistant that can write with the user's own key. */
export interface Writer {
  engine: WriterEngine;
  model: string;
  ready: boolean;
  usd: number;
}

export interface WritingEstimate {
  writers: Writer[];
  material_chars: number;
}

/** A reply to a review an assistant drafted. */
export interface DraftedReply {
  text: string;
  model: string;
  cost_usd: number;
}

// ----- Site audit: the crawl's links, pages and hreflang; Core Web Vitals ----------------------

export interface LinkedPage {
  url: string;
  title: string;
  depth: number | null;
  inlinks: number;
  /** Internal PageRank, 0–100. */
  rank: number;
  indexability: string;
}

export interface OrphanPage {
  url: string;
  status: number | null;
  indexability: string;
}

export interface LinksReport {
  audit_id: number;
  pages: number;
  /** [clicks from the start page (6 = six or more), pages]. */
  depth: [number, number][];
  average_depth: number | null;
  deep: number;
  sitemaps: boolean;
  orphan_total: number;
  orphans: OrphanPage[];
  weak_total: number;
  weak: LinkedPage[];
  top: LinkedPage[];
}

export interface HreflangReport {
  audit_id: number;
  pages: number;
  languages: [string, number][];
  issues: { id: string; title: string; priority: string; count: number }[];
  total: number;
  columns: string[];
  rows: string[][];
}

export interface PagesView {
  audit_id: number;
  total: number;
  offset: number;
  columns: string[];
  rows: string[][];
}

export interface PagesAsk {
  offset?: number;
  limit?: number;
  sort?: string;
  descending?: boolean;
  search?: string;
  status_class?: number;
  indexability?: string;
}

export interface CrawledPageDetail {
  url: string;
  facts: [string, string][];
  findings: { id: string; title: string; priority: string; kind: string }[];
  inlinks_total: number;
  inlinks: { url: string; anchor: string }[];
}

export interface FindingsFile {
  name: string;
  csv: string;
  rows: number;
  cut: boolean;
}

/** A Core Web Vital: its 75th percentile and how visits were spread (shares 0–1). */
export interface VitalMetric {
  p75: number | null;
  good: number | null;
  needs: number | null;
  poor: number | null;
}

export type VitalName = "lcp" | "inp" | "cls" | "fcp" | "ttfb";

export interface FieldData {
  lcp: VitalMetric | null;
  inp: VitalMetric | null;
  cls: VitalMetric | null;
  fcp: VitalMetric | null;
  ttfb: VitalMetric | null;
  first_date: string | null;
  last_date: string | null;
  origin_fallback: boolean;
}

export interface FieldHistory {
  dates: string[];
  lcp: (number | null)[];
  inp: (number | null)[];
  cls: (number | null)[];
  fcp: (number | null)[];
  ttfb: (number | null)[];
}

export interface LabReport {
  url: string;
  score: number | null;
  lcp_ms: number | null;
  cls: number | null;
  tbt_ms: number | null;
  fcp_ms: number | null;
  speed_index_ms: number | null;
  ttfb_ms: number | null;
  opportunities: { id: string; title: string; savings_ms: number }[];
  field: FieldData | null;
}

export type VitalDevice = "phone" | "desktop";

export interface OriginVitals {
  origin: string;
  device: VitalDevice;
  day: string;
  field: FieldData | null;
  history: FieldHistory | null;
  passes: boolean | null;
}

export interface PageVitals {
  url: string;
  device: VitalDevice;
  day: string;
  lab: LabReport | null;
  field: FieldData | null;
  passes: boolean | null;
}

export interface VitalsReport {
  origins: OriginVitals[];
  pages: PageVitals[];
  /** [day, url, device, PageSpeed score]. */
  scores: [string, string, VitalDevice, number][];
}

export interface VitalsSetup {
  key_ready: boolean;
  pages: string[];
  last_checked: string | null;
}

// ----- Opportunities and the project dashboard ---------------------------------------------

export type OpportunityRule =
  | "near_top"
  | "dropped"
  | "low_ctr"
  | "losing_page"
  | "cannibal"
  | "audit_issue"
  | "orphans"
  | "weak_links"
  | "deep_pages"
  | "vitals"
  | "slow_page"
  | "ai_gap"
  | "low_content"
  | "local_weak"
  | "rival_growth"
  | "lost_links";

export type OpportunityState = "open" | "done" | "ignored" | "snoozed";
export type Effort = "low" | "medium" | "high";

export interface Opportunity {
  key: string;
  rule: OpportunityRule;
  /** The module page that shows it. */
  page: PageId;
  /** Its tab and what it shows (`keyword`, `url`, `domain`). */
  params: Record<string, string>;
  subject: string;
  detail: Record<string, string | number | boolean | null>;
  /** Estimated extra clicks a month. */
  gain: number | null;
  impact: number;
  effort: Effort;
  priority: number;
  state: OpportunityState;
  until: string | null;
  returned: boolean;
}

export interface OpportunitySources {
  rank: boolean;
  gsc: boolean;
  audit: boolean;
  vitals: boolean;
  ai: boolean;
  content: boolean;
  local: boolean;
  snapshots: boolean;
}

export interface OpportunityList {
  items: Opportunity[];
  open: number;
  hidden: Partial<Record<OpportunityState, number>>;
  sources: OpportunitySources;
  gain: number;
}

export interface DatedValue {
  date: string;
  value: number | null;
}

export interface ProjectDashboard {
  project: ProjectRow;
  organic: { metrics: { keywords: number; traffic: number; traffic_value_usd: number; pos_1: number; pos_2_3: number; pos_4_10: number }; fetched_at: string; traffic: DatedValue[]; keywords: DatedValue[] } | null;
  links: { links: { rank: number | null; backlinks: number; referring_domains: number; broken_backlinks: number }; fetched_at: string; referring_domains: DatedValue[] } | null;
  rank: { visibility: number | null; visibility_change: number | null; average_position: number | null; top3: number; top10: number; ranked: number; checked: number; improved: number; declined: number; lost: number; traffic: number; series: DatedValue[] } | null;
  search: { period: [string, string]; clicks: number; clicks_before: number; impressions: number; impressions_before: number; ctr: number; position: number | null; position_before: number | null; series: DatedValue[] } | null;
  audit: { audit_id: number; health: number | null; health_before: number | null; geo: number | null; issues: number; issues_before: number | null; pages: number; finished_at: string | null; series: DatedValue[] } | null;
  ai: { answers: number; mention_rate: number; citation_rate: number; share_of_voice: number; series: DatedValue[] } | null;
  local: { arp: number | null; solv: number; top3: number; points: number; series: DatedValue[] } | null;
  vitals: { devices: { device: VitalDevice; passes: boolean | null; lcp: number | null; inp: number | null; cls: number | null; day: string }[] } | null;
  content: { briefs: number; scored_pages: number; average_score: number | null };
  opportunities: Opportunity[];
  opportunities_open: number;
  opportunities_gain: number;
  schedule: { module: PageId; at: string }[];
  spend: [string, number][];
  spend_total: number;
}

export type AlertKind = "rank_drop" | "visibility_drop" | "gsc_drop" | "audit_health" | "audit_issues" | "ai_drop" | "ai_gain" | "local_drop" | "review" | "rival_growth" | "links";

export type AlertSeverity = "bad" | "good" | "info";

/** A kept alert with its project's name. */
export interface AlertRow {
  id: number;
  project_id: number;
  /** What it is about; kept once. */
  key: string;
  kind: AlertKind;
  severity: AlertSeverity;
  /** The module page that shows it. */
  page: PageId;
  params: Record<string, string>;
  /** Numbers and names its text uses. */
  detail: Record<string, string | number>;
  created_at: string;
  read_at: string | null;
  project: string;
}

export interface AlertList {
  alerts: AlertRow[];
  unread: number;
  /** Unread alerts by project id. */
  by_project: Record<string, number>;
}

export interface AlertRules {
  /** Places a tracked keyword must fall; 0 turns it off. */
  rank_drop: number;
  top10_exit: boolean;
  /** Visibility falling by this many percent; 0 turns it off. */
  visibility_drop: number;
  gsc_drop: number;
  health_drop: number;
  audit_issues: boolean;
  ai_change: number;
  local_drop: number;
  reviews: boolean;
  rivals: boolean;
  /** Won and lost linking sites, checked weekly (paid). */
  links: boolean;
}

export type AlertEmail = "off" | "instant" | "daily";

export interface AlertSettings {
  enabled: boolean;
  rules: AlertRules;
  email: AlertEmail;
  recipients: string[];
  /** The project's language when empty. */
  lang: string;
  last_digest_at?: string | null;
  links_last_run?: string | null;
}

export interface AlertSetup {
  settings: AlertSettings;
  /** E-mails can be sent from here (the web, with mail set up). */
  can_email: boolean;
  /** System notifications are shown here (the desktop). */
  can_notify: boolean;
  /** This copy looks for alerts (a syncing desktop leaves it to the web). */
  evaluates_here: boolean;
  /** What the weekly link check costs, list price. */
  links_usd: number;
  links_estimate: Estimate | null;
}
