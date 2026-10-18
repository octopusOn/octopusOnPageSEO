// Shapes of the desktop commands, as the Rust side serializes them.

export type Engine = "google" | "yandex" | "bing";
export type KeywordStatus = "active" | "excluded";
export type Intent = "informational" | "navigational" | "commercial" | "transactional" | "local";

export interface AppInfo {
  version: string;
  data_dir: string;
  secret_store: string;
}

export interface ProjectSpec {
  name: string;
  country: string;
  language: string;
  engines: Engine[];
  site: string | null;
  competitors: string[];
}

export interface ProjectRow extends ProjectSpec {
  id: number;
  created_at: string;
  keywords: number;
  excluded: number;
  clusters: number | null;
}

export type KeywordSort =
  | "keyword"
  | "volume"
  | "cpc"
  | "competition"
  | "difficulty"
  | "intent"
  | "cluster"
  | "added";

export interface KeywordQuery {
  search?: string | null;
  status?: KeywordStatus | null;
  tag?: string | null;
  source?: string | null;
  intent?: string | null;
  min_volume?: number | null;
  max_volume?: number | null;
  cluster?: number | null;
  sort: KeywordSort;
  descending: boolean;
  offset: number;
  limit: number;
}

export interface MonthlyVolume {
  month: string;
  volume: number;
}

export interface KeywordRow {
  id: number;
  keyword: string;
  status: KeywordStatus;
  tags: string[];
  sources: string[];
  volume: number | null;
  cpc_usd: number | null;
  competition: number | null;
  difficulty: number | null;
  monthly: MonthlyVolume[];
  intent: string | null;
  cluster: number | null;
  pillar: boolean;
  serp: boolean;
}

export interface KeywordPage {
  total: number;
  rows: KeywordRow[];
}

export interface Facets {
  tags: [string, number][];
  sources: [string, number][];
}

export interface AddOutcome {
  received: number;
  added: number;
  known: number;
  rejected: number;
}

export type ColumnRole =
  | "keyword"
  | "volume"
  | "cpc"
  | "competition"
  | "competition-index"
  | "difficulty"
  | "clicks"
  | "impressions"
  | "position"
  | { month: string };

export interface ImportColumn {
  header: string;
  role: ColumnRole | null;
}

export interface ImportPreview {
  columns: ImportColumn[];
  rows: string[][];
  total: number;
}

export interface ImportOutcome extends AddOutcome {
  rows: number;
  metrics: number;
  columns: ImportColumn[];
}

export type DiscoverySource =
  | "wordstat"
  | "ads-ideas"
  | "labs-ideas"
  | "related"
  | "suggestions"
  | "competitors"
  | "autocomplete"
  | "serpapi-autocomplete"
  | "google-ads"
  | "google-suggest"
  | "yandex-suggest";

export type Expansion = "alphabet" | "turkish-letters" | "questions" | "commercial";

export interface FilterSpec {
  include?: string[];
  exclude?: string[];
  regex?: string | null;
  exclude_regex?: string | null;
  min_volume?: number | null;
  max_volume?: number | null;
  max_words?: number | null;
}

export interface DiscoverRequest {
  seeds: string[];
  competitors: string[];
  sources: DiscoverySource[];
  expansions: Expansion[];
  limit: number;
  filter: FilterSpec;
}

export interface SourceEstimate {
  source: DiscoverySource;
  provider: string;
  requests: number;
  cost_usd: number;
  ready: boolean;
  reason: string | null;
}

export interface DiscoverEstimate {
  sources: SourceEstimate[];
  requests: number;
  cost_usd: number;
  wordstat_hours: number;
}

export type Enrichment = "volume" | "difficulty" | "intent";

export interface EnrichRequest {
  asks: Enrichment[];
  volume_source: "dataforseo" | "google-ads";
  only_missing: boolean;
}

export interface EnrichEstimate {
  keywords: [number, number, number];
  requests: number;
  cost_usd: number;
  reason: string | null;
}

export interface SerpRequest {
  engine?: Engine | null;
  device: "desktop" | "mobile";
  top?: number | null;
  per_cluster?: number | null;
  refresh: boolean;
}

export interface SerpEstimate {
  keywords: number;
  cached: number;
  provider: string | null;
  cost_usd: number;
  skipped: [string, string][];
}

export type Method = "lexical" | "serp" | "hybrid";

export interface ClusterRequest {
  method: Method;
  threshold: number;
  serp_threshold: number;
  mode: "soft" | "hard";
  level: "page" | "site";
  engine?: Engine | null;
  device: "desktop" | "mobile";
}

export interface ClusteringRow {
  id: number;
  project_id: number;
  method: Method;
  params: Record<string, unknown>;
  created_at: string;
  keywords: number;
  clusters: number;
}

export type ClusterSort = "volume" | "size" | "difficulty" | "label" | "number";

