export type Cell = string | number | boolean | null;
export type SchemaIssue = {
  severity: "error" | "warning" | "info"; code: string; path: string;
  current: string | null; expected: string; example: string | null; documentation: string;
};
export type SchemaItem = {
  block: number; format: string; path: string; types: string[]; profile: string | null;
  status: "error" | "warning" | "checked" | "unchecked"; issues: SchemaIssue[];
};
export type SchemaReport = {rule_version:string;jsonld_blocks:number;limited:boolean;items:SchemaItem[]};
export type SchemaTestRequest = {mode:"url"|"html"|"jsonld";input:string;rendering:CrawlConfig["rendering"]};
export type SchemaTestResult = {
  url:string|null;status:number|null;rendered:boolean;render_error:string|null;redirects:string[];
  indexability:string|null;truncated:boolean;error:string|null;report:SchemaReport|null;tested_at_ms:number;
};
export type DuplicateKind = "title" | "description" | "h1" | "h2" | "content";
export type DuplicateScope = { kind: DuplicateKind; url: string };
export type DuplicateGroup = { kind: DuplicateKind; value: string | null; value_truncated: boolean; count: number };
export type ContentMatch = {
  url: string;
  match_url: string;
  match_title: string | null;
  distance: number;
  words: number;
  match_words: number;
};
export type TablePage = {
  columns: { name: string; kind: string }[];
  rows: Cell[][];
  total: number;
  link_summary?: { sources: number; destinations: number };
};
export type Progress = {
  crawled: number;
  queued: number;
  errors: number;
  elapsed_ms: number;
  discovered: number;
  in_flight: number;
  sitemaps: number;
  paused: boolean;
};
export type CrawlConfig = {
  start_urls: string[];
  mode: "spider" | "list";
  agent: string;
  robots: string;
  speed: { threads: number; max_requests_per_second: number };
  limits: {
    max_urls: number;
    max_depth: number | null;
    timeout_seconds: number;
  };
  follow: { check_external: boolean; images: boolean; sitemaps: boolean };
  urls: {
    subdomains: boolean;
    start_folder_only?: boolean;
    include: string[];
    exclude: string[];
    strip_params: string[];
  };
  schema_validation?: boolean;
  storage: { headers: boolean; html: boolean };
  rendering: {
    mode: "html" | "javascript";
    wait_ms: number;
    timeout_seconds: number;
    concurrency: number;
    cache_resources?: boolean;
    browser_path: string | null;
    viewport?: Viewport | null;
  };
};
export type CrawlMeta = {
  id: string;
  name: string;
  start_urls: string[];
  created_at_ms: number;
  finished_at_ms: number | null;
  status: "running" | "completed" | "stopped" | "failed";
  progress: Progress;
  limit_reached: boolean;
  error: string | null;
  config: CrawlConfig;
};
export type Viewport = { width: number; height: number; mobile: boolean };
/** Logical file sizes, measured without opening saved databases. Null means unknown. */
export type CrawlStorage = { id: string; bytes: number | null };
export type ActivityKind = "page" | "resource" | "external" | "robots" | "sitemap" | "site_file";
export type UrlActivity = {
  id: number;
  url: string;
  kind: ActivityKind;
  state: "processing" | "completed" | "error" | "blocked" | "cancelled";
  started_ms: number;
  elapsed_ms: number;
  status: number | null;
  title: string | null;
  error: string | null;
  rendered: boolean;
  render_error: string | null;
  response_ms: number | null;
  size_bytes: number | null;
};
export type CrawlActivity = {
  active: UrlActivity[];
  recent: UrlActivity[];
  rendered: number;
  render_errors: number;
  request_errors: number;
  blocked: number;
};
export type Job = {
  id: string;
  url: string;
  rendering?: "html" | "javascript";
  status: "running" | "paused" | "stopping" | "completed" | "stopped" | "failed";
  progress: Progress;
  crawl_id?: string | null;
  phase?: "starting" | "crawling" | "finalizing";
  activity?: CrawlActivity;
  revision?: number;
  meta?: CrawlMeta | null;
  error?: string | null;
};
export type Overview = {
  seo_score?: SeoScore;
  internal: number;
  external: number;
  indexable: number;
  html: number;
  blocked: number;
  errors: number;
  redirects: number;
  responses?: ResponseCount[];
  average_response_ms: number;
  issues: { id: string; count: number }[];
};
export type SeoScore = {
  version: number;
  value: number | null;
  urls: number;
  affected_urls: number;
  analyzed_html: number;
  limited_urls: number;
  unknown_findings: number;
  deductions: { id: string; count: number; points: number }[];
  excluded_urls?: number;
  excluded_links?: number;
  excluded_findings?: number;
  canonical_context_findings?: number;
  anchor_context_findings?: number;
  advisory_findings?: number;
  contact_redirect_findings?: number;
  unverified_anchor_findings?: number;
  excluded_rules?: {id:string;count:number}[];
  schema_validation?: {checked_urls:number;scored_urls:number;error_urls:number;warning_urls:number;missing_urls:number;unverified_urls:number}|null;
  categories?: ScoreCategory[];
};
export type ScoreCategory = {
  id:string; value:number|null; points:number; urls:number; affected_urls:number; checks:number;
  deductions:SeoScore["deductions"];
};
export type ResponseGroup = "success" | "redirects" | "client-error" | "server-error" | "other-http" | "robots-blocked" | "request-error" | "no-response";
export type ResponseCount = { group: ResponseGroup; count: number };
export type PageRequest = {
  tab: string;
  offset: number;
  limit: number;
  search: string;
  status_class: number;
  indexability: string;
  issue_id: string;
  sort: string;
  descending: boolean;
  link_scope?: LinkScope | null;
  link_class?: LinkClass | null;
  duplicate_scope?: DuplicateScope | null;
  response_group?: ResponseGroup | null;
};
export type LinkScope = { url: string; direction: "incoming" | "outgoing" };
export type LinkClass = "internal" | "external";
export type LinkCounts = { incoming: { internal: number; external: number }; outgoing: { internal: number; external: number } };
export type RedirectHop = { url: string; status: number | null; location: string | null; error: string | null; blocked: boolean; recorded: boolean };
export type RedirectTrace = {
  hops: RedirectHop[];
  outcome: "complete" | "http-error" | "request-error" | "blocked" | "unchecked" | "missing-location" | "loop" | "limit";
};
export type UrlDetail = {
  facts: TablePage;
  findings?: TablePage;
  inlinks: TablePage;
  outlinks: TablePage;
  link_counts?: LinkCounts;
  headers: TablePage;
  html: string | null;
  raw_html: string | null;
  schema_report?: SchemaReport|null;
  schema_error?: string|null;
};
export type TableExportRequest = {
  request: PageRequest;
  columns: string[];
};
export const defaultConfig: CrawlConfig = {
  start_urls: [],
  mode: "spider",
  agent: "octopus",
  robots: "respect",
  speed: { threads: 5, max_requests_per_second: 5 },
  limits: { max_urls: 0, max_depth: null, timeout_seconds: 20 },
  follow: { check_external: true, images: true, sitemaps: true },
  urls: {
    subdomains: false,
    include: [],
    exclude: [
      String.raw`^https?://[^/?#]+/cdn-cgi/l/email-protection(?:[?#]|$)`,
      String.raw`^https?://[^/?#]+/cdn-cgi/scripts/[0-9a-fA-F]+/cloudflare-static/email-decode\.min\.js(?:[?#]|$)`,
      String.raw`^https?://[^/?#]+/cdn-cgi/challenge-platform(?:/|[?#]|$)`,
      String.raw`^https?://[^/?#]+/cdn-cgi/(?:trace|rum|script_monitor/report)(?:[?#]|$)`,
    ],
    strip_params: ["utm_source", "utm_medium", "utm_campaign", "gclid"],
  },
  schema_validation: false,
  storage: { headers: true, html: false },
  rendering: {
    mode: "html",
    wait_ms: 1000,
    timeout_seconds: 30,
    concurrency: 2,
    cache_resources: true,
    browser_path: null,
    viewport: null,
  },
};

