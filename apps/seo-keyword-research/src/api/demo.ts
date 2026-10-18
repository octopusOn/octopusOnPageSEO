// Sample data for the browser preview (`pnpm keywords:dev`), shaped like the desktop commands.

import type {
  CloudStatus,
  ClusterQuery,
  ClusterRecord,
  ClusteringRow,
  DiscoverRequest,
  GoogleStatus,
  Job,
  JobKind,
  KeywordQuery,
  KeywordRow,
  MemberRow,
  ProjectRow,
  ProviderInfo,
  Settings,
} from "./types";

const now = () => new Date().toISOString().replace(/\.\d+Z$/, "Z");

const settings: Settings = {
  version: 1,
  projects: { country: "tr", language: "tr", engines: ["google"] },
  clustering: { method: "hybrid", threshold: 0.6, serp_threshold: 3, mode: "soft", level: "page" },
  budgets: { dataforseo: 20 },
  confirm_above_usd: 1,
  disabled: [],
  wordstat_hourly_quota: 100,
  suggest: { google: false, yandex: false, accepted_at: null, pause_ms: 2000, daily_limit: 2000 },
  google: { client_id: null, scopes: [], ads_customer_id: null, ads_login_customer_id: null },
  serp: { chains: { google: ["dataforseo", "serper", "serpapi"], yandex: ["yandex-cloud", "dataforseo", "serpapi"], bing: ["dataforseo", "serpapi"] }, max_age_days: 30 },
  browser: { executable: null, delay_secs: 30, daily_cap: 100, proxy: null },
  network: { proxy: null, timeout_secs: 30 },
};

const HEADS: [string, number, string | null][] = [
  ["kahve makinesi", 90500, "commercial"],
  ["türk kahvesi", 74000, null],
  ["filtre kahve", 33100, null],
  ["espresso makinesi", 27100, "commercial"],
  ["kahve çekirdeği", 18100, null],
  ["french press", 14800, null],
  ["kahve değirmeni", 9900, null],
  ["moka pot", 8100, null],
  ["soğuk kahve", 6600, "informational"],
  ["kahve fincanı", 5400, null],
];
const MODIFIERS: [string, number, string | null][] = [
  ["", 1, null],
  ["fiyatları", 0.18, "transactional"],
  ["nasıl yapılır", 0.12, "informational"],
  ["en iyi", 0.1, "commercial"],
  ["tavsiye", 0.06, "commercial"],
  ["yorumları", 0.05, "commercial"],
  ["nedir", 0.04, "informational"],
  ["indirim", 0.03, "transactional"],
  ["temizleme", 0.025, "informational"],
  ["2026", 0.02, null],
  ["istanbul", 0.015, "local"],
  ["ucuz", 0.012, "transactional"],
  ["markaları", 0.01, "commercial"],
  ["kullanımı", 0.008, "informational"],
  ["hediye", 0.006, null],
];

function seasonal(volume: number, seed: number) {
  return Array.from({ length: 12 }, (_, month) => ({
    month: `2025-${String(month + 1).padStart(2, "0")}`,
    volume: Math.round(volume * (0.8 + 0.4 * Math.abs(Math.sin(seed + month / 2)))),
  }));
}

