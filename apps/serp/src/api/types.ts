// Shapes of the desktop commands, as the Rust side serializes them.

export type Engine = "google" | "bing" | "yandex";
export const engines: Engine[] = ["google", "bing", "yandex"];
export type Device = "desktop" | "mobile";
export type MatchMode = "domain" | "host" | "prefix";

export interface Target {
  value: string;
  mode: MatchMode;
}
export interface Location {
  name?: string;
  google_canonical?: string;
  yandex_lr?: number;
  provider_code?: string;
}
export interface SerpQuery {
  engine: Engine;
  keyword: string;
  country: string;
  language: string;
  location?: Location;
  device?: Device;
  depth: number;
}
export interface SearchRequest {
  query: SerpQuery;
  providers?: string[];
  target?: Target;
}
export interface PageCrawlOptions {
  javascript?: boolean;
  identity?: "octopus" | "chrome";
}
export interface AnalyzeRequest {
  search: SearchRequest;
  pages?: number;
  crawl?: PageCrawlOptions;
  target_url?: string;
  volume?: boolean;
  backlinks?: boolean;
}

export interface Sitelink {
  title: string;
  url: string;
}
export interface Organic {
  position: number;
  absolute_position?: number;
  page: number;
  url: string;
  host: string;
  domain: string;
  title: string;
  snippet?: string;
  displayed_url?: string;
  date?: string;
  sitelinks?: Sitelink[];
  /** The engine hid the page address; `url` is the site's home address. */
  site_only?: boolean;
}
export type FeatureKind =
  | "ads-top"
  | "ads-bottom"
  | "featured-snippet"
  | "people-also-ask"
  | "local-pack"
  | "videos"
  | "images"
  | "ai-overview"
  | "related-searches"
  | "top-stories"
  | "knowledge-panel"
  | "shopping"
  | "other";
export interface FeatureItem {
  text?: string;
  url?: string;
  domain?: string;
}
export interface Feature {
  kind: FeatureKind;
  absolute_position?: number;
  page: number;
  items?: FeatureItem[];
}
export interface SerpSnapshot {
  query: SerpQuery;
  provider: string;
  provider_rev?: string;
  parser_rev?: string;
  fetched_at: string;
  total_results?: number;
  organic: Organic[];
  features: Feature[];
  warnings?: string[];
}

export interface ScoreComponent {
  id: string;
  weight: number;
  value?: number | null;
  normalized?: number | null;
  contribution: number;
  reason: string;
  params?: Record<string, unknown>;
}
export interface Breakdown {
  score: number;
  model: string;
  confidence: number;
  components: ScoreComponent[];
}
export type Band = "very-easy" | "easy" | "medium" | "hard" | "very-hard" | "extreme";
export type IntentClass =
  | "informational"
  | "navigational"
  | "commercial"
  | "transactional"
  | "local";
export interface Intent {
  primary: IntentClass;
  secondary?: IntentClass | null;
  scores: Partial<Record<IntentClass, number>>;
  signals: string[];
  model: string;
}
export interface PageScore {
  position: number;
  url: string;
  domain: string;
  content?: Breakdown | null;
  technical?: Breakdown | null;
  site?: Breakdown | null;
  strength?: number | null;
  words: number;
}
export interface Range {
  low: number;
  median: number;
  high: number;
}
export interface TitlePattern {
  median_chars: number;
  median_px: number;
  keyword_first: number;
  with_year: number;
  with_number: number;
}
export interface Recommendations {
  words?: Range | null;
  h2?: Range | null;
  headings: string[];
  schema: string[];
  title: TitlePattern;
  questions: string[];
  short_answer: boolean;
  terms: string[];
  ideas: string[];
}
export interface TargetScore {
  domain: string;
  position?: number | null;
  page?: PageScore | null;
  opportunity: Breakdown;
  gaps: string[];
}
export interface KeywordAnalysis {
  models: Record<string, string>;
  difficulty: Breakdown;
  band: Band;
  intent: Intent;
  pages: PageScore[];
  recommendations: Recommendations;
  target?: TargetScore | null;
  volume?: number | null;
}
export interface DomainFacts {
  domain: string;
  created?: string;
  age_source?: "rdap" | "whois" | "wayback";
  age_years?: number;
  open_page_rank?: number;
  page_rank_position?: number;
  referring_domains?: number;
  referring_source?: string;
  backlinks?: number;
  tranco_rank?: number;
  errors?: Record<string, string>;
}
export interface AnalysisView {
  id: number;
  analysis: KeywordAnalysis;
  snapshot: SerpSnapshot;
  domains: DomainFacts[];
}
export interface AnalysisSummary {
  id: number;
  snapshot_id: number;
  keyword: string;
  engine: Engine;
  country: string;
  created_at: string;
  target_domain?: string | null;
  difficulty: number;
  band: Band;
  intent: IntentClass;
  opportunity?: number | null;
  volume?: number | null;
}

