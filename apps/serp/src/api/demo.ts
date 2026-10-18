// Sample backend for the browser preview: invented sites, realistic shapes.

import type {
  AnalysisSummary,
  AnalysisView,
  AnalyzeRequest,
  AppInfo,
  Breakdown,
  CloudStatus,
  DomainFacts,
  Engine,
  Feature,
  Job,
  KeywordAnalysis,
  KeywordRow,
  Organic,
  PageScore,
  ParsedPage,
  ProfileOverride,
  Project,
  ProjectSpec,
  ProviderInfo,
  ProviderSpec,
  ProviderUsage,
  RankReport,
  RankRow,
  ScoreComponent,
  SearchRequest,
  Settings,
  SnapshotSummary,
} from "./types";

const sites = [
  ["kahverehberi.example", "Türk kahvesi nasıl yapılır? Adım adım köpüklü tarif", 2140],
  ["mutfakdefteri.example", "Evde Türk Kahvesi Yapımı: Ölçüler ve Püf Noktaları", 1780],
  ["video.example", "Türk kahvesi nasıl yapılır (video)", 320],
  ["lezzetatlasi.example", "Bol Köpüklü Türk Kahvesi Tarifi", 1290],
  ["forum.example", "Türk kahvesi köpüğü neden olmuyor? - Forum", 860],
  ["kavurmaci.example", "Türk Kahvesi Nasıl Pişirilir? Kavurmacıdan Rehber", 1520],
  ["kahvekeyfi.example", "Türk Kahvesi Yapmanın 7 Kuralı", 940],
  ["ansiklopedi.example", "Türk kahvesi - Ansiklopedi", 3100],
  ["gurmedergi.example", "Cezvede Türk Kahvesi: Gurme Yöntemi", 1150],
  ["tarifkutusu.example", "Türk Kahvesi Tarifi (Resimli Anlatım)", 610],
] as const;

const own = "kahvekeyfi.example";

const round = (value: number, digits = 1) => Math.round(value * 10 ** digits) / 10 ** digits;
const seeded = (seed: number) => () => {
  seed = (seed * 9301 + 49297) % 233280;
  return seed / 233280;
};

function breakdown(model: string, parts: [string, number, number, string, Record<string, unknown>?][]): Breakdown {
  const total = parts.reduce((sum, [, weight]) => sum + weight, 0);
  const components: ScoreComponent[] = parts.map(([id, weight, normalized, reason, params]) => ({
    id,
    weight,
    value: normalized,
    normalized,
    contribution: round((normalized * weight * 100) / total),
    reason,
    ...(params ? { params } : {}),
  }));
  return {
    score: round(components.reduce((sum, component) => sum + component.contribution, 0)),
    model,
    confidence: 1,
    components,
  };
}