const keywords: KeywordRow[] = [];
const clusters: ClusterRecord[] = [];
const members = new Map<number, MemberRow[]>();
HEADS.forEach(([head, volume, headIntent], headIndex) => {
  const number = headIndex;
  const rows: MemberRow[] = [];
  MODIFIERS.forEach(([modifier, share, intent], modifierIndex) => {
    const text = modifier === "en iyi" ? `en iyi ${head}` : modifier ? `${head} ${modifier}` : head;
    const id = keywords.length + 1;
    const keywordVolume = Math.max(10, Math.round((volume * share) / 10) * 10);
    const difficulty = Math.round(25 + ((headIndex * 7 + modifierIndex * 5) % 60));
    const row: KeywordRow = {
      id,
      keyword: text,
      status: "active",
      tags: modifierIndex === 1 ? ["fiyat"] : [],
      sources: modifierIndex % 3 === 0 ? ["dataforseo-related", "import"] : ["dataforseo-related"],
      volume: keywordVolume,
      cpc_usd: Math.round((0.2 + ((headIndex + modifierIndex) % 9) / 10) * 100) / 100,
      competition: Math.round(((headIndex * 13 + modifierIndex * 7) % 100)) / 100,
      difficulty,
      monthly: seasonal(keywordVolume, headIndex + modifierIndex),
      intent: intent ?? headIntent,
      cluster: modifierIndex < 9 ? number : HEADS.length + headIndex,
      pillar: modifierIndex === 0 || modifierIndex === 9,
      serp: modifierIndex < 4,
    };
    keywords.push(row);
    rows.push({
      keyword_id: id,
      keyword: text,
      score: modifierIndex === 0 ? 1 : 0.9 - modifierIndex * 0.03,
      shared: modifierIndex < 4 ? 6 - modifierIndex : null,
      volume: row.volume,
      cpc_usd: row.cpc_usd,
      difficulty: row.difficulty,
      intent: row.intent,
      pillar: modifierIndex === 0,
      pinned: false,
    });
  });
  const main = rows.slice(0, 9);
  const tail = rows.slice(9).map((member, index) => ({ ...member, pillar: index === 0, score: index === 0 ? 1 : member.score }));
  members.set(number, main);
  members.set(HEADS.length + headIndex, tail);
  const total = (list: MemberRow[]) => list.reduce((sum, member) => sum + (member.volume ?? 0), 0);
  clusters.push({
    number,
    pillar_id: main[0]?.keyword_id ?? 0,
    label: head,
    size: main.length,
    volume: total(main),
    intent: headIntent,
    mixed_intent: headIndex % 3 === 0,
    difficulty: 30 + headIndex * 4,
    cpc_usd: 0.4 + headIndex / 20,
    verified: headIndex < 6,
    target_url: headIndex % 2 === 0 ? `https://www.ornek-kahve.com.tr/${head.replace(/\s+/g, "-")}` : null,
    competing: headIndex === 2 ? ["https://www.ornek-kahve.com.tr/blog/filtre-kahve-nasil-yapilir"] : [],
    gap: headIndex % 2 === 1,
    competitors: [
      { domain: "trendyol.com", keywords: 7 },
      { domain: "hepsiburada.com", keywords: 6 },
      { domain: "kahvedunyasi.com", keywords: 4 },
    ],
  });
  clusters.push({
    number: HEADS.length + headIndex,
    pillar_id: tail[0]?.keyword_id ?? 0,
    label: tail[0]?.keyword ?? head,
    size: tail.length,
    volume: total(tail),
    intent: null,
    mixed_intent: false,
    difficulty: 20 + headIndex * 3,
    cpc_usd: 0.3,
    verified: false,
    target_url: null,
    competing: [],
    gap: false,
    competitors: [],
  });
});

const projects: ProjectRow[] = [
  {
    id: 1,
    name: "Örnek Kahve · Türkiye",
    country: "tr",
    language: "tr",
    engines: ["google", "yandex"],
    site: "ornek-kahve.com.tr",
    competitors: ["kahvedunyasi.com", "starbucks.com.tr"],
    created_at: "2026-10-12T09:30:00Z",
    keywords: keywords.length,
    excluded: 4,
    clusters: clusters.length,
  },
];

const clusterings: ClusteringRow[] = [
  {
    id: 3,
    project_id: 1,
    method: "hybrid",
    params: { threshold: 0.6, serp_threshold: 3, mode: "soft", level: "page" },
    created_at: "2026-10-18T08:12:00Z",
    keywords: keywords.length,
    clusters: clusters.length,
  },
  {
    id: 2,
    project_id: 1,
    method: "lexical",
    params: { threshold: 0.6 },
    created_at: "2026-10-17T16:40:00Z",
    keywords: keywords.length - 10,
    clusters: clusters.length + 4,
  },
];

