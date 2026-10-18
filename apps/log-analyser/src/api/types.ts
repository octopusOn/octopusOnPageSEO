// Types mirroring the serialized Rust DTOs of the Tauri commands.

export type ColumnKind = "text" | "integer" | "float" | "boolean" | "date" | "timestamp";

export interface Column {
  name: string;
  kind: ColumnKind;
}

/** A result cell: dates and timestamps arrive as ISO strings. */
export type Cell = string | number | boolean | null;

export interface Table {
  columns: Column[];
  rows: Cell[][];
}

export interface CommandError {
  kind: "syntax" | "invalid-query" | "not-found" | "duplicate" | "busy" | "cancelled" | "invalid-input" | "error";
  message: string;
  span?: [number, number];
  query?: string;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  memoryLimitMb: number;
}

export interface ClientInfo {
  id: number;
  name: string;
  created_at: string;
}

export interface SiteInfo {
  id: number;
  client: string;
  name: string;
  format: string;
  created_at: string;
}

export interface ImportSummary {
  id: number;
  path: string;
  size_bytes: number;
  format: string;
  finished_at: string;
  elapsed_ms: number;
  total_lines: number;
  parsed_lines: number;
  inserted_rows: number;
  duplicate_rows: number;
  first_request: string | null;
  last_request: string | null;
}

export interface SiteOverview extends SiteInfo {
  imports: number;
  hits: number;
  urls: number;
  user_agents: number;
  first_request: string | null;
  last_request: string | null;
  history: ImportSummary[];
}

export interface FormatInfo {
  id: string;
  name: string;
}

export type FormatInput =
  | { kind: "auto" }
  | { kind: "builtin"; id: string }
  | { kind: "nginx"; format: string }
  | { kind: "apache"; format: string };

export interface ErrorSample {
  stream: string;
  line_number: number;
  reason: string;
  excerpt: string;
}

export interface ParseSummary {
  streams: { name: string; lines: number; parsed: number }[];
  total_lines: number;
  parsed_lines: number;
  /** Parsed lines not stored because of the site's import settings. */
  filtered_lines: number;
  skipped_by_reason: Record<string, number>;
  error_samples: ErrorSample[];
  first_request: string | null;
  last_request: string | null;
  status_classes: Record<string, number>;
}

export interface Throughput {
  elapsed_ms: number;
  lines_per_second: number;
}

export interface InspectReport extends ParseSummary, Throughput {
  format_id: string;
  format_name: string;
  detection_confidence: number | null;
}

export type FileOutcome =
  | { status: "already_imported" }
  | ({ status: "unrecognized" } & ParseSummary)
  | ({ status: "imported"; import_id: number; inserted_rows: number; duplicate_rows: number } & ParseSummary);

export type FileImportReport = { path: string } & FileOutcome;

export type TrafficScope = "all" | "bots";

export interface SiteSettings {
  host: string | null;
  traffic: TrafficScope;
  bot_categories: string[];
  include_urls: string[];
  exclude_urls: string[];
  strip_parameters: string[];
  strip_tracking_parameters: boolean;
}

export interface ImportReport extends Throughput {
  site: string;
  format_id: string;
  files: FileImportReport[];
  inserted_rows: number;
  duplicate_rows: number;
  total_lines: number;
  dns_verification: boolean;
}

export type ImportEvent =
  | { event: "fileStarted"; path: string; index: number; total: number }
  | { event: "progress"; lines: number; bytes: number }
  | { event: "fileFinished"; report: FileImportReport };

export type QuerySpec =
  | { kind: "text"; text: string }
  | { kind: "template"; id: string; refine?: string | null }
  | { kind: "saved"; name: string; refine?: string | null };

export interface SavedQueryInfo {
  name: string;
  description: string;
  query: string;
  created_at: string;
  updated_at: string;
}

export interface SavedImportReport {
  added: string[];
  replaced: string[];
  skipped: { name: string; reason: string }[];
}

export interface TemplateInfo {
  id: string;
  title: string;
  description: string;
  query: string;
}

export interface CheckedQuery {
  query: string;
  source: string;
  columns: Column[];
}

export interface QueryOutcome {
  site: string;
  query: string;
  source: string;
  columns: Column[];
  rows: number;
  elapsed_ms: number;
}

/** `run_query` result: one page of rows. */
export interface QueryPageResult {
  outcome: QueryOutcome;
  rows: Cell[][];
  hasMore: boolean;
}

export interface ExportReport extends QueryOutcome {
  path: string;
  format: string;
}

export interface BotInfo {
  id: string;
  name: string;
  operator: string;
  category: string;
  verification: "ip-list" | "dns" | "ip-list+dns" | "none";
  /** Defined by the user. */
  custom: boolean;
}