function page(position: number, random: () => number): PageScore {
  const [domain, , words] = sites[(position - 1) % sites.length]!;
  const strong = 1 - position / 14;
  const content = breakdown("content-1", [
    ["placement", 20, round(0.55 + random() * 0.45, 2), "content.placement", { title: 1, h1: 1, opening: round(random(), 1), slug: position % 3 ? 1 : 0, meta_description: 0.6 }],
    ["coverage", 25, round(0.4 + strong * 0.5 * random() + 0.1, 2), "content.coverage", { expected: 14, missing: ["cezve", "köpük", "telve"].slice(0, position % 4) }],
    ["depth", 15, round(Math.min(1, words / 1400), 2), "content.depth", { words, median: 1220 }],
    ["structure", 10, round(0.6 + random() * 0.4, 2), "content.structure", { h1: 1, h2: 3 + (position % 5), median_h2: 5, lists: 2, tables: position % 2 }],
    ["questions", 5, round(random(), 2), "content.questions", { answered: position % 4, questions: 4 }],
    ["media", 5, round(0.5 + random() * 0.5, 2), "content.media", { images: 6, with_alt: 4, video: position === 3 }],
    ["schema", 10, position % 3 ? 1 : 0.4, "content.schema", { types: position % 3 ? ["Recipe", "FAQPage"] : ["Article"], expected: ["Recipe", "HowTo"] }],
    ["snippet", 5, round(0.7 + random() * 0.3, 2), "content.snippet", { title_chars: 52, description_chars: 148 }],
    ["freshness", 5, round(random(), 2), "content.freshness", { days: 40 + position * 31 }],
  ]);
  const technical = breakdown("tech-1", [
    ["health", 70, round(0.62 + random() * 0.36, 2), "technical.health", { findings: ["images-missing-alt", "canonical-missing"].slice(0, position % 3), penalty: round(10 + random() * 20) }],
    ["security", 15, position === 5 ? 0.5 : 1, "technical.security", { https: true, hsts: position !== 5 }],
    ["speed", 15, round(0.45 + random() * 0.55, 2), "technical.speed", { ttfb_ms: 180 + Math.round(random() * 700) }],
  ]);
  const opr = round(7.4 - position * 0.45 + random(), 2);
  const site = breakdown("site-1", [
    ["pagerank", 30, round(opr / 10, 2), "site.pagerank", { open_page_rank: opr }],
    ["referring_domains", 25, round(0.85 - position * 0.05, 2), "site.referring_domains", { referring_domains: Math.round(24000 / position) }],
    ["age", 15, round(Math.log(1 + 22 - position * 1.6) / Math.log(16), 2), "site.age", { years: round(22 - position * 1.6) }],
    ["technical", 10, round(technical.score / 100, 2), "site.technical", { https: true, hsts: position !== 5 }],
  ]);
  site.confidence = 0.8;
  const strength = round(0.55 * site.score + 0.3 * content.score + 0.15 * (90 - position * 3));
  return { position, url: `https://www.${domain}/turk-kahvesi`, domain, content, technical, site, strength, words };
}

function organic(keyword: string): Organic[] {
  return sites.map(([domain, title], index) => ({
    position: index + 1,
    absolute_position: index + 3,
    page: 1,
    url: `https://www.${domain}/turk-kahvesi`,
    host: `www.${domain}`,
    domain,
    title: index === 0 ? title : title.replace("Türk kahvesi", keyword),
    snippet:
      "Cezveye kişi başı bir fincan soğuk su ve bir tatlı kaşığı kahve koyun; kısık ateşte köpürene kadar karıştırmadan pişirin.",
    displayed_url: `www.${domain} › turk-kahvesi`,
  }));
}

const features: Feature[] = [
  { kind: "ai-overview", absolute_position: 1, page: 1, items: [{ text: "Türk kahvesi cezvede, soğuk suyla ve kısık ateşte pişirilir." }] },
  { kind: "featured-snippet", absolute_position: 2, page: 1, items: [{ url: "https://www.kahverehberi.example/turk-kahvesi", domain: "kahverehberi.example" }] },
  {
    kind: "people-also-ask",
    absolute_position: 4,
    page: 1,
    items: [
      { text: "Türk kahvesi kaç dakika pişirilir?" },
      { text: "Türk kahvesine şeker ne zaman konur?" },
      { text: "Köpüklü kahve için ne yapmalı?" },
      { text: "Türk kahvesi sütle yapılır mı?" },
    ],
  },
  { kind: "videos", absolute_position: 6, page: 1, items: [{ url: "https://video.example/watch?v=1", domain: "video.example" }] },
  {
    kind: "related-searches",
    absolute_position: 14,
    page: 1,
    items: [{ text: "türk kahvesi nasıl yapılır köpüklü" }, { text: "sütlü türk kahvesi" }, { text: "türk kahvesi makinesi" }, { text: "dibek kahvesi nasıl yapılır" }],
  },
];

function domainFacts(): DomainFacts[] {
  return sites.map(([domain], index) => ({
    domain,
    created: `${2003 + index * 2}-0${(index % 9) + 1}-12`,
    age_source: index % 3 === 2 ? "wayback" : index % 2 ? "whois" : "rdap",
    age_years: round(22 - index * 1.9),
    open_page_rank: round(7.1 - index * 0.42, 2),
    page_rank_position: 4200 + index * 9100,
    referring_domains: Math.round(24000 / (index + 1)),
    referring_source: "open-pagerank",
  }));
}