const providers: ProviderInfo[] = [
  { id: "wordstat", name: "Yandex Wordstat", secrets: [{ name: "api_key", state: "unset" }, { name: "folder_id", state: "unset" }], ready: false, via: null, mode: "auto", credit: null, enabled: true, budget_usd: null, spent_usd: 0, offers: ["discover", "volume", "trend"], signup: "https://yandex.cloud/en/docs/search-api/" },
  { id: "dataforseo", name: "DataForSEO", secrets: [{ name: "credentials", state: "saved" }], ready: true, via: "own", mode: "auto", credit: null, enabled: true, budget_usd: 20, spent_usd: 3.42, offers: ["discover", "volume", "difficulty", "intent", "competitors", "autocomplete"], signup: "https://app.dataforseo.com/register" },
  { id: "serpapi", name: "SerpApi", secrets: [{ name: "api_key", state: "unset" }], ready: false, via: null, mode: "auto", credit: null, enabled: true, budget_usd: null, spent_usd: 0, offers: ["autocomplete"], signup: "https://serpapi.com/users/sign_up" },
  { id: "yandex-cloud", name: "Yandex Search API", secrets: [{ name: "api_key", state: "unset" }, { name: "folder_id", state: "unset" }], ready: false, via: null, mode: "auto", credit: null, enabled: true, budget_usd: null, spent_usd: 0, offers: ["result pages"], signup: "https://yandex.cloud/en/docs/search-api/" },
  { id: "serper", name: "Serper", secrets: [{ name: "api_key", state: "saved" }], ready: true, via: "own", mode: "auto", credit: null, enabled: true, budget_usd: 5, spent_usd: 0.12, offers: ["result pages"], signup: "https://serper.dev" },
  { id: "browser", name: "Browser (attended)", secrets: [], ready: true, via: "own", mode: "auto", credit: null, enabled: true, budget_usd: null, spent_usd: 0, offers: ["result pages"], signup: "" },
];

/** The engines whose chain lists the browser. */
const browserEngines = () => (["google", "yandex", "bing"] as const).filter((engine) => settings.serp.chains[engine]?.includes("browser"));

const CREDIT_SOURCES = ["wordstat", "dataforseo", "serpapi", "yandex-cloud", "serper"];
const cloud: CloudStatus = { signed_in: false, account: null, error: null, portal_url: "https://my.octopuson.com", api: "https://api.octopuson.com" };

/** Signs the preview in: every paid source gets a credit offer, and missing keys use it. */
function signInDemo() {
  cloud.signed_in = true;
  cloud.account = {
    email: "ornek@ajans.com.tr",
    name: "Örnek Ajans",
    balance_micros: 12_400_000,
    held_micros: 0,
    available_micros: 12_400_000,
    credit_unit_micros: 1000,
    low_balance: false,
    portal_url: "https://my.octopuson.com",
  };
  for (const provider of providers) {
    if (!CREDIT_SOURCES.includes(provider.id)) continue;
    provider.credit = { connector: provider.id === "yandex-cloud" ? "yandex-search" : provider.id, price_multiplier: 1.35 };
    refreshVia(provider);
  }
}

function refreshVia(provider: ProviderInfo) {
  const own = provider.secrets.every((secret) => secret.state === "saved");
  const credit = cloud.signed_in && provider.credit !== null;
  provider.via =
    provider.mode === "own" ? (own ? "own" : null) : provider.mode === "credit" ? (credit ? "credit" : null) : own ? "own" : credit ? "credit" : null;
  provider.ready = provider.via !== null;
}

const google: GoogleStatus = { client_id: null, client_secret: false, connected: false, scopes: [], ads_customer_id: null };
const jobs: Job[] = [];

function emit(job: Job) {
  window.dispatchEvent(new CustomEvent("keywords-demo-progress", { detail: { ...job } }));
}