export type ComparisonFilter = "all" | "changed" | "new" | "missing" | "unchanged";
export type ComparisonRequest = { filter: ComparisonFilter; field: string; search: string; internal_only: boolean; offset: number; limit: number };
export type ComparisonSummary = { new: number; missing: number; changed: number; unchanged: number; fields: { field: string; count: number }[]; warnings: string[] };

export type SitemapReason = "external" | "blocked" | "response" | "non-html" | "noindex" | "rendering" | "truncated" | "canonical" | "non-indexable" | "invalid-url" | "duplicate" | "other-origin";
export type SitemapSelection = { origin:string };
export type SitemapPlan = { origin:string; origins:{origin:string;count:number}[]; scanned:number; included:number; excluded:{reason:SitemapReason;count:number}[]; sample:string[]; parts:number; xml_bytes:number; warnings:string[] };
export type SitemapExport = { urls:number;parts:number };

export type FindingChange = "all"|"introduced"|"not-detected"|"persisting"|"new-url"|"missing-url"|"unverified";
export type FindingComparisonRequest = {change:FindingChange;issue_id:string;priority:string;search:string;internal_only:boolean;offset:number;limit:number};
export type FindingRuleCount = {id:string;priority:string;category:string;introduced:number;not_detected:number;persisting:number;new_url:number;missing_url:number;unverified:number;total:number};
export type FindingComparisonSummary = {counts:FindingRuleCount;baseline:number;current:number;rules:FindingRuleCount[];rule_total:number;warnings:string[]};

export type RemediationRequest = {issue_id:string;search:string;offset:number;limit:number;url:string;evidence_offset:number};
export type RemediationDetail = {urls:TablePage;selected_url:string|null;facts:TablePage;evidence:TablePage;evidence_kind:string;schema_report?:SchemaReport|null};