export interface ClusterQuery {
  search?: string | null;
  intent?: string | null;
  verified?: boolean | null;
  gap?: boolean | null;
  min_size?: number | null;
  sort: ClusterSort;
  descending: boolean;
  offset: number;
  limit: number;
}

export interface Competitor {
  domain: string;
  keywords: number;
}

export interface ClusterRecord {
  number: number;
  pillar_id: number;
  label: string;
  size: number;
  volume: number;
  intent: string | null;
  mixed_intent: boolean;
  difficulty: number | null;
  cpc_usd: number | null;
  verified: boolean;
  target_url: string | null;
  competing: string[];
  gap: boolean;
  competitors: Competitor[];
}

export interface ClusterPage {
  total: number;
  rows: ClusterRecord[];
}

export interface MemberRow {
  keyword_id: number;
  keyword: string;
  score: number;
  shared: number | null;
  volume: number | null;
  cpc_usd: number | null;
  difficulty: number | null;
  intent: string | null;
  pillar: boolean;
  pinned: boolean;
}

export interface SerpRecord {
  keyword_id: number;
  engine: string;
  device: string;
  provider: string;
  fetched_at: string;
  urls: string[];
  site_only: boolean;
  features: string[];
  questions: string[];
  related: string[];
}

export type SecretState = "saved" | "environment" | "unset" | "rebind";

/** How a paid source is paid: the user's choice and what is used now. */
export type SourceMode = "auto" | "own" | "credit";
export type Via = "own" | "credit";

export interface CreditOffer {
  connector: string;
  /** Credit price = provider price × this. */
  price_multiplier: number;
}

export interface ProviderInfo {
  id: string;
  name: string;
  secrets: { name: string; state: SecretState }[];
  ready: boolean;
  via: Via | null;
  mode: SourceMode;
  credit: CreditOffer | null;
  enabled: boolean;
  budget_usd: number | null;
  spent_usd: number;
  offers: string[];
  signup: string;
}

export interface ProviderUsage {
  provider: string;
  month: string;
  requests: number;
  items: number;
  failures: number;
  cost_usd: number;
  /** The part paid with OctopusOn SEO credits. */
  credit_usd: number;
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

export interface GoogleStatus {
  client_id: string | null;
  client_secret: boolean;
  connected: boolean;
  scopes: string[];
  ads_customer_id: string | null;
}

export interface Property {
  site_url: string;
  permission: string;
}

export interface SearchConsoleRequest {
  site_url: string;
  days: number;
  country_only: boolean;
  max_rows: number;
}

export interface RunRow {
  id: number;
  project_id: number | null;
  kind: string;
  status: string;
  planned: number;
  done: number;
  estimated_cost_usd: number;
  cost_usd: number;
  started_at: string;
  finished_at: string | null;
  error: string | null;
}

export interface Settings {
  version: number;
  projects: { country: string; language: string; engines: Engine[] };
  clustering: {
    method: Method;
    threshold: number;
    serp_threshold: number;
    mode: "soft" | "hard";
    level: "page" | "site";
  };
  budgets: Record<string, number>;
  confirm_above_usd: number;
  disabled: string[];
  wordstat_hourly_quota: number;
  suggest: {
    google: boolean;
    yandex: boolean;
    accepted_at: string | null;
    pause_ms: number;
    daily_limit: number;
  };
  google: {
    client_id: string | null;
    scopes: string[];
    ads_customer_id: string | null;
    ads_login_customer_id: string | null;
  };
  serp: { chains: Record<string, string[]>; max_age_days: number };
  browser: { executable: string | null; delay_secs: number; daily_cap: number; proxy: string | null };
  network: { proxy: string | null; timeout_secs: number };
}

/** Where the attended browser mode stands. */
export interface BrowserStatus {
  executable: string | null;
  enabled: boolean;
  engines: Engine[];
  used_today: Record<string, number>;
  daily_cap: number;
  delay_secs: number;
  profile_dir: string;
}

export type JobKind = "discover" | "enrich" | "serp" | "cluster" | "search-console" | "google-login" | "account-login";
export type JobStatus = "running" | "done" | "partial" | "failed" | "cancelled";

export interface Failure {
  provider: string;
  step: string;
  error: string;
}

export type ChallengeKind = "consent" | "captcha" | "blocked";

/** A check page in the browser window waiting for the person. */
export interface CheckPage {
  kind: ChallengeKind;
  url: string;
  until: string;
}

export interface Job {
  id: string;
  kind: JobKind;
  label: string;
  project_id?: number;
  status: JobStatus;
  step: string;
  done: number;
  total: number;
  found: number;
  waiting_until?: string;
  check?: CheckPage;
  result?: number;
  cost_usd: number;
  error?: string;
  failures?: Failure[];
  started_at: number;
  revision: number;
}