/** A job; with `checkAt`, a check page waits for the person at that step for a few ticks. */
function startJob(kind: JobKind, total: number, step: string, checkAt?: number) {
  const job: Job = {
    id: `demo-${jobs.length + 1}`,
    kind,
    label: projects[0]?.name ?? "",
    project_id: 1,
    status: "running",
    step,
    done: 0,
    total,
    found: 0,
    cost_usd: 0,
    started_at: Date.now(),
    revision: 0,
  };
  jobs.unshift(job);
  emit(job);
  let waited = 0;
  const timer = setInterval(() => {
    if (checkAt !== undefined && job.done === checkAt && waited < 8) {
      waited += 1;
      job.check = {
        kind: "captcha",
        url: "https://www.google.com/search?q=kahve+makinesi&hl=tr&gl=tr",
        until: new Date(Date.now() + 600_000).toISOString().replace(/\.\d+Z$/, "Z"),
      };
      job.revision += 1;
      emit(job);
      return;
    }
    job.check = undefined;
    job.done = Math.min(job.total, job.done + 1);
    job.found += 37;
    job.cost_usd = Math.round(job.done * 0.0132 * 1000) / 1000;
    job.revision += 1;
    if (job.done >= job.total) {
      job.status = "done";
      job.result = 4;
      clearInterval(timer);
    }
    emit(job);
  }, 700);
  return job.id;
}

function keywordPage(query: KeywordQuery) {
  const search = (query.search ?? "").toLocaleLowerCase("tr");
  let rows = keywords.filter(
    (row) =>
      (!search || row.keyword.toLocaleLowerCase("tr").includes(search)) &&
      (!query.intent || row.intent === query.intent) &&
      (!query.source || row.sources.includes(query.source)) &&
      (!query.tag || row.tags.includes(query.tag)) &&
      (query.min_volume == null || (row.volume ?? 0) >= query.min_volume) &&
      (query.cluster == null || row.cluster === query.cluster) &&
      (query.status !== "excluded"),
  );
  const value = (row: KeywordRow): number | string => {
    switch (query.sort) {
      case "keyword":
        return row.keyword;
      case "cpc":
        return row.cpc_usd ?? -1;
      case "competition":
        return row.competition ?? -1;
      case "difficulty":
        return row.difficulty ?? -1;
      case "intent":
        return row.intent ?? "~";
      case "cluster":
        return row.cluster ?? 1e9;
      case "added":
        return row.id;
      default:
        return row.volume ?? -1;
    }
  };
  rows = [...rows].sort((a, b) => {
    const [x, y] = [value(a), value(b)];
    const order = typeof x === "string" ? String(x).localeCompare(String(y), "tr") : (x as number) - (y as number);
    return query.descending ? -order : order;
  });
  return { total: rows.length, rows: rows.slice(query.offset, query.offset + query.limit) };
}

function clusterPage(query: ClusterQuery) {
  const search = (query.search ?? "").toLocaleLowerCase("tr");
  let rows = clusters.filter(
    (row) =>
      (!search || (members.get(row.number) ?? []).some((member) => member.keyword.toLocaleLowerCase("tr").includes(search))) &&
      (!query.intent || row.intent === query.intent) &&
      (query.verified == null || row.verified === query.verified) &&
      (query.gap == null || row.gap === query.gap) &&
      (query.min_size == null || row.size >= query.min_size),
  );
  const key = (row: ClusterRecord) =>
    query.sort === "size" ? row.size : query.sort === "difficulty" ? row.difficulty ?? -1 : query.sort === "number" ? -row.number : query.sort === "label" ? 0 : row.volume;
  rows = [...rows].sort((a, b) => (query.sort === "label" ? a.label.localeCompare(b.label, "tr") : key(b) - key(a)));
  return { total: rows.length, rows: rows.slice(query.offset, query.offset + query.limit) };
}