function analysisOf(keyword: string, seed: number, engine: Engine = "google", fetchedAt = "2026-10-18T07:42:10Z"): AnalysisView {
  const random = seeded(seed);
  const pages = sites.map((_, index) => page(index + 1, random));
  // Click-weighted like the real model, so the first results count most.
  const ctr = [0.28, 0.16, 0.11, 0.08, 0.06, 0.05, 0.04, 0.035, 0.03, 0.025];
  const competition = round(
    pages.reduce((sum, item, index) => sum + ctr[index]! * (item.strength ?? 0), 0) / ctr.reduce((a, b) => a + b, 0) / 100 - 0.22,
    3,
  );
  const difficulty: Breakdown = {
    score: round(competition * 100 + 2 - 1 + 1.4),
    model: "kd-1",
    confidence: 1,
    components: [
      { id: "competition", weight: 1, value: competition, normalized: competition, contribution: round(competition * 100), reason: "kd.competition", params: { measured: 10, results: 10 } },
      { id: "brands", weight: 0, value: 0.2, contribution: 2, reason: "kd.brands", params: { domains: ["ansiklopedi.example", "video.example"] } },
      { id: "forums", weight: 0, value: 0.1, contribution: -1, reason: "kd.forums" },
      { id: "ads", weight: 0, value: 3, contribution: 1.4, reason: "kd.ads" },
    ],
  };
  const target = pages[6]!;
  const analysis: KeywordAnalysis = {
    models: { content: "content-1", technical: "tech-1", site: "site-1", difficulty: "kd-1", intent: "intent-1" },
    difficulty,
    band: difficulty.score < 15 ? "very-easy" : difficulty.score < 30 ? "easy" : difficulty.score < 50 ? "medium" : difficulty.score < 70 ? "hard" : difficulty.score < 85 ? "very-hard" : "extreme",
    intent: {
      primary: "informational",
      secondary: null,
      scores: { informational: 1, commercial: 0.12 },
      signals: ["intent.modifier:nasıl", "intent.feature:people-also-ask", "intent.feature:ai-overview"],
      model: "intent-1",
    },
    pages,
    recommendations: {
      words: { low: 950, median: 1220, high: 1780 },
      h2: { low: 4, median: 5, high: 7 },
      headings: ["malzemeler", "püf noktaları", "köpük nasıl olur", "şekerli mi şekersiz mi"],
      schema: ["Recipe", "FAQPage", "HowTo"],
      title: { median_chars: 49, median_px: 438, keyword_first: 0.7, with_year: 0.1, with_number: 0.3 },
      questions: ["Türk kahvesi kaç dakika pişirilir?", "Türk kahvesine şeker ne zaman konur?", "Köpüklü kahve için ne yapmalı?"],
      short_answer: true,
      terms: ["cezve", "köpük", "fincan", "telve", "kısık ateş", "soğuk su", "orta şekerli", "lokum"],
      ideas: ["türk kahvesi nasıl yapılır köpüklü", "sütlü türk kahvesi", "dibek kahvesi nasıl yapılır", "türk kahvesi kaç dakika pişirilir"],
    },
    target: {
      domain: own,
      position: 7,
      page: target,
      opportunity: breakdown("opp-1", [
        ["win", 40, 0.46, "opportunity.win", { difficulty: difficulty.score }],
        ["demand", 25, 0.71, "opportunity.demand", { volume: 33100 }],
        ["clicks", 20, 0.55, "opportunity.clicks", { features: ["ai-overview", "featured-snippet"], ads: 3 }],
        ["position", 15, 1, "opportunity.position", { position: 7 }],
      ]),
      gaps: ["telve", "orta şekerli", "lokum", "kısık ateş"],
    },
    volume: 33100,
  };
  return {
    id: seed,
    analysis,
    snapshot: {
      query: { engine, keyword, country: "tr", language: "tr", device: "desktop", depth: 10 },
      provider: "browser",
      parser_rev: "google-html@1",
      fetched_at: fetchedAt,
      total_results: 1_840_000,
      organic: organic(keyword),
      features,
    },
    domains: domainFacts(),
  };
}