export interface ArchiveReport {
  site: string;
  path: string;
  files: number;
  bytes: number;
}

export interface GeoRelease {
  country: string;
  asn: string;
  fetched_at: string;
}

export interface GeoStatus {
  release: GeoRelease | null;
  attribution: string;
}

export interface GeoUpdateReport {
  release: GeoRelease;
  country_ranges: number;
  asn_ranges: number;
}

export interface CustomBot {
  id: string;
  name: string;
  operator: string;
  category: string;
  patterns: string[];
  ip_ranges: string[];
  ip_list_url: string | null;
  dns_suffixes: string[];
}

export interface RangeSourceStatus {
  id: string;
  operator: string;
  url: string;
  prefixes: number;
  fetched_at: string | null;
  age_days: number | null;
  origin: "downloaded" | "bundled" | "missing";
}

export interface RangeUpdateReport {
  path: string;
  updated: string[];
  failed: { id: string; error: string }[];
}

export interface VerifySiteReport {
  site: string;
  dns_used: boolean;
  dns_unavailable: boolean;
  reclassified_user_agents: number;
  checked: number;
  changed: number;
  rebuilt_bots: string[];
  verdicts: Record<string, number>;
}

export interface IpCheck {
  ip: string;
  bot: BotInfo;
  verdict: "verified" | "spoofed" | "unverified";
  method: string;
}

export interface Share {
  name: string;
  value: number;
  ratio: number;
}

export interface BotHits {
  bot: string;
  verified: number;
  spoofed: number;
  unverified: number;
}

export interface Anomaly {
  series: "googlebot" | "search" | "ai" | "all";
  day: string;
  kind: "spike" | "drop" | "server-errors";
  value: number;
  baseline: number;
}

export interface CrawlOverview {
  site: string;
  period: string | null;
  alerts: Anomaly[];
  total_hits: number;
  categories: Share[];
  bots: BotHits[];
  spoofed_hits: number;
  search: {
    hits: number;
    unique_urls: number;
    hits_per_day: number;
    unique_urls_per_day: number;
    errors: number;
    avg_bytes: number | null;
    avg_ms: number | null;
    status_classes: Share[];
    non_success_ratio: number;
    parameter_ratio: number;
    asset_ratio: number;
  };
}

export type QueryMode = "keep" | "drop-tracking" | "drop-all";

export interface NormalizeOptions {
  query: QueryMode;
  ignore_trailing_slash: boolean;
  ignore_case: boolean;
}

export interface UrlListReport {
  site: string;
  filter: string;
  sources: { source: string; urls: number }[];
  skipped: number;
  summary: { listed: number; logged: number; both: number; not_in_logs: number; not_in_list: number };
  rows: number;
}

export interface PeriodReport {
  site: string;
  group: string;
  summary: { before_hits: number; after_hits: number; new: number; lost: number; up: number; down: number };
  rows: number;
}

export interface RobotsBotSummary {
  bot: string;
  name: string;
  category: string;
  group: string | null;
  blocked: boolean;
  user_triggered: boolean;
  hits: number;
  disallowed_hits: number;
  verified_disallowed_hits: number;
  disallowed_urls: number;
}

export interface RobotsReport {
  site: string;
  robots: string;
  filter: string;
  sitemaps: string[];
  bots: RobotsBotSummary[];
  rows: number;
}

export interface AnalysisTable<R> extends Table {
  report: R;
}

// Query syntax tree as serialized by `octopuson_query::Query`.

export type AstLiteral = { text: string } | { integer: number } | { date: string } | { boolean: boolean };

export type AstPredicate =
  | { one_of: AstLiteral[] }
  | { glob: string }
  | { regex: string }
  | { compare: ["gt" | "ge" | "lt" | "le", AstLiteral] }
  | { between: [AstLiteral, AstLiteral] }
  | { status_class: number }
  | { last_days: number };

export type AstExpr =
  | { and: AstExpr[] }
  | { or: AstExpr[] }
  | { not: AstExpr }
  | { condition: { field: string; predicate: AstPredicate } };

export interface QueryAst {
  filter: AstExpr | null;
  group: string[];
  metrics: string[];
  sort: { column: string; direction: "asc" | "desc" }[];
  limit: number | null;
}

export type FieldKind = "text" | "integer" | "date" | "boolean";

export interface QuerySchema {
  fields: { name: string; kind: FieldKind }[];
  dimensions: string[];
  metrics: string[];
}

export interface ValueSuggestion {
  value: string;
  hits: number;
}

/** The OctopusOn SEO account every program on this computer shares. */
export interface CloudAccount {
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
  account: CloudAccount | null;
  error: string | null;
  portal_url: string;
  api: string;
}