export async function demo(command: string, args: Record<string, unknown>): Promise<unknown> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  switch (command) {
    case "app_info":
      return { version: "0.1.0", data_dir: "~/Library/Application Support/com.octopusSEO.octopusSEO-Keyword-Miner", secret_store: "keychain" };
    case "get_settings":
      return structuredClone(settings);
    case "save_settings":
      Object.assign(settings, args.settings);
      return null;
    case "list_projects":
      return projects;
    case "create_project":
    case "update_project":
      return { ...projects[0], ...(args.spec as object) };
    case "list_runs":
      return [
        { id: 7, project_id: 1, kind: "serp", status: "done", planned: 30, done: 30, estimated_cost_usd: 0.018, cost_usd: 0.018, started_at: "2026-10-18T08:05:00Z", finished_at: "2026-10-18T08:09:00Z", error: null },
        { id: 6, project_id: 1, kind: "discover", status: "partial", planned: 24, done: 24, estimated_cost_usd: 0.31, cost_usd: 0.27, started_at: "2026-10-17T15:10:00Z", finished_at: "2026-10-17T15:16:00Z", error: "wordstat: the Yandex Wordstat key `api_key` is not set" },
        { id: 5, project_id: 1, kind: "import", status: "done", planned: 412, done: 412, estimated_cost_usd: 0, cost_usd: 0, started_at: "2026-10-12T09:40:00Z", finished_at: "2026-10-12T09:40:02Z", error: null },
      ];
    case "list_keywords":
      return keywordPage(args.query as KeywordQuery);
    case "keyword_facets":
      return { tags: [["fiyat", 10]], sources: [["dataforseo-related", keywords.length], ["import", 50]] };
    case "add_keywords":
      return { received: (args.keywords as string[]).length, added: (args.keywords as string[]).length, known: 0, rejected: 0 };
    case "preview_import":
      return {
        columns: [
          { header: "Keyword", role: "keyword" },
          { header: "Volume", role: "volume" },
          { header: "KD", role: "difficulty" },
          { header: "CPC", role: "cpc" },
          { header: "Parent Topic", role: null },
        ],
        rows: keywords.slice(0, 8).map((row) => [row.keyword, String(row.volume), String(row.difficulty), String(row.cpc_usd), row.keyword.split(" ")[0] ?? ""]),
        total: 412,
      };
    case "import_file":
      return { rows: 412, received: 412, added: 388, known: 24, rejected: 0, metrics: 412, columns: [] };
    case "set_status":
      return (args.ids as number[]).length;
    case "estimate_discovery": {
      const request = args.request as DiscoverRequest;
      return {
        sources: request.sources.map((source) => ({
          source,
          provider: source === "wordstat" ? "wordstat" : source.endsWith("-suggest") ? source : "dataforseo",
          requests: Math.max(1, request.seeds.length) * (source.includes("autocomplete") ? 1 + request.expansions.length * 6 : 1),
          cost_usd: source === "wordstat" ? 0.0003 : 0.072,
          ready: source !== "wordstat" && !source.endsWith("-suggest"),
          reason: source === "wordstat" ? "Yandex Wordstat key `api_key` is not set" : source.endsWith("-suggest") ? "the free suggestion endpoints are off" : null,
        })),
        requests: 4,
        cost_usd: 0.288,
        wordstat_hours: 0,
      };
    }
    case "estimate_enrichment":
      return { keywords: [12, 40, 0], requests: 2, cost_usd: 0.1068, reason: null };
    case "estimate_serps":
      // With the browser in the chain, the sample APIs before it are out of budget or keys.
      return browserEngines().includes("google")
        ? {
            keywords: 30,
            cached: 40,
            provider: "browser",
            cost_usd: 0,
            skipped: [
              ["dataforseo", "switched off"],
              ["serper", "the monthly budget of 5 USD is reached"],
              ["serpapi", "SerpApi key `api_key` is not set"],
            ],
          }
        : { keywords: 30, cached: 40, provider: "serper", cost_usd: 0.03, skipped: [["dataforseo", "switched off"]] };
    case "start_discovery":
      return startJob("discover", 6, "dataforseo-related");
    case "start_enrichment":
      return startJob("enrich", 2, "dataforseo-ads");
    case "start_serps":
      return startJob("serp", 8, "google", browserEngines().length > 0 ? 2 : undefined);
    case "start_cluster":
      return startJob("cluster", 1, "hybrid");
    case "start_google_login":
      return startJob("google-login", 1, "browser");
    case "start_account_login": {
      const id = startJob("account-login", 1, "WXYZ-K7PQ");
      setTimeout(signInDemo, 600);
      return id;
    }
    case "cloud_status":
      return cloud;
    case "account_logout":
      cloud.signed_in = false;
      cloud.account = null;
      for (const provider of providers) {
        provider.credit = null;
        refreshVia(provider);
      }
      return null;
    case "set_source_mode": {
      const provider = providers.find((other) => other.id === args.provider);
      if (provider) {
        provider.mode = args.mode as ProviderInfo["mode"];
        refreshVia(provider);
      }
      return null;
    }
    case "open_account_page":
      return null;
    case "start_search_console":
      return startJob("search-console", 2, "search-console");
    case "list_clusterings":
      return clusterings;
    case "list_clusters":
      return clusterPage(args.query as ClusterQuery);
    case "cluster_members":
      return members.get(args.number as number) ?? [];
    case "keyword_serp":
      return {
        keyword_id: args.keywordId,
        engine: "google",
        device: "desktop",
        provider: "serper",
        fetched_at: now(),
        urls: ["https://www.trendyol.com/kahve-makinesi", "https://www.hepsiburada.com/kahve-makineleri", "https://www.ornek-kahve.com.tr/kahve-makinesi", "https://www.kahvedunyasi.com/makineler", "https://tr.wikipedia.org/wiki/Kahve_makinesi"],
        site_only: false,
        features: ["people-also-ask", "shopping", "related-searches"],
        questions: ["Hangi kahve makinesi daha iyi?", "Kahve makinesi kaç watt olmalı?", "Filtre kahve makinesi nasıl temizlenir?", "Evde espresso için hangi makine?"],
        related: ["kahve makinesi fiyatları", "philips kahve makinesi", "arçelik kahve makinesi", "kapsül kahve makinesi"],
      };
    case "place_keywords":
    case "remap":
      return clusterings[0];
    case "list_providers":
      return providers;
    case "usage":
      return [
        { provider: "dataforseo", month: "2026-10", requests: 58, items: 6120, failures: 1, cost_usd: 3.42, credit_usd: 0 },
        { provider: "serper", month: "2026-10", requests: 120, items: 120, failures: 0, cost_usd: 0.12, credit_usd: 0 },
      ];
    case "google_status":
      return google;
    case "search_console_properties":
      return [{ site_url: "sc-domain:ornek-kahve.com.tr", permission: "siteOwner" }];
    case "list_jobs":
      return jobs;
    case "export_keywords":
      return keywords.length;
    case "export_clusters":
      return keywords.length;
    case "secret_state":
      return "unset";
    case "set_enabled": {
      const id = String(args.provider);
      settings.disabled = args.enabled ? settings.disabled.filter((other) => other !== id) : [...new Set([...settings.disabled, id])];
      const provider = providers.find((other) => other.id === id);
      if (provider) provider.enabled = Boolean(args.enabled);
      return null;
    }
    case "browser_status":
      return {
        executable: settings.browser.executable ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        enabled: !settings.disabled.includes("browser"),
        engines: browserEngines(),
        used_today: { google: browserEngines().includes("google") ? 12 : 0, bing: 0, yandex: 0 },
        daily_cap: settings.browser.daily_cap,
        delay_secs: settings.browser.delay_secs,
        profile_dir: "~/Library/Application Support/com.octopusSEO.octopusSEO-Keyword-Miner/browser-profile",
      };
    case "reset_browser_profile":
      return null;
    default:
      return null;
  }
}