const summaries: AnalysisSummary[] = [
  { id: 2, snapshot_id: 2, keyword: "türk kahvesi nasıl yapılır", engine: "google", country: "tr", created_at: "2026-10-18T07:42:10Z", target_domain: own, difficulty: 0, band: "medium", intent: "informational", opportunity: 0, volume: 33100 },
  { id: 1, snapshot_id: 1, keyword: "filtre kahve makinesi", engine: "google", country: "tr", created_at: "2026-10-16T13:05:44Z", target_domain: own, difficulty: 0, band: "hard", intent: "commercial", opportunity: 0, volume: 14800 },
];
const views = new Map<number, AnalysisView>();
for (const summary of summaries) {
  const view = analysisOf(summary.keyword, summary.id);
  views.set(summary.id, view);
  summary.difficulty = view.analysis.difficulty.score;
  summary.band = view.analysis.band;
  summary.opportunity = view.analysis.target?.opportunity.score ?? null;
}

let settings: Settings = {
  defaults: { country: "tr", language: "tr", device: "desktop", depth: 10 },
  chains: {
    google: ["serper", "dataforseo", "serpapi", "browser"],
    bing: ["dataforseo", "serpapi", "browser"],
    yandex: ["yandex-cloud", "dataforseo", "serpapi", "browser"],
  },
  disabled: [],
  budgets: {},
  confirm_above_usd: 1,
  timeout_secs: 60,
  browser: { delay_secs: 30, daily_cap: 100 },
};

const secret = (name: string, provider: string) => ({
  name,
  state: "unset" as const,
  variable: `OCTOPUSSEO_SECRET_${provider.toUpperCase().replace(/-/g, "_")}_${name.toUpperCase()}`,
});
let providers: ProviderInfo[] = [
  { id: "browser", name: "Browser (free)", kind: "browser", engines: ["google", "bing", "yandex"], devices: ["desktop"], secrets: [], origins: [], ready: true, via: "own", mode: "auto", credit: null, enabled: true, per_request_usd: 0 },
  { id: "serper", name: "Serper", kind: "builtin", engines: ["google"], devices: ["desktop", "mobile"], secrets: [secret("api_key", "serper")], origins: ["https://google.serper.dev"], ready: false, via: null, mode: "auto", credit: null, enabled: true, per_request_usd: 0.001 },
  { id: "serpapi", name: "SerpApi", kind: "builtin", engines: ["google", "bing", "yandex"], devices: ["desktop", "mobile"], secrets: [secret("api_key", "serpapi")], origins: ["https://serpapi.com"], ready: false, via: null, mode: "auto", credit: null, enabled: true, per_request_usd: 0.015 },
  { id: "dataforseo", name: "DataForSEO", kind: "builtin", engines: ["google", "bing", "yandex"], devices: ["desktop", "mobile"], secrets: [secret("credentials", "dataforseo")], origins: ["https://api.dataforseo.com"], ready: false, via: null, mode: "auto", credit: null, enabled: true, per_request_usd: 0.002 },
  { id: "yandex-cloud", name: "Yandex Search API", kind: "builtin", engines: ["yandex"], devices: ["desktop", "mobile"], secrets: [secret("api_key", "yandex-cloud"), secret("folder_id", "yandex-cloud")], origins: ["https://searchapi.api.cloud.yandex.net"], ready: false, via: null, mode: "auto", credit: null, enabled: true, per_request_usd: 0.004 },
];

const cloud: CloudStatus = { signed_in: false, account: null, error: null, portal_url: "https://my.octopuson.com", api: "https://api.octopuson.com" };

/** Signs the preview in: every built-in source gets a credit offer, and missing keys use it. */
function signInDemo() {
  cloud.signed_in = true;
  cloud.account = { email: "ornek@ajans.com.tr", name: "Örnek Ajans", balance_micros: 12_400_000, held_micros: 0, available_micros: 12_400_000, credit_unit_micros: 1000, low_balance: false, portal_url: "https://my.octopuson.com" };
  for (const provider of providers) {
    if (provider.kind !== "builtin") continue;
    provider.credit = { connector: provider.id === "yandex-cloud" ? "yandex-search" : provider.id, price_multiplier: 1.35 };
    refreshVia(provider);
  }
}

