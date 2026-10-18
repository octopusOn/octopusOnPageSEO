export type IndexEngine =
  | "google"
  | "yandex"
  | "bing"
  | "duckduckgo"
  | "wayback"
  | "commoncrawl"
  | "sitemap"
  | "google_api"
  | "yandex_api"
  | "serper_api"
  | "serpapi"
  | "seed_list";

export type FindingCategory =
  | "error_404"
  | "error_4xx"
  | "error_5xx"
  | "soft_404"
  | "thin_200"
  | "network_error"
  | "healthy_200";

export type RedirectServerFormat =
  | "nginx"
  | "nginx_cluster"
  | "apache"
  | "apache_410"
  | "cloudflare"
  | "cloudflare_csv"
  | "vercel"
  | "nextjs"
  | "caddy"
  | "iis"
  | "nginx_410"
  | "wordpress_csv"
  | "shopify_csv"
  | "removal_list"
  | "clean_sitemap"
  | "redirect_chains_csv"
  | "indexnow_json"
  | "markdown_report";

export interface ApiConfig {
  google_api_key: string;
  google_cx: string;
  yandex_api_key: string;
  yandex_user: string;
  serper_api_key: string;
  serpapi_key: string;
}

export interface ScanConfig {
  domain: string;
  include_subdomains: boolean;
  engines: IndexEngine[];
  use_wayback: boolean;
  use_commoncrawl: boolean;
  use_sitemaps: boolean;
  expand_subpaths: boolean;
  pages_per_query: number;
  seed_queries: string[];
  extra_urls: string[];
  api: ApiConfig;
  detect_all_4xx: boolean;
  detect_5xx: boolean;
  detect_thin_200: boolean;
  min_word_count: number;
  concurrency: number;
  timeout_seconds: number;
  max_redirects: number;
  user_agent: string;
}

export interface VerifiedUrl {
  url: string;
  engines: string;
  serp_title: string;
  serp_snippet: string;
  found_by_query: string;
  initial_status: number;
  final_status: number;
  redirect_hops: number;
  final_url: string;
  response_time_ms: number;
  content_type: string;
  live_title: string;
  h1: string;
  word_count: number;
  canonical: string;
  noindex: boolean;
  category: FindingCategory;
  reason: string;
  suggested_target: string;
  error_message: string;
}

export type ScanPhase =
  | "starting"
  | "discovering"
  | "verifying"
  | "saving"
  | "completed"
  | "stopped"
  | "failed";

export interface ScanProgress {
  queries_completed: number;
  queries_total: number;
  discovered_urls: number;
  verified_urls: number;
  found_404: number;
  found_4xx: number;
  found_5xx: number;
  found_thin_soft404: number;
  current_item: string;
  live_rows?: VerifiedUrl[];
  /** Search API requests paid with OctopusOn SEO credits so far. */
  credits?: CreditUsage;
}

/** Search API requests of a scan paid with OctopusOn SEO credits. */
export interface CreditUsage {
  /** Sources that ran on credits (`serper`, `serpapi`). */
  sources: string[];
  /** Requests the OctopusOn SEO gateway answered. */
  requests: number;
  /** What the gateway charged, in micro-dollars. */
  charged_micros: number;
  /** Why a source stopped running on credits during the scan. */
  stopped: string | null;
}

export interface ScanMeta {
  id: string;
  domain: string;
  created_at_ms: number;
  finished_at_ms: number;
  status: string;
  config: ScanConfig;
  queries_executed: number;
  discovered_urls: number;
  verified_urls: number;
  count_404: number;
  count_4xx: number;
  count_5xx: number;
  count_soft_404: number;
  count_thin_200: number;
  count_network_error: number;
  count_healthy: number;
  /** Search API requests paid with OctopusOn SEO credits (older scans have none). */
  credits?: CreditUsage;
}

export interface EngineSummary {
  engine: string;
  total_urls: number;
  issue_urls: number;
}

export interface PathCluster {
  prefix: string;
  total_urls: number;
  issue_urls: number;
  count_404: number;
  count_soft_thin: number;
  count_4xx_5xx: number;
  avg_word_count: number;
  avg_response_ms: number;
  action_hint: string;
}

export interface ScanOverview {
  meta: ScanMeta;
  health_score: number;
  engines: EngineSummary[];
  clusters: PathCluster[];
  top_issues: VerifiedUrl[];
}

export interface FinderPageRequest {
  category: string;
  engine: string;
  search: string;
  max_words?: number | null;
  sort_by?: string;
  offset: number;
  limit: number;
}

export interface FinderTablePage {
  total: number;
  rows: VerifiedUrl[];
}

export interface FinderJob {
  id: string;
  domain: string;
  status: string;
  phase: ScanPhase;
  progress: ScanProgress;
  scan_id: string | null;
  meta: ScanMeta | null;
  error: string | null;
}

export type DiffStatus = "resolved" | "new_issue" | "persistent_issue";

export interface ComparedUrl {
  url: string;
  engines: string;
  diff: DiffStatus;
  base_status: number;
  target_status: number;
  base_category: FindingCategory;
  target_category: FindingCategory;
  suggested_target: string;
}

export interface ScanComparison {
  base_scan_id: string;
  target_scan_id: string;
  domain: string;
  resolved_count: number;
  new_issue_count: number;
  persistent_issue_count: number;
  rows: ComparedUrl[];
}

export interface RedirectCandidate {
  target_url: string;
  title: string;
  similarity: number;
  matched_tokens: string[];
  kind: string;
}

export type Page = "overview" | "explorer" | "engines" | "sessions" | "settings";
export type Lang = "tr" | "en";

/** How a paid source is paid: the user's choice and what is used now. */
export type SourceMode = "auto" | "own" | "credit";
export type Via = "own" | "credit";

export interface CreditOffer {
  connector: string;
  /** Credit price = provider price × this. */
  price_multiplier: number;
}

/** A paid search API and how the next scan pays for it. */
export interface CreditSource {
  /** `serper`, `serpapi`, `google-cse` or `yandex-xml`. */
  id: string;
  name: string;
  engine: IndexEngine;
  /** It can run on OctopusOn SEO credits at all. */
  creditable: boolean;
  mode: SourceMode;
  own_key: boolean;
  /** How the next scan pays for it; null when it does not run. */
  via: Via | null;
  credit: CreditOffer | null;
}

export interface Account {
  email: string | null;
  name: string | null;
  balance_micros: number;
  held_micros: number;
  available_micros: number;
  credit_unit_micros: number;
  low_balance: boolean;
  portal_url: string;
}

/** The OctopusOn SEO account (credits). */
export interface CloudStatus {
  signed_in: boolean;
  account: Account | null;
  error: string | null;
  portal_url: string;
  api: string;
}

/** A one-click sign-in to the OctopusOn SEO account, finished in the browser. */
export interface AccountLogin {
  status: "waiting" | "approved" | "cancelled" | "failed";
  error: string | null;
}