export type JobKind = "analysis" | "search" | "track" | "account";
export type JobStatus = "running" | "challenge" | "done" | "failed" | "cancelled";
export type ChallengeKind = "consent" | "captcha" | "blocked";
export interface Attempt {
  provider: string;
  outcome: "success" | "skipped" | { failed: string };
  message?: string;
}
export interface Job {
  id: string;
  kind: JobKind;
  label: string;
  engine?: Engine;
  project_id?: number;
  status: JobStatus;
  step: string;
  done: number;
  total: number;
  detail?: string;
  challenge?: { provider: string; kind: ChallengeKind; url: string };
  result?: number;
  error?: string;
  attempts?: Attempt[];
  diagnostic?: string;
  started_at: number;
  revision: number;
}

export interface CostEstimate {
  requests: number;
  usd: number;
}
export interface ProviderEstimate {
  provider: string;
  ready: boolean;
  reason?: string;
  estimate: CostEstimate;
}

export type SecretState = "unset" | "saved" | "rebind" | "environment";
export interface SecretInfo {
  name: string;
  label?: string;
  state: SecretState;
  variable: string;
}
export interface ProviderInfo {
  id: string;
  name: string;
  kind: "builtin" | "custom" | "browser";
  engines: Engine[];
  devices: Device[];
  secrets: SecretInfo[];
  origins: string[];
  ready: boolean;
  enabled: boolean;
  per_request_usd: number;
  budget_usd?: number;
  notes?: string;
  error?: string;
  /** How it is paid now, when usable. */
  via: Via | null;
  mode: SourceMode;
  credit: CreditOffer | null;
}
/** How a paid source is paid: the user's choice and what is used now. */
export type SourceMode = "auto" | "own" | "credit";
export type Via = "own" | "credit";
export interface CreditOffer {
  connector: string;
  /** Credit price = provider price × this. */
  price_multiplier: number;
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
export interface ProviderUsage {
  provider: string;
  month: string;
  requests: number;
  successes: number;
  failures: number;
  cost_usd: number;
  /** The part paid with OctopusOn SEO credits. */
  credit_usd: number;
}

export interface Settings {
  defaults: { country: string; language: string; device: Device; depth: number };
  chains: Partial<Record<Engine, string[]>>;
  disabled?: string[];
  budgets?: Record<string, number>;
  confirm_above_usd: number;
  proxy?: string;
  timeout_secs: number;
  browser: { delay_secs: number; daily_cap: number; executable?: string };
}

export interface Dataset {
  name: string;
  version?: string | null;
  imported_at: string;
  rows: number;
}
export interface AppInfo {
  version: string;
  data_dir: string;
  secret_store: "keychain" | "file";
  open_pagerank: SecretState;
  tranco?: Dataset | null;
  tranco_notice: string;
}

export interface SnapshotSummary {
  id: number;
  run_id?: number | null;
  keyword: string;
  engine: Engine;
  country: string;
  language: string;
  device: Device;
  depth: number;
  provider: string;
  fetched_at: string;
  organic_count: number;
  total_results?: number | null;
  cost_usd: number;
}

export type Schedule = "daily" | "weekly" | "manual";
export interface TrackerSpec {
  engine: Engine;
  country: string;
  language: string;
  device?: Device;
  depth: number;
  location?: Location;
}
export interface TrackerRow extends TrackerSpec {
  id: number;
  project_id: number;
}
export interface ProjectSpec {
  name: string;
  target: Target;
  schedule?: Schedule;
  trackers: TrackerSpec[];
  providers?: string[];
}
export interface Project {
  id: number;
  name: string;
  target: Target;
  schedule: Schedule;
  providers?: string[];
  trackers: TrackerRow[];
  keywords: number;
  competitors: string[];
  created_at: string;
  last_run_at?: string;
}
export interface KeywordRow {
  id: number;
  project_id: number;
  keyword: string;
  tags: string[];
  created_at: string;
}
export interface TrackEstimate {
  searches: number;
  requests: number;
  usd: number;
}
export interface RankRow {
  keyword_id: number;
  keyword: string;
  tags: string[];
  tracker_id: number;
  position?: number | null;
  previous?: number | null;
  change?: number | null;
  best?: number | null;
  url?: string | null;
  cannibalised: boolean;
  checked_at?: string | null;
  history: [string, number | null][];
  volume?: number | null;
  error?: string | null;
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
  average_position?: number | null;
  own: boolean;
}
export interface RankReport {
  project: Project;
  rows: RankRow[];
  visibility: VisibilityPoint[];
  competitors: CompetitorShare[];
}

export interface Coverage {
  matched: number;
  usable: number;
}
export interface ParsedPage {
  page: number;
  organic: Organic[];
  features: Feature[];
  total_results?: number | null;
  error?: string | null;
  challenge?: { kind: ChallengeKind; reason: string } | null;
  coverage: Coverage;
}
export interface ParserInfo {
  engine: Engine;
  key: string;
  patched: boolean;
  stale_base?: string;
  error?: string;
  mapping: unknown;
  base: string;
  patch?: unknown;
}
export interface ProfileOverride {
  engine: Engine;
  base?: string;
  mapping: unknown;
  enabled: boolean;
}
/** A provider spec, edited as JSON; the backend validates it. */
export type ProviderSpec = { id: string; name: string; engines?: Engine[] } & Record<string, unknown>;
export interface StoredPage {
  page: number;
  url: string;
  status: number;
  content_type?: string;
  size: number;
  body: string;
}