function refreshVia(provider: ProviderInfo) {
  const own = provider.secrets.every((secret) => secret.state === "saved");
  const credit = cloud.signed_in && provider.credit !== null;
  provider.via =
    provider.mode === "own" ? (own ? "own" : null) : provider.mode === "credit" ? (credit ? "credit" : null) : own ? "own" : credit ? "credit" : null;
  provider.ready = provider.kind === "browser" || provider.via !== null;
}

function runAccountLogin() {
  const id = `job-${nextJob++}`;
  const job: Job = { id, kind: "account", label: "OctopusOn SEO", status: "running", step: "account", detail: "WXYZ-K7PQ", done: 0, total: 0, started_at: Date.now(), revision: 0 };
  emit(job);
  setTimeout(() => {
    signInDemo();
    emit({ ...job, status: "done", result: 0, revision: 1 });
  }, 1500);
  return id;
}

const usage: ProviderUsage[] = [
  { provider: "browser", month: "2026-10", requests: 38, successes: 36, failures: 2, cost_usd: 0, credit_usd: 0 },
];

const project: Project = {
  id: 1,
  name: "Kahve Keyfi",
  target: { value: own, mode: "domain" },
  schedule: "daily",
  trackers: [{ id: 1, project_id: 1, engine: "google", country: "tr", language: "tr", device: "desktop", depth: 20 }],
  keywords: 6,
  competitors: ["kahverehberi.example", "mutfakdefteri.example"],
  created_at: "2026-09-20T09:00:00Z",
  last_run_at: "2026-10-18T06:00:00Z",
};
let projects: Project[] = [project];
const keywordNames = ["türk kahvesi nasıl yapılır", "filtre kahve makinesi", "köpüklü türk kahvesi", "dibek kahvesi", "kahve çekirdeği", "sütlü türk kahvesi"];
let keywords: KeywordRow[] = keywordNames.map((keyword, index) => ({
  id: index + 1,
  project_id: 1,
  keyword,
  tags: index < 3 ? ["kahve"] : [],
  created_at: "2026-09-20T09:00:00Z",
}));

function history(seed: number, start: number) {
  const random = seeded(seed);
  const days: [string, number | null][] = [];
  let position = start;
  for (let day = 0; day < 28; day++) {
    position = Math.max(1, Math.min(30, position + Math.round((random() - 0.55) * 3)));
    const date = new Date(Date.UTC(2026, 8, 21 + day)).toISOString().slice(0, 10);
    days.push([date, position > 20 ? null : position]);
  }
  return days;
}

function report(id: number): RankReport {
  const owner = projects.find((item) => item.id === id) ?? project;
  const rows: RankRow[] = keywords
    .filter((keyword) => keyword.project_id === owner.id)
    .map((keyword, index) => {
      const days = history(keyword.id * 7, [9, 14, 4, 18, 25, 11][index % 6]!);
      const known = days.filter(([, position]) => position !== null).map(([, position]) => position as number);
      const position = days.at(-1)?.[1] ?? null;
      const previous = days.at(-2)?.[1] ?? null;
      return {
        keyword_id: keyword.id,
        keyword: keyword.keyword,
        tags: keyword.tags,
        tracker_id: 1,
        position,
        previous,
        change: position !== null && previous !== null ? previous - position : null,
        best: known.length ? Math.min(...known) : null,
        url: position !== null ? `https://www.${own}/${keyword.keyword.replace(/\s+/g, "-")}` : null,
        cannibalised: index === 1,
        checked_at: "2026-10-18T06:00:00Z",
        history: days,
        volume: [33100, 14800, 5400, 2900, 8100, 1900][index % 6]!,
        error: null,
      };
    });
  const visibility = history(3, 10).map(([date], index) => ({
    date,
    visibility: round(8 + index * 0.35 + Math.sin(index / 3) * 1.5),
    top3: 1 + (index > 14 ? 1 : 0),
    top10: 3 + (index > 20 ? 1 : 0),
    ranked: 5,
  }));
  return {
    project: owner,
    rows,
    visibility,
    competitors: [
      { domain: "kahverehberi.example", visibility: 41.2, keywords: 6, average_position: 2.3, own: false },
      { domain: "mutfakdefteri.example", visibility: 28.6, keywords: 5, average_position: 3.8, own: false },
      { domain: own, visibility: 17.9, keywords: 5, average_position: 9.6, own: true },
      { domain: "lezzetatlasi.example", visibility: 12.4, keywords: 4, average_position: 6.1, own: false },
    ],
  };
}

const snapshots = (): SnapshotSummary[] =>
  summaries.map((summary) => ({
    id: summary.snapshot_id,
    keyword: summary.keyword,
    engine: summary.engine,
    country: "tr",
    language: "tr",
    device: "desktop",
    depth: 10,
    provider: "browser",
    fetched_at: summary.created_at,
    organic_count: 10,
    total_results: 1_840_000,
    cost_usd: 0,
  }));

const overrides = new Map<Engine, unknown>();
let specs: ProviderSpec[] = [];

const jobs = new Map<string, Job>();
let nextJob = 1;
const emit = (job: Job) => {
  jobs.set(job.id, job);
  window.dispatchEvent(new CustomEvent("serp-demo-progress", { detail: job }));
};

function runAnalysis(request: AnalyzeRequest) {
  const keyword = request.search.query.keyword.trim();
  const id = `job-${nextJob++}`;
  let job: Job = {
    id,
    kind: "analysis",
    label: keyword,
    engine: request.search.query.engine,
    status: "running",
    step: "search",
    done: 0,
    total: 0,
    started_at: Date.now(),
    revision: 0,
  };
  emit(job);
  const steps: Partial<Job>[] = [
    { step: "page", detail: "browser:1/1" },
    ...Array.from({ length: 11 }, (_, done) => ({ step: "crawl", done, total: 10, detail: undefined })),
    { step: "domains", done: 0, total: 10 },
    { step: "volume" },
    { step: "scoring" },
  ];
  steps.forEach((change, index) => {
    setTimeout(() => {
      job = { ...job, ...change, revision: job.revision + 1 };
      emit(job);
      if (index === steps.length - 1) {
        setTimeout(() => {
          const analysisId = Math.max(...views.keys()) + 1;
          const view = analysisOf(keyword, analysisId, request.search.query.engine, new Date().toISOString());
          views.set(analysisId, view);
          summaries.unshift({
            id: analysisId,
            snapshot_id: analysisId,
            keyword,
            engine: request.search.query.engine,
            country: request.search.query.country,
            created_at: new Date().toISOString(),
            target_domain: request.search.target?.value ?? null,
            difficulty: view.analysis.difficulty.score,
            band: view.analysis.band,
            intent: view.analysis.intent.primary,
            opportunity: view.analysis.target?.opportunity.score ?? null,
            volume: view.analysis.volume ?? null,
          });
          job = { ...job, status: "done", result: analysisId, revision: job.revision + 1 };
          emit(job);
        }, 350);
      }
    }, 280 * (index + 1));
  });
  return id;
}

function runTrack(projectId: number) {
  const owner = projects.find((item) => item.id === projectId);
  if (!owner) throw new Error(`no project ${projectId}`);
  const id = `job-${nextJob++}`;
  const names = keywords.filter((keyword) => keyword.project_id === projectId);
  let job: Job = { id, kind: "track", label: owner.name, project_id: projectId, status: "running", step: "keyword", done: 0, total: names.length, started_at: Date.now(), revision: 0 };
  emit(job);
  names.forEach((keyword, index) => {
    setTimeout(() => {
      job = { ...job, done: index + 1, detail: keyword.keyword, engine: "google", revision: job.revision + 1 };
      if (index === names.length - 1) job = { ...job, status: "done", result: 1 };
      emit(job);
    }, 400 * (index + 1));
  });
  return id;
}

const wait = (ms = 120) => new Promise((resolve) => setTimeout(resolve, ms));

export async function demo(command: string, args: Record<string, unknown>): Promise<unknown> {
  await wait();
  switch (command) {
    case "app_info":
      return {
        version: "0.1.0",
        data_dir: "~/Library/Application Support/com.octopusSEO.octopusSEO-SERP-Finder",
        secret_store: "keychain",
        open_pagerank: "unset",
        tranco: null,
        tranco_notice:
          "The Tranco list aggregates Cisco Umbrella, Majestic, Chrome UX Report, Cloudflare Radar (CC BY-NC 4.0) and Farsight data. Cloudflare Radar data may only be used non-commercially.",
      } satisfies AppInfo;
    case "get_settings":
      return structuredClone(settings);
    case "save_settings": {
      const next = args.settings as Settings;
      if (next.browser.delay_secs < 10) throw new Error("browser.delay_secs must be at least 10");
      settings = structuredClone(next);
      return null;
    }
    case "list_providers":
      return providers.map((provider) => ({
        ...provider,
        enabled: !settings.disabled?.includes(provider.id),
        budget_usd: settings.budgets?.[provider.id],
      }));
    case "set_secret":
    case "remove_secret": {
      const saved = command === "set_secret";
      providers = providers.map((provider) => {
        if (provider.id !== args.provider) return provider;
        const secrets = provider.secrets.map((item) =>
          item.name === args.name ? { ...item, state: saved ? ("saved" as const) : ("unset" as const) } : item,
        );
        return { ...provider, secrets, ready: secrets.every((item) => item.state !== "unset") };
      });
      for (const provider of providers) refreshVia(provider);
      return null;
    }
    case "estimate_search": {
      const request = args.request as SearchRequest;
      const chain = settings.chains[request.query.engine] ?? [];
      return chain.map((id) => {
        const provider = providers.find((item) => item.id === id);
        const pages = Math.ceil(request.query.depth / 10);
        return {
          provider: id,
          ready: Boolean(provider?.ready),
          reason: provider?.ready ? undefined : "set the API key",
          estimate: { requests: pages, usd: (provider?.per_request_usd ?? 0) * pages },
        };
      });
    }
    case "start_analysis":
      return runAnalysis(args.request as AnalyzeRequest);
    case "start_search":
      throw new Error("searching without analysis is not part of the preview");
    case "start_track":
      return runTrack(args.projectId as number);
    case "start_account_login":
      return runAccountLogin();
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
    case "list_jobs":
      return [...jobs.values()].sort((a, b) => b.started_at - a.started_at);
    case "cancel_job": {
      const job = jobs.get(args.id as string);
      if (!job) throw new Error("no job");
      emit({ ...job, status: "cancelled", revision: job.revision + 1 });
      return null;
    }
    case "clear_jobs":
      for (const [id, job] of jobs) if (job.status !== "running" && job.status !== "challenge") jobs.delete(id);
      return null;
    case "list_analyses":
      return summaries.filter((summary) => !args.keyword || summary.keyword.includes(String(args.keyword)));
    case "get_analysis": {
      const view = views.get(args.id as number);
      if (!view) throw new Error(`no analysis ${String(args.id)}`);
      return structuredClone(view);
    }
    case "rescore_analysis":
      return structuredClone(views.get(args.id as number)?.analysis);
    case "list_snapshots":
      return snapshots();
    case "get_snapshot":
      return structuredClone(views.get(args.id as number)?.snapshot);
    case "delete_snapshot":
      return null;
    case "usage":
      return usage;
    case "domain_facts":
      return domainFacts();
    case "list_parsers":
      return (["google", "bing", "yandex"] as const).map((engine) => ({
        engine,
        key: `${engine}-html@1${overrides.has(engine) ? "+local" : ""}`,
        base: `${engine}-html@1`,
        patched: overrides.has(engine),
        mapping: { format: "html", results: { items: engine === "google" ? "div.MjjYud:has(h3)" : "li.b_algo", fields: { url: "a @href", title: "h3 ::text" } } },
        ...(overrides.has(engine) ? { patch: overrides.get(engine) } : {}),
      }));
    case "save_parser_override": {
      const patch = args.patch as ProfileOverride;
      overrides.set(patch.engine, patch.mapping);
      return null;
    }
    case "remove_parser_override":
      overrides.delete(args.engine as Engine);
      return null;
    case "test_parser":
    case "test_spec": {
      const view = views.get(2)!;
      return {
        page: 1,
        organic: view.snapshot.organic.slice(0, 10),
        features: view.snapshot.features,
        total_results: view.snapshot.total_results,
        coverage: { matched: 10, usable: 10 },
      } satisfies ParsedPage;
    }
    case "stored_pages":
      return [
        {
          page: 1,
          url: "https://www.google.com/search?q=t%C3%BCrk+kahvesi+nas%C4%B1l+yap%C4%B1l%C4%B1r&hl=tr&gl=tr",
          status: 0,
          content_type: "text/html",
          size: 812_340,
          body: "<!doctype html><html lang=\"tr\"><head><title>türk kahvesi nasıl yapılır - Google'da Ara</title></head><body>…</body></html>",
        },
      ];
    case "reparse_snapshot":
      return structuredClone(views.get(args.snapshotId as number)?.snapshot);
    case "custom_specs":
      return structuredClone(specs);
    case "save_spec": {
      const spec = args.spec as ProviderSpec;
      if (!spec.id || !/^[a-z0-9-]+$/.test(spec.id)) throw new Error("the id may contain lowercase letters, digits and dashes only");
      specs = [...specs.filter((item) => item.id !== spec.id), spec];
      return null;
    }
    case "remove_spec":
      specs = specs.filter((item) => item.id !== args.id);
      return null;
    case "import_specs": {
      const parsed = JSON.parse(args.json as string) as { providers?: ProviderSpec[] } | ProviderSpec[];
      const list = Array.isArray(parsed) ? parsed : (parsed.providers ?? []);
      specs = [...specs.filter((item) => !list.some((spec) => spec.id === item.id)), ...list];
      return list.map((spec) => spec.id);
    }
    case "export_specs":
      return JSON.stringify({ version: 1, providers: specs.filter((spec) => (args.ids as string[]).includes(spec.id)) }, null, 2);
    case "export_rank_report":
    case "export_analysis":
      throw new Error("the preview does not write files");
    case "close_browser":
    case "reset_browser_profile":
      return null;
    case "update_tranco":
      throw new Error("the preview does not download datasets");
    case "list_projects":
      return projects.map((item) => ({ ...item, keywords: keywords.filter((keyword) => keyword.project_id === item.id).length }));
    case "create_project": {
      const spec = args.spec as ProjectSpec;
      if (!spec.name.trim()) throw new Error("the project needs a name");
      const created: Project = {
        id: Math.max(0, ...projects.map((item) => item.id)) + 1,
        name: spec.name,
        target: spec.target,
        schedule: spec.schedule ?? "daily",
        trackers: spec.trackers.map((tracker, index) => ({ ...tracker, id: 10 + index, project_id: 0 })),
        keywords: 0,
        competitors: [],
        created_at: new Date().toISOString(),
      };
      projects = [...projects, created];
      return created;
    }
    case "update_project":
      projects = projects.map((item) => (item.id === (args.project as Project).id ? (args.project as Project) : item));
      return null;
    case "delete_project":
      projects = projects.filter((item) => item.id !== args.id);
      return null;
    case "add_keywords": {
      const known = new Set(keywords.filter((item) => item.project_id === args.projectId).map((item) => item.keyword.toLocaleLowerCase("tr")));
      const added = (args.keywords as string[]).filter((keyword) => keyword.trim() && !known.has(keyword.toLocaleLowerCase("tr")));
      keywords = [
        ...keywords,
        ...added.map((keyword, index) => ({
          id: keywords.length + index + 1,
          project_id: args.projectId as number,
          keyword: keyword.trim(),
          tags: args.tags as string[],
          created_at: new Date().toISOString(),
        })),
      ];
      return added.length;
    }
    case "list_keywords":
      return keywords.filter((item) => item.project_id === args.projectId);
    case "remove_keyword":
      keywords = keywords.filter((item) => item.id !== args.keywordId);
      return null;
    case "set_competitors":
      projects = projects.map((item) => (item.id === args.projectId ? { ...item, competitors: args.domains as string[] } : item));
      return null;
    case "estimate_track": {
      const count = keywords.filter((item) => item.project_id === args.projectId).length;
      return { searches: count, requests: count * 2, usd: 0 };
    }
    case "rank_report":
      return report(args.projectId as number);
    default:
      throw new Error(`unknown command ${command}`);
  }
}
