// Sample answers for the interface outside the program (site previews, screenshots): nothing is
// sent anywhere and nothing is stored.

import { CallError } from "./transport";
import type {
  AccountStatus,
  AiAnswer,
  AiCell,
  AiEngine,
  AiEstimate,
  AiPrompt,
  AiPromptRow,
  AiReport,
  AiRun,
  AiRunPoint,
  AiSchedule,
  AiSettings,
  AiSetup,
  AnswerBrand,
  AnswerDetail,
  Brand,
  BrandShare,
  EngineVisibility,
  GoogleStatus,
  GscDay,
  GscPage,
  GscQuery,
  GscReport,
  GscSchedule,
  GscSettings,
  GscSetup,
  GscSync,
  Branding,
  ReportRow,
  ReportSpec,
  ReportsSetup,
  MentionMetrics,
  MentionRow,
  Rival,
  Visibility,
  AnchorRow,
  AppInfo,
  AuditReport,
  AuditRow,
  AuditSchedule,
  AuditSettings,
  AuditSetup,
  BacklinkRow,
  CategoryReport,
  GeoReadiness,
  IssueDetail,
  IssueKind,
  IssuePriority,
  CheckResult,
  CompetitorShare,
  KeywordDetail,
  KeywordRank,
  NewKeyword,
  RankEstimate,
  RankReport,
  RankRun,
  RankSettings,
  RankSetup,
  Schedule,
  SearchEngine,
  TrackerRow,
  VisibilityPoint,
  BacklinkSummary,
  CompetitorRow,
  DomainOverview,
  Estimate,
  Fetched,
  HistoryPoint,
  KeywordData,
  KeywordList,
  KeywordListRow,
  LinkedPageRow,
  ListKeyword,
  NewLostPoint,
  PageRow,
  ProjectRow,
  ProjectSpec,
  ProviderInfo,
  RankedKeywords,
  RankMetrics,
  RankRow,
  ReferringDomainRow,
  SerpPage,
  Settings,
  SubdomainRow,
  Topic,
  TrafficRow,
  UsageRow,
} from "./types";

/** Two hours ago: sample data looks freshly fetched whenever it is shown. */
const NOW = new Date(Date.now() - 2 * 3600 * 1000).toISOString().replace(/\.\d+Z$/, "Z");

let projects: ProjectRow[] = [
  {
    id: 1,
    name: "Örnek Mağaza",
    domain: "ornekmagaza.com",
    country: "tr",
    language: "tr",
    competitors: ["rakip-magaza.com", "digerdukkan.com.tr"],
    brands: ["Örnek Mağaza", "ornekmagaza"],
    created_at: NOW,
    updated_at: NOW,
  },
  {
    id: 2,
    name: "example.org",
    domain: "example.org",
    country: "us",
    language: "en",
    competitors: [],
    brands: [],
    created_at: NOW,
    updated_at: NOW,
  },
];

const overview = (target: string): DomainOverview => ({
  target,
  organic: {
    keywords: 18_420,
    traffic: 96_310,
    traffic_value_usd: 41_870,
    pos_1: 412,
    pos_2_3: 1_034,
    pos_4_10: 3_918,
    pos_11_20: 4_220,
    pos_21_100: 8_836,
    new: 1_204,
    up: 2_310,
    down: 1_985,
    lost: 870,
  },
  paid: {
    keywords: 132,
    traffic: 4_210,
    traffic_value_usd: 3_580,
    pos_1: 61,
    pos_2_3: 48,
    pos_4_10: 23,
    pos_11_20: 0,
    pos_21_100: 0,
    new: 12,
    up: 9,
    down: 14,
    lost: 7,
  },
});

const metrics = (keywords: number, traffic: number, value: number, top: number): RankMetrics => ({
  keywords,
  traffic,
  traffic_value_usd: value,
  pos_1: Math.round(top * 0.2),
  pos_2_3: Math.round(top * 0.3),
  pos_4_10: Math.round(top * 0.5),
  pos_11_20: Math.round(keywords * 0.2),
  pos_21_100: Math.round(keywords * 0.5),
  new: Math.round(keywords * 0.05),
  up: Math.round(keywords * 0.1),
  down: Math.round(keywords * 0.08),
  lost: Math.round(keywords * 0.03),
});

const SAMPLE_KEYWORDS: [string, number, number, number, number, string, string, string[]][] = [
  ["spor ayakkabı", 3, 40500, 62, 3900, "commercial", "/spor-ayakkabi", ["people_also_ask", "images", "ai_overview"]],
  ["koşu ayakkabısı", 1, 14800, 48, 3100, "commercial", "/kosu-ayakkabisi", ["people_also_ask", "video"]],
  ["kadın sneaker", 5, 22200, 55, 1450, "transactional", "/kadin/sneaker", ["images", "shopping"]],
  ["ayakkabı numarası nasıl ölçülür", 2, 6600, 18, 1210, "informational", "/blog/numara-olcme", ["featured_snippet", "people_also_ask", "ai_overview"]],
  ["ornekmagaza", 1, 2900, 4, 2300, "navigational", "/", ["knowledge_graph"]],
  ["deri bot", 8, 9900, 41, 520, "transactional", "/kadin/bot", ["shopping", "images"]],
  ["ayakkabı bakımı", 12, 4400, 22, 140, "informational", "/blog/ayakkabi-bakimi", ["people_also_ask", "video"]],
  ["erkek loafer", 17, 5400, 37, 60, "commercial", "/erkek/loafer", ["images"]],
  ["çocuk ayakkabısı", 24, 18100, 58, 25, "commercial", "/cocuk", ["shopping"]],
  ["su geçirmez ayakkabı", 9, 3600, 33, 210, "commercial", "/su-gecirmez", ["people_also_ask", "ai_overview"]],
];

function report(kind: string, target: string): unknown {
  switch (kind) {
    case "history":
      return Array.from({ length: 24 }, (_, index) => {
        const date = new Date(Date.UTC(2024, 9 + index, 1));
        const growth = 1 + index * 0.035 + Math.sin(index / 2) * 0.04;
        return {
          year: date.getUTCFullYear(),
          month: date.getUTCMonth() + 1,
          organic: metrics(Math.round(12_000 * growth), Math.round(62_000 * growth), Math.round(27_000 * growth), 3_000),
          paid: metrics(120, 3_900, 3_200, 100),
        } satisfies HistoryPoint;
      });
    case "keywords":
      return {
        total: 18_420,
        keywords: SAMPLE_KEYWORDS.map(([keyword, position, volume, difficulty, traffic, intent, path, features], index) => ({
          keyword,
          position,
          absolute_position: position + 1,
          previous_position: index % 3 === 0 ? position + 3 : index % 3 === 1 ? null : position - 2,
          change: index % 3 === 0 ? "up" : index % 3 === 1 ? "new" : "down",
          url: `https://${target}${path}`,
          volume,
          difficulty,
          cpc: 0.35 + index * 0.11,
          competition: 0.4,
          intent,
          traffic,
          traffic_value_usd: traffic * 0.6,
          serp_features: features,
        })),
      } satisfies RankedKeywords;
    case "pages":
      return [
        ["/spor-ayakkabi", 410, 9_800],
        ["/kosu-ayakkabisi", 220, 6_100],
        ["/", 95, 4_700],
        ["/blog/numara-olcme", 160, 3_050],
        ["/kadin/sneaker", 130, 2_400],
      ].map(([path, keywords, traffic]) => ({ url: `https://${target}${String(path)}`, organic: metrics(Number(keywords), Number(traffic), Number(traffic) * 0.5, Number(keywords) / 3) })) satisfies PageRow[];
    case "competitors":
      return [
        ["rakip-magaza.com", 4_210, 9.1, 26_000, 140_000],
        ["digerdukkan.com.tr", 2_950, 12.4, 15_500, 61_000],
        ["buyukpazar.com", 2_310, 7.8, 410_000, 2_900_000],
        ["kosuclub.com", 1_120, 15.2, 6_400, 21_000],
      ].map(([domain, shared, position, keywords, traffic]) => ({
        domain: String(domain),
        shared_keywords: Number(shared),
        average_position: Number(position),
        shared_traffic: Number(traffic) * 0.08,
        keywords: Number(keywords),
        traffic: Number(traffic),
        traffic_value_usd: Number(traffic) * 0.45,
      })) satisfies CompetitorRow[];
    case "subdomains":
      return [
        [`www.${target}`, 17_900, 93_000],
        [`blog.${target}`, 480, 3_100],
        [`destek.${target}`, 40, 210],
      ].map(([subdomain, keywords, traffic]) => ({ subdomain: String(subdomain), organic: metrics(Number(keywords), Number(traffic), Number(traffic) * 0.4, Number(keywords) / 5) })) satisfies SubdomainRow[];
    case "compare":
      return [
        [target, 96_310, 4_210],
        ["rakip-magaza.com", 140_000, 12_400],
        ["digerdukkan.com.tr", 61_000, 900],
      ].map(([domain, organic, paid]) => ({ target: String(domain), organic_traffic: Number(organic), organic_keywords: Math.round(Number(organic) / 5), paid_traffic: Number(paid), paid_keywords: Math.round(Number(paid) / 30) })) satisfies TrafficRow[];
    default:
      return overview(target);
  }
}

const sampleKeyword = (keyword: string, volume: number, difficulty: number, intent: string, features: string[] = []): KeywordData => ({
  keyword,
  volume,
  cpc: 0.4 + (keyword.length % 7) * 0.13,
  competition: 0.45,
  competition_level: difficulty > 50 ? "HIGH" : difficulty > 25 ? "MEDIUM" : "LOW",
  difficulty,
  intent,
  other_intents: intent === "commercial" ? ["transactional"] : [],
  monthly: Array.from({ length: 12 }, (_, index) => {
    const date = new Date(Date.UTC(2025, 10 + index, 1));
    return { year: date.getUTCFullYear(), month: date.getUTCMonth() + 1, volume: Math.round(volume * (0.8 + 0.35 * Math.sin((index + 2) / 2))) };
  }),
  trend: { monthly: 8, quarterly: 21, yearly: 14 },
  serp_features: features,
  results: 4_200_000,
  top_backlinks: 1_520,
  top_referring_domains: 96,
});

const SAMPLE_IDEAS: KeywordData[] = [
  sampleKeyword("spor ayakkabı", 40_500, 62, "commercial", ["people_also_ask", "images", "ai_overview"]),
  sampleKeyword("spor ayakkabı kadın", 18_100, 48, "commercial", ["images", "shopping"]),
  sampleKeyword("spor ayakkabı erkek", 14_800, 47, "commercial", ["images", "shopping"]),
  sampleKeyword("en iyi spor ayakkabı markaları", 2_900, 35, "commercial", ["featured_snippet", "people_also_ask", "ai_overview"]),
  sampleKeyword("spor ayakkabı nasıl yıkanır", 2_400, 12, "informational", ["featured_snippet", "video", "ai_overview"]),
  sampleKeyword("spor ayakkabı numarası nasıl seçilir", 880, 9, "informational", ["people_also_ask"]),
  sampleKeyword("beyaz spor ayakkabı", 9_900, 39, "transactional", ["images", "shopping"]),
  sampleKeyword("spor ayakkabı indirim", 6_600, 44, "transactional", ["shopping"]),
  sampleKeyword("koşu ayakkabısı", 14_800, 51, "commercial", ["video", "images"]),
  sampleKeyword("spor ayakkabı mı sneaker mı", 320, 6, "informational", ["people_also_ask"]),
];

let lists: KeywordListRow[] = [{ id: 1, project_id: 1, name: "Spor ayakkabı", keywords: 3, created_at: NOW }];
let listEntries: Record<number, ListKeyword[]> = {
  1: SAMPLE_IDEAS.slice(0, 3).map((data) => ({ keyword: data.keyword, data, added_at: NOW })),
};

function keywordReport(kind: string, args: Record<string, unknown>): unknown {
  const seed = typeof args.seed === "string" && args.seed ? args.seed : Array.isArray(args.keywords) ? String(args.keywords[0] ?? "") : "";
  switch (kind) {
    case "serp":
      return {
        keyword: seed,
        results: 4_200_000,
        features: ["ai_overview", "organic", "people_also_ask", "images", "shopping"],
        organic: ["rakip-magaza.com", "ornekmagaza.com", "buyukpazar.com", "kosuclub.com", "digerdukkan.com.tr", "blog.ornek.org", "magaza.net", "sporium.com.tr", "ayakkabix.com", "trendy.com"].map((domain, index) => ({
          kind: "organic",
          rank: index + 1,
          absolute: index + 3,
          domain,
          url: `https://www.${domain}/${seed.replace(/\s+/g, "-")}`,
          title: `${seed} | ${domain}`,
          description: "…",
        })),
        other: [{ kind: "ai_overview", rank: 1, absolute: 1, domain: null, url: null, title: null, description: null }],
        check_url: `https://www.google.com.tr/search?q=${encodeURIComponent(seed)}`,
      } satisfies SerpPage;
    case "overview": {
      const asked = Array.isArray(args.keywords) && args.keywords.length > 0 ? args.keywords.map(String) : [seed];
      return {
        total: asked.length,
        seed: null,
        keywords: asked.map((keyword, index) => SAMPLE_IDEAS.find((idea) => idea.keyword === keyword.toLowerCase()) ?? sampleKeyword(keyword.toLowerCase(), 5_000 - index * 300, 30 + index * 5, "commercial", ["images", "ai_overview"])),
      } satisfies KeywordList;
    }
    default:
      return { total: 5_210, seed: sampleKeyword(seed || "spor ayakkabı", 40_500, 62, "commercial"), keywords: SAMPLE_IDEAS } satisfies KeywordList;
  }
}

function backlinkReport(kind: string, args: Record<string, unknown>): unknown {
  const target = typeof args.target === "string" && args.target ? args.target : "ornekmagaza.com";
  const sites = ["haber.com.tr", "blog.moda.net", "forum.kosucular.org", "magazin.com", "spor.gazete.com.tr", "rehber.ornek.org", "alisveris.info", "trendler.co"];
  switch (kind) {
    case "summary":
      return {
        target,
        rank: 48,
        backlinks: 182_340,
        referring_domains: 2_310,
        referring_domains_nofollow: 402,
        referring_main_domains: 2_105,
        referring_ips: 1_980,
        referring_subnets: 1_502,
        referring_pages: 96_500,
        broken_backlinks: 312,
        broken_pages: 41,
        spam_score: 7,
        first_seen: "2017-03-02 11:20:41 +00:00",
        attributes: [{ name: "nofollow", count: 15_200 }, { name: "ugc", count: 320 }],
        link_types: [{ name: "anchor", count: 171_200 }, { name: "image", count: 9_800 }, { name: "redirect", count: 1_340 }],
        top_level_domains: [{ name: "com", count: 1_201 }, { name: "com.tr", count: 640 }, { name: "net", count: 120 }, { name: "org", count: 80 }],
        countries: [{ name: "TR", count: 1_500 }, { name: "US", count: 300 }, { name: "DE", count: 90 }, { name: "NL", count: 44 }],
        platforms: [{ name: "cms", count: 1_400 }, { name: "blogs", count: 610 }, { name: "news", count: 120 }],
      } satisfies BacklinkSummary;
    case "new_lost":
      return Array.from({ length: 12 }, (_, index) => {
        const date = new Date(Date.UTC(2025, 10 + index + 1, 0));
        return { date: date.toISOString().slice(0, 10), new_backlinks: 900 + index * 40, lost_backlinks: 700 + (index % 3) * 150, new_referring_domains: 30 + (index % 4) * 6, lost_referring_domains: 18 + (index % 5) * 4 };
      }) satisfies NewLostPoint[];
    case "backlinks":
      return sites.map((site, index) => ({
        url_from: `https://${site}/${index % 2 ? "en-iyi-ayakkabilar" : "spor-giyim-rehberi"}`,
        domain_from: site,
        title: index % 2 ? "En iyi ayakkabılar" : "Spor giyim rehberi",
        url_to: `https://www.${target}/${index % 3 ? "spor-ayakkabi" : ""}`,
        url_to_status: index === 5 ? 404 : 200,
        anchor: index === 6 ? null : ["spor ayakkabı", "ornekmagaza", "buradan", "koşu ayakkabısı"][index % 4] ?? null,
        dofollow: index % 4 !== 3,
        kind: index === 6 ? "image" : "anchor",
        page_rank: 70 - index * 6,
        domain_rank: 78 - index * 5,
        spam_score: 2 + index,
        first_seen: `2026-0${(index % 9) + 1}-12 10:00:00 +00:00`,
        last_seen: "2026-10-20 10:00:00 +00:00",
        is_new: index < 2,
        is_lost: false,
        is_broken: index === 5,
        country: "TR",
      })) satisfies BacklinkRow[];
    case "referring_domains":
      return sites.map((domain, index) => ({ domain, rank: 78 - index * 5, backlinks: 340 - index * 30, nofollow_only: index % 4 === 3, spam_score: 2 + index, first_seen: `201${9 - (index % 3)}-05-01 00:00:00 +00:00` })) satisfies ReferringDomainRow[];
    case "anchors":
      return [
        ["ornekmagaza", 20_100, 620],
        ["spor ayakkabı", 4_300, 210],
        ["", 5_100, 180],
        ["buradan", 2_200, 140],
        ["koşu ayakkabısı", 900, 66],
      ].map(([anchor, backlinks, domains]) => ({ anchor: String(anchor), backlinks: Number(backlinks), referring_domains: Number(domains), referring_domains_nofollow: Math.round(Number(domains) * 0.1), first_seen: "2018-01-01 00:00:00 +00:00" })) satisfies AnchorRow[];
    case "pages":
      return [
        ["/", "Örnek Mağaza", 1_500, 40_100, 55],
        ["/spor-ayakkabi", "Spor Ayakkabı Modelleri", 420, 9_800, 41],
        ["/blog/numara-olcme", "Ayakkabı numarası nasıl ölçülür?", 260, 3_100, 33],
        ["/eski-kampanya", "Eski kampanya", 80, 950, 20],
      ].map(([path, title, domains, backlinks, rank]) => ({ url: `https://www.${target}${String(path)}`, title: String(title), status: path === "/eski-kampanya" ? 404 : 200, rank: Number(rank), backlinks: Number(backlinks), referring_domains: Number(domains), broken_backlinks: path === "/eski-kampanya" ? 950 : 2 })) satisfies LinkedPageRow[];
    case "bulk": {
      const targets = Array.isArray(args.targets) && args.targets.length ? args.targets.map(String) : [target, "rakip-magaza.com", "digerdukkan.com.tr"];
      return targets.map((entry, index) => ({ target: entry, rank: Math.max(5, 61 - index * 9 + (entry.length % 7)) })) satisfies RankRow[];
    }
    default:
      return null;
  }
}

function gapReport(kind: string, args: Record<string, unknown>): unknown {
  const competitors = Array.isArray(args.competitors) && args.competitors.length ? args.competitors.map(String) : ["rakip-magaza.com", "digerdukkan.com.tr"];
  const own = "ornekmagaza.com";
  const ranking = (domain: string, position: number) => ({ domain, position, url: `https://www.${domain}/`, traffic: Math.round(4000 / position) });
  switch (kind) {
    case "missing":
    case "shared":
      return {
        own,
        competitors,
        keywords: [
          ["koşu ayakkabısı", 14_800, 51, "commercial", [3, 1], 9],
          ["trail ayakkabı", 1_900, 30, "commercial", [6, null], null],
          ["spor çorap", 4_400, 22, "transactional", [2, 5], 14],
          ["sneaker temizleme", 1_300, 11, "informational", [null, 4], 2],
          ["ortopedik ayakkabı", 9_900, 46, "commercial", [8, 3], 12],
          ["yürüyüş ayakkabısı", 12_100, 44, "commercial", [4, 7], 3],
        ].map(([keyword, volume, difficulty, intent, positions, ownPosition]) => {
          const ranks = (positions as (number | null)[]).flatMap((position, index) => (position && competitors[index] ? [ranking(competitors[index], position)] : []));
          const mine = kind === "shared" && ownPosition ? ranking(own, Number(ownPosition)) : null;
          return {
            keyword: String(keyword),
            volume: Number(volume),
            difficulty: Number(difficulty),
            cpc: 0.7,
            intent: String(intent),
            own: mine,
            competitors: ranks.sort((left, right) => left.position - right.position),
            weak: Boolean(mine && ranks.some((rank) => rank.position < mine.position)),
          };
        }),
      };
    case "links":
      return ["haber.com.tr", "kosu-rehberi.com", "moda-blog.net", "forum.sporcular.org"].map((domain, index) => ({
        domain,
        rank: 72 - index * 9,
        backlinks: Object.fromEntries(competitors.map((competitor, position) => [competitor, 12 - index * 2 + position])),
      }));
    case "bulk": {
      const targets = Array.isArray(args.targets) && args.targets.length ? args.targets.map(String) : [own, ...competitors];
      return targets.map((target, index) => ({ target, organic_traffic: 140_000 / (index + 1.4), organic_keywords: 26_000 - index * 4_000, paid_traffic: 4_000 - index * 900, rank: 61 - index * 7 }));
    }
    default:
      return null;
  }
}

// ----- Rank Tracker -----

interface DemoKeyword {
  id: number;
  keyword: string;
  volume: number | null;
  tags: string[];
  /** Typical position; `null` when the site does not rank. */
  base: number | null;
}

interface DemoRank {
  settings: RankSettings;
  trackers: TrackerRow[];
  keywords: DemoKeyword[];
}

const CTR = [0.28, 0.15, 0.1, 0.07, 0.05, 0.04, 0.03, 0.025, 0.02, 0.018];
const ctr = (position: number | null) => (position === null ? 0 : position <= 10 ? CTR[position - 1]! : position <= 20 ? 0.01 : 0);

const tracker = (id: number, device: "desktop" | "mobile", location: string | null): TrackerRow => ({
  id,
  project_id: 1,
  engine: "google",
  country: "tr",
  language: "tr",
  device,
  location,
  created_at: NOW,
});

const rankState: Record<number, DemoRank> = {
  1: {
    settings: { schedule: "daily", depth: 20, ai_overview: true, last_run_at: NOW },
    trackers: [tracker(1, "desktop", null), tracker(2, "mobile", "Istanbul,Istanbul,Turkey")],
    keywords: [
      { id: 1, keyword: "spor ayakkabı", volume: 49_500, tags: ["ayakkabı"], base: 4 },
      { id: 2, keyword: "koşu ayakkabısı", volume: 14_800, tags: ["ayakkabı", "koşu"], base: 7 },
      { id: 3, keyword: "beyaz sneaker", volume: 8_100, tags: ["sneaker"], base: 2 },
      { id: 4, keyword: "kadın spor ayakkabı", volume: 22_200, tags: ["ayakkabı", "kadın"], base: 11 },
      { id: 5, keyword: "erkek bot", volume: 12_100, tags: ["bot"], base: 15 },
      { id: 6, keyword: "su geçirmez bot", volume: 2_900, tags: ["bot"], base: 1 },
      { id: 7, keyword: "çocuk ayakkabısı", volume: 6_600, tags: ["çocuk"], base: 24 },
      { id: 8, keyword: "ayakkabı numarası nasıl ölçülür", volume: 1_900, tags: ["rehber"], base: 3 },
      { id: 9, keyword: "en iyi koşu ayakkabısı 2026", volume: 1_300, tags: ["koşu", "rehber"], base: 9 },
      { id: 10, keyword: "ornekmagaza", volume: 880, tags: ["marka"], base: 1 },
      { id: 11, keyword: "outdoor ayakkabı", volume: 4_400, tags: [], base: 38 },
      { id: 12, keyword: "trekking botu", volume: 3_600, tags: ["bot"], base: null },
    ],
  },
};

function rankOf(projectId: number): DemoRank {
  return (rankState[projectId] ??= { settings: { schedule: "manual", depth: 20, ai_overview: true, last_run_at: null }, trackers: [], keywords: [] });
}

/** A day `back` days ago, `YYYY-MM-DD`. */
function dayBefore(back: number): string {
  return new Date(Date.parse(NOW) - back * 86_400_000).toISOString().slice(0, 10);
}

/** The sample position of a keyword on a day (0 = today) and place. */
function samplePosition(keyword: DemoKeyword, back: number, trackerId: number): number | null {
  if (keyword.base === null) return null;
  const drift = Math.round(2 * Math.sin(back / 4 + keyword.id) + back / 25 - (trackerId === 2 ? -1 : 0));
  const position = keyword.base + drift;
  return position > 100 ? null : Math.max(1, position);
}

const slug = (keyword: string) => keyword.toLowerCase().replace(/[^a-z0-9ğüşıöç]+/g, "-");

function rankRows(state: DemoRank, trackerId: number, days: number): KeywordRank[] {
  const span = Math.min(days, 60);
  return state.keywords.map((keyword) => {
    const history: [string, number | null][] = Array.from({ length: span }, (_, index) => {
      const back = span - 1 - index;
      return [dayBefore(back), samplePosition(keyword, back, trackerId)];
    });
    const position = history.at(-1)?.[1] ?? null;
    const previous = history.at(-2)?.[1] ?? null;
    const ranked = history.map(([, value]) => value).filter((value): value is number => value !== null);
    const informational = /nasıl|en iyi|rehber/.test(keyword.keyword) || keyword.id <= 2;
    const url = position === null ? null : `https://ornekmagaza.com/${slug(keyword.keyword)}`;
    return {
      keyword_id: keyword.id,
      keyword: keyword.keyword,
      tags: keyword.tags,
      volume: keyword.volume,
      position,
      previous,
      change: position !== null && previous !== null ? previous - position : null,
      best: ranked.length ? Math.min(...ranked) : null,
      url,
      cannibalised: keyword.id === 4,
      checked_at: NOW,
      history,
      error: null,
      pages: url ? (keyword.id === 4 ? [[position!, url], [17, "https://ornekmagaza.com/kadin"]] : [[position!, url]]) : [],
      features: informational ? ["ai_overview", "organic", "people_also_ask"] : ["organic", "images", "shopping"],
      ai_overview: informational,
      ai_cited: keyword.id === 8 || keyword.id === 9,
      snippet: keyword.id === 8,
    } satisfies KeywordRank;
  });
}

function visibilityOf(rows: KeywordRank[], index: number): VisibilityPoint {
  let best = 0;
  let got = 0;
  let top3 = 0;
  let top10 = 0;
  let ranked = 0;
  for (const row of rows) {
    const weight = Math.max(1, row.volume ?? 1);
    const position = row.history[index]?.[1] ?? null;
    best += CTR[0]! * weight;
    got += ctr(position) * weight;
    if (position !== null) ranked += 1;
    if (position !== null && position <= 3) top3 += 1;
    if (position !== null && position <= 10) top10 += 1;
  }
  return { date: rows[0]?.history[index]?.[0] ?? NOW.slice(0, 10), visibility: best ? Math.round((got / best) * 1000) / 10 : 0, top3, top10, ranked };
}

function rankReport(projectId: number, trackerId: number | null, days: number): RankReport {
  const state = rankOf(projectId);
  const chosen = state.trackers.find((entry) => entry.id === trackerId) ?? state.trackers[0] ?? null;
  const rows = chosen ? rankRows(state, chosen.id, days) : [];
  const span = rows[0]?.history.length ?? 0;
  const visibility = Array.from({ length: span }, (_, index) => visibilityOf(rows, index));
  const last = visibility.at(-1);
  const before = visibility.at(-2);
  const own = projects.find((row) => row.id === projectId)?.domain ?? "example.com";
  const competitors = projects.find((row) => row.id === projectId)?.competitors ?? [];
  const factors = [1.32, 0.54, 0.4];
  const competitorShares: CompetitorShare[] = [
    { domain: own, visibility: last?.visibility ?? 0, keywords: last?.ranked ?? 0, average_position: 8.4, own: true },
    ...competitors.map((domain, index) => ({ domain, visibility: Math.round((last?.visibility ?? 0) * (factors[index] ?? 0.3) * 10) / 10, keywords: 9 - index * 3, average_position: 6.1 + index * 4, own: false })),
  ];
  const positions = rows.map((row) => row.position).filter((value): value is number => value !== null);
  const count = (test: (row: KeywordRank) => boolean) => rows.filter(test).length;
  return {
    tracker: chosen,
    rows,
    summary: {
      visibility: last?.visibility ?? null,
      visibility_change: last && before ? Math.round((last.visibility - before.visibility) * 10) / 10 : null,
      average_position: positions.length ? Math.round((positions.reduce((sum, value) => sum + value, 0) / positions.length) * 10) / 10 : null,
      top3: last?.top3 ?? 0,
      top10: last?.top10 ?? 0,
      ranked: last?.ranked ?? 0,
      checked: rows.length,
      improved: count((row) => (row.change ?? 0) > 0),
      declined: count((row) => (row.change ?? 0) < 0),
      lost: count((row) => row.position === null && row.previous !== null),
      ai_overviews: count((row) => row.ai_overview),
      ai_cited: count((row) => row.ai_cited),
      snippets: count((row) => row.snippet),
      cannibalised: count((row) => row.cannibalised),
    },
    visibility,
    competitors: competitorShares,
    share_history: visibility.map((point) => ({
      date: point.date,
      shares: Object.fromEntries(competitorShares.map((share, index) => [share.domain, index === 0 ? point.visibility : Math.round(point.visibility * (factors[index - 1] ?? 0.3) * 10) / 10])),
    })),
    last_run: rankRuns[0] ?? null,
    since: dayBefore(days),
  };
}

const rankRuns: RankRun[] = [
  { id: 14, project_id: 1, scheduled: true, status: "done", total: 24, checked: 24, failed: 0, cost_usd: 0.144, error: null, started_at: NOW, finished_at: NOW },
  { id: 13, project_id: 1, scheduled: false, status: "partial", total: 24, checked: 23, failed: 1, cost_usd: 0.138, error: null, started_at: dayBefore(1) + "T09:12:00Z", finished_at: dayBefore(1) + "T09:13:10Z" },
  { id: 12, project_id: 1, scheduled: true, status: "done", total: 24, checked: 24, failed: 0, cost_usd: 0.144, error: null, started_at: dayBefore(2) + "T08:00:00Z", finished_at: dayBefore(2) + "T08:01:05Z" },
];

function rankSetup(projectId: number): RankSetup {
  const state = rankOf(projectId);
  return {
    settings: state.settings,
    trackers: state.trackers,
    keywords: state.keywords.length,
    missing_volumes: state.keywords.filter((keyword) => keyword.volume === null).length,
    next_check_at: state.settings.schedule === "manual" ? null : new Date(Date.parse(NOW) + 20 * 3_600_000).toISOString().replace(/\.\d+Z$/, "Z"),
  };
}

function rankKeyword(projectId: number, keywordId: number, trackerId: number): KeywordDetail {
  const state = rankOf(projectId);
  const chosen = state.trackers.find((entry) => entry.id === trackerId) ?? state.trackers[0]!;
  const row = rankRows(state, chosen.id, 30).find((entry) => entry.keyword_id === keywordId)!;
  const own = projects.find((entry) => entry.id === projectId)?.domain ?? "example.com";
  const others = ["rakip-magaza.com", "digerdukkan.com.tr", "trendyol.com", "hepsiburada.com", "n11.com", "decathlon.com.tr", "nike.com", "adidas.com.tr", "sportive.com.tr", "superstep.com.tr"];
  const results: CheckResult[] = [];
  let other = 0;
  for (let position = 1; position <= 10; position += 1) {
    if (position === row.position && row.url) results.push({ position, domain: own, url: row.url, title: `${row.keyword} — Örnek Mağaza` });
    else {
      const domain = others[other++ % others.length]!;
      results.push({ position, domain, url: `https://${domain}/${slug(row.keyword)}`, title: `${row.keyword} | ${domain}` });
    }
  }
  return {
    keyword: { id: row.keyword_id, project_id: projectId, keyword: row.keyword, tags: row.tags, volume: row.volume, added_at: NOW },
    tracker: chosen,
    checks: row.history.map(([date, position]) => ({
      checked_at: `${date}T08:00:00Z`,
      position,
      url: row.url,
      pages: row.pages,
      features: row.features,
      ai_overview: row.ai_overview,
      ai_cited: row.ai_cited,
      snippet: row.snippet,
      error: null,
    })),
    results,
  };
}

// ----- Site Audit -----

let auditSettings: Record<number, AuditSettings> = { 1: { schedule: "weekly", max_pages: 1000, last_run_at: NOW } };

let audits: AuditRow[] = [
  { id: 2, project_id: 1, crawl_id: "20261021-070000-ornekmagaza-com", scheduled: true, status: "done", start_url: "https://ornekmagaza.com/", pages: 842, health: 78.4, geo: 61.5, issues: 1_264, metered_pages: 0, charged_micros: 0, error: null, started_at: NOW, finished_at: NOW },
  { id: 1, project_id: 1, crawl_id: "20261014-070000-ornekmagaza-com", scheduled: true, status: "done", start_url: "https://ornekmagaza.com/", pages: 815, health: 74.9, geo: 52.0, issues: 1_402, metered_pages: 0, charged_micros: 0, error: null, started_at: dayBefore(7) + "T07:00:00Z", finished_at: dayBefore(7) + "T07:04:00Z" },
];

/** Sample checks: id, category, kind, priority, Turkish and English titles, count now and before. */
const SAMPLE_ISSUES: [string, string, IssueKind, IssuePriority, string, string, number, number][] = [
  ["response-4xx", "response-codes", "issue", "high", "İstemci hatası (4xx)", "Client error (4xx)", 14, 21],
  ["redirect-chain", "response-codes", "issue", "medium", "Yönlendirme zinciri", "Redirect chain", 6, 6],
  ["title-missing", "titles", "issue", "high", "Başlık (title) yok", "Missing title", 3, 9],
  ["title-too-long", "titles", "warning", "low", "Başlık çok uzun", "Title too long", 57, 61],
  ["description-missing", "meta-descriptions", "warning", "medium", "Meta açıklama yok", "Missing meta description", 112, 140],
  ["h1-missing", "headings", "issue", "medium", "H1 yok", "Missing H1", 8, 8],
  ["content-thin", "content", "warning", "medium", "Az içerik", "Thin content", 96, 103],
  ["image-alt-missing", "images", "warning", "low", "Alt metni olmayan görsel", "Images without alt text", 431, 455],
  ["canonical-missing", "canonicals", "warning", "low", "Canonical yok", "Missing canonical", 0, 12],
  ["structured-data-missing", "structured-data", "opportunity", "low", "Yapılandırılmış veri yok", "No structured data", 402, 420],
  ["ai-training-allowed", "ai", "opportunity", "low", "AI eğitim botlarına izin var", "AI training crawlers allowed", 1, 1],
  ["llms-txt-missing", "ai", "opportunity", "low", "llms.txt yok", "No llms.txt", 1, 1],
];

const CATEGORY_NAMES: Record<string, [string, string]> = {
  "response-codes": ["Yanıt kodları", "Response codes"],
  titles: ["Sayfa başlıkları", "Page titles"],
  "meta-descriptions": ["Meta açıklamalar", "Meta descriptions"],
  headings: ["Başlıklar", "Headings"],
  content: ["İçerik", "Content"],
  images: ["Görseller", "Images"],
  canonicals: ["Canonical", "Canonicals"],
  "structured-data": ["Yapılandırılmış veri", "Structured data"],
  ai: ["AI hazırlığı", "AI readiness"],
};

const sampleGeo: GeoReadiness = {
  score: 61.5,
  pages: 512,
  robots: "found",
  signals: [
    { id: "ai-search-access", weight: 25, value: 0.75, status: "warning", passed: 3, total: 4, examples: ["PerplexityBot"] },
    { id: "snippets-allowed", weight: 15, value: 0.98, status: "warning", passed: 502, total: 512, examples: ["https://ornekmagaza.com/kampanya", "https://ornekmagaza.com/uyelik"] },
    { id: "structured-data", weight: 15, value: 0.21, status: "poor", passed: 110, total: 512, examples: ["https://ornekmagaza.com/hakkimizda", "https://ornekmagaza.com/iletisim"] },
    { id: "ai-assistant-access", weight: 10, value: 1, status: "good", passed: 3, total: 3, examples: [] },
    { id: "entity-schema", weight: 10, value: 0.5, status: "warning", passed: 1, total: 2, examples: ["WebSite"] },
    { id: "question-headings", weight: 10, value: 0.34, status: "warning", passed: 74, total: 218, examples: ["https://ornekmagaza.com/blog/kosu-ayakkabisi-secimi"] },
    { id: "substantial-content", weight: 10, value: 0.43, status: "warning", passed: 218, total: 512, examples: ["https://ornekmagaza.com/kategori/bot"] },
    { id: "llms-txt", weight: 5, value: 0, status: "poor", passed: 0, total: 1, examples: ["https://ornekmagaza.com/llms.txt"] },
  ],
  crawlers: [
    { id: "oai-searchbot", name: "OAI-SearchBot", operator: "OpenAI", purpose: "search", allowed: true, rule: null },
    { id: "perplexitybot", name: "PerplexityBot", operator: "Perplexity", purpose: "search", allowed: false, rule: "disallow: /" },
    { id: "claude-searchbot", name: "Claude-SearchBot", operator: "Anthropic", purpose: "search", allowed: true, rule: null },
    { id: "chatgpt-user", name: "ChatGPT-User", operator: "OpenAI", purpose: "assistant", allowed: true, rule: null },
    { id: "gptbot", name: "GPTBot", operator: "OpenAI", purpose: "training", allowed: true, rule: null },
  ],
  schema_types: [
    ["Product", 96],
    ["BreadcrumbList", 88],
    ["Organization", 1],
  ],
};

function auditReport(projectId: number, auditId: number | null, lang: string): AuditReport | null {
  const mine = audits.filter((row) => row.project_id === projectId);
  const audit = mine.find((row) => row.id === auditId) ?? mine[0];
  if (!audit) return null;
  const previous = mine.find((row) => row.id < audit.id && row.status === "done") ?? null;
  const tr = lang === "tr";
  const latest = audit.id === mine[0]?.id;
  const categories: CategoryReport[] = [];
  for (const [id, category, kind, priority, trTitle, enTitle, now, before] of SAMPLE_ISSUES) {
    const count = latest ? now : before;
    const earlier = previous ? before : null;
    if (count === 0 && !earlier) continue;
    let group = categories.find((entry) => entry.id === category);
    if (!group) {
      const names = CATEGORY_NAMES[category] ?? [category, category];
      group = { id: category, name: tr ? names[0] : names[1], issues: [] };
      categories.push(group);
    }
    group.issues.push({ id, title: tr ? trTitle : enTitle, kind, priority, count, previous: earlier });
  }
  return {
    audit,
    previous,
    overview: { health: audit.health, internal: audit.pages, external: 0, indexable: 512, html: 604, blocked: 3, errors: 14, redirects: 22, average_response_ms: 312, affected_urls: 588 },
    categories,
    geo: latest ? sampleGeo : { ...sampleGeo, score: audit.geo },
  };
}


// ----- AI visibility -------------------------------------------------------------------------

const AI_ENGINES: AiEngine[] = ["chatgpt", "claude", "gemini", "perplexity", "google_ai_mode"];
const AI_MODELS: Record<AiEngine, string> = { chatgpt: "gpt-5", claude: "claude-opus-5", gemini: "gemini-2.5-flash", perplexity: "sonar", google_ai_mode: "google-ai-mode" };
const AI_USD: Record<AiEngine, number> = { chatgpt: 0.0306, claude: 0.0406, gemini: 0.0206, perplexity: 0.0106, google_ai_mode: 0.004 };
/** How often each assistant names the sample brand in the latest run. */
const AI_REACH: Record<AiEngine, number> = { chatgpt: 0.55, claude: 0.42, gemini: 0.36, perplexity: 0.6, google_ai_mode: 0.48 };

const SAMPLE_PROMPTS: [string, string[]][] = [
  ["En iyi spor ayakkabı markaları hangileri?", ["kategori"]],
  ["Koşu ayakkabısı nereden alınır?", ["kategori"]],
  ["Örnek Mağaza güvenilir mi?", ["marka"]],
  ["Örnek Mağaza yorumları nasıl?", ["marka"]],
  ["Uygun fiyatlı sneaker önerileri neler?", ["kategori"]],
  ["Kışlık bot için en iyi online mağaza hangisi?", ["sezon"]],
  ["Çocuk spor ayakkabısı alırken nelere dikkat edilmeli?", ["bilgi"]],
  ["Rakip Mağaza mı Örnek Mağaza mı?", ["karşılaştırma"]],
];

const SAMPLE_SOURCES: [string, string][] = [
  ["https://ornekmagaza.com/spor-ayakkabi", "Spor Ayakkabı Modelleri | Örnek Mağaza"],
  ["https://rakip-magaza.com/ayakkabi", "Ayakkabı | Rakip Mağaza"],
  ["https://tr.wikipedia.org/wiki/Spor_ayakkab%C4%B1s%C4%B1", "Spor ayakkabısı - Vikipedi"],
  ["https://kosu-dergisi.com/en-iyi-kosu-ayakkabilari", "En iyi koşu ayakkabıları"],
  ["https://alisveris-rehberi.com/online-ayakkabi-magazalari", "Online ayakkabı mağazaları rehberi"],
  ["https://ornekmagaza.com/kosu-ayakkabisi", "Koşu Ayakkabısı | Örnek Mağaza"],
  ["https://forum-sporcu.com/konu/ayakkabi-onerisi", "Ayakkabı önerisi - Sporcu Forumu"],
  ["https://digerdukkan.com.tr/kampanya", "Kampanyalar | Diğer Dükkan"],
];

interface DemoAi {
  settings: AiSettings;
  prompts: AiPrompt[];
}

const aiState: Record<number, DemoAi> = {
  1: {
    settings: {
      schedule: "weekly",
      engines: [...AI_ENGINES],
      models: {},
      rivals: [
        { domain: "rakip-magaza.com", names: ["Rakip Mağaza"] },
        { domain: "digerdukkan.com.tr", names: ["Diğer Dükkan"] },
      ],
      last_run_at: NOW,
    },
    prompts: SAMPLE_PROMPTS.map(([prompt, tags], index) => ({ id: index + 1, project_id: 1, prompt, tags, added_at: NOW })),
  },
};

function aiOf(projectId: number): DemoAi {
  return (aiState[projectId] ??= { settings: { schedule: "manual", engines: [...AI_ENGINES], models: {}, rivals: [], last_run_at: null }, prompts: [] });
}

function brandOf(domain: string, names: string[], own: boolean): Brand {
  const all = [...names];
  for (const extra of [domain, domain.split(".")[0]!]) if (extra.length >= 3 && !all.some((name) => name.toLowerCase() === extra.toLowerCase())) all.push(extra);
  return { key: domain, names: all, own };
}

function aiBrands(projectId: number): Brand[] {
  const project = projects.find((row) => row.id === projectId);
  if (!project) return [];
  const { settings } = aiOf(projectId);
  const rivals: Rival[] = settings.rivals.length ? settings.rivals : project.competitors.map((domain) => ({ domain, names: [] }));
  return [brandOf(project.domain, project.brands, true), ...rivals.map((rival) => brandOf(rival.domain, rival.names, false))];
}

/** A stable number in [0, 1) for a seed. */
function chance(seed: string): number {
  let hash = 2166136261;
  for (const char of seed) hash = Math.imul(hash ^ char.charCodeAt(0), 16777619);
  return ((hash >>> 0) % 10_000) / 10_000;
}

interface SampleAnswer extends AiCell {
  brands: AnswerBrand[];
}

/** The sample answer of a question on an assistant, `back` weekly runs ago. */
function sampleAnswer(prompt: AiPrompt, engine: AiEngine, back: number, brands: Brand[]): SampleAnswer {
  const seed = `${prompt.id}-${engine}-${back}`;
  const checked_at = new Date(Date.parse(NOW) - back * 7 * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z");
  if (back === 0 && engine === "gemini" && prompt.id === 6) {
    return { mentioned: false, position: null, cited: false, sentiment: null, error: "HTTP 503: the model is overloaded", checked_at, brands: [] };
  }
  const reach = AI_REACH[engine] - back * 0.04 + (prompt.prompt.includes("Örnek Mağaza") ? 0.3 : 0);
  const mentioned = chance(seed) < reach;
  const named = brands.filter((brand) => (brand.own ? mentioned : chance(`${seed}-${brand.key}`) < 0.6));
  const order = [...named].sort((left, right) => chance(`${seed}-o-${left.key}`) - chance(`${seed}-o-${right.key}`));
  const cited = mentioned && chance(`${seed}-c`) < (engine === "perplexity" || engine === "google_ai_mode" ? 0.7 : 0.45);
  const sentiment = mentioned ? Math.round((chance(`${seed}-s`) * 1.1 - 0.25) * 100) / 100 : null;
  const answerBrands: AnswerBrand[] = order.map((brand, index) => ({
    key: brand.key,
    own: brand.own,
    position: index + 1,
    cited: brand.own ? cited : chance(`${seed}-rc-${brand.key}`) < 0.4,
    sentiment: brand.own ? sentiment : 0.2,
  }));
  const own = answerBrands.find((brand) => brand.own);
  return { mentioned, position: own?.position ?? null, cited, sentiment, error: null, checked_at, brands: answerBrands };
}

function visibilityOfAnswers(answers: SampleAnswer[]): Visibility {
  const ok = answers.filter((answer) => !answer.error);
  const round = (value: number) => Math.round(value * 10) / 10;
  const mentioned = ok.filter((answer) => answer.mentioned).length;
  const cited = ok.filter((answer) => answer.cited).length;
  const positions = ok.flatMap((answer) => (answer.position === null ? [] : [answer.position]));
  const tones = ok.flatMap((answer) => (answer.sentiment === null ? [] : [answer.sentiment]));
  const all = ok.reduce((sum, answer) => sum + answer.brands.length, 0);
  return {
    answers: ok.length,
    mentioned,
    cited,
    mention_rate: ok.length ? round((mentioned / ok.length) * 100) : 0,
    citation_rate: ok.length ? round((cited / ok.length) * 100) : 0,
    average_position: positions.length ? round(positions.reduce((sum, value) => sum + value, 0) / positions.length) : null,
    sentiment: tones.length ? Math.round((tones.reduce((sum, value) => sum + value, 0) / tones.length) * 100) / 100 : null,
    share_of_voice: all ? round((mentioned / all) * 100) : 0,
  };
}

function aiReport(projectId: number, days: number): AiReport {
  const state = aiOf(projectId);
  const brands = aiBrands(projectId);
  const engines = state.settings.engines;
  const runs = state.prompts.length ? Math.min(6, Math.floor(days / 7)) : 0;
  const answersOf = (back: number) => state.prompts.flatMap((prompt) => engines.map((engine) => ({ prompt, engine, answer: sampleAnswer(prompt, engine, back, brands) })));
  const latest = runs ? answersOf(0) : [];
  const ok = latest.filter((entry) => !entry.answer.error);
  const allMentions = ok.reduce((sum, entry) => sum + entry.answer.brands.length, 0);
  const brandShares: BrandShare[] = brands.map((brand) => {
    const hits = ok.flatMap((entry) => entry.answer.brands.filter((hit) => hit.key === brand.key));
    const positions = hits.flatMap((hit) => (hit.position === null ? [] : [hit.position]));
    return {
      key: brand.key,
      own: brand.own,
      mentions: hits.length,
      citations: hits.filter((hit) => hit.cited).length,
      share: allMentions ? Math.round((hits.length / allMentions) * 1000) / 10 : 0,
      average_position: positions.length ? Math.round((positions.reduce((sum, value) => sum + value, 0) / positions.length) * 10) / 10 : null,
    };
  });
  const history: AiRunPoint[] = Array.from({ length: runs }, (_, index) => {
    const back = runs - 1 - index;
    const rows = answersOf(back);
    const overall = visibilityOfAnswers(rows.map((entry) => entry.answer));
    return {
      run_id: 100 - back,
      date: rows[0]?.answer.checked_at.slice(0, 10) ?? NOW.slice(0, 10),
      mention_rate: overall.mention_rate,
      citation_rate: overall.citation_rate,
      share_of_voice: overall.share_of_voice,
      engines: Object.fromEntries(engines.map((engine) => [engine, visibilityOfAnswers(rows.filter((entry) => entry.engine === engine).map((entry) => entry.answer)).mention_rate])),
    };
  });
  const prompts: AiPromptRow[] = state.prompts.map((prompt) => ({
    prompt_id: prompt.id,
    prompt: prompt.prompt,
    tags: prompt.tags,
    results: Object.fromEntries(
      latest.filter((entry) => entry.prompt.id === prompt.id).map((entry) => {
        const { brands: _, ...cell } = entry.answer;
        return [entry.engine, cell];
      }),
    ),
  }));
  const citedAnswers = ok.filter((entry) => entry.answer.cited).length;
  return {
    summary: visibilityOfAnswers(latest.map((entry) => entry.answer)),
    engines: engines.map((engine): EngineVisibility => ({ engine, ...visibilityOfAnswers(latest.filter((entry) => entry.engine === engine).map((entry) => entry.answer)) })),
    brands: brandShares.sort((left, right) => Number(right.own) - Number(left.own) || right.mentions - left.mentions),
    history,
    prompts,
    top_domains: runs
      ? [
          { key: "rakip-magaza.com", title: null, own: false, answers: Math.round(ok.length * 0.42) },
          { key: "ornekmagaza.com", title: null, own: true, answers: citedAnswers },
          { key: "tr.wikipedia.org", title: null, own: false, answers: Math.round(ok.length * 0.3) },
          { key: "kosu-dergisi.com", title: null, own: false, answers: Math.round(ok.length * 0.22) },
          { key: "alisveris-rehberi.com", title: null, own: false, answers: Math.round(ok.length * 0.18) },
          { key: "forum-sporcu.com", title: null, own: false, answers: Math.round(ok.length * 0.12) },
          { key: "digerdukkan.com.tr", title: null, own: false, answers: Math.round(ok.length * 0.08) },
        ].sort((left, right) => right.answers - left.answers)
      : [],
    own_pages: runs
      ? [
          { key: "https://ornekmagaza.com/spor-ayakkabi", title: "Spor Ayakkabı Modelleri | Örnek Mağaza", own: true, answers: Math.ceil(citedAnswers * 0.6) },
          { key: "https://ornekmagaza.com/kosu-ayakkabisi", title: "Koşu Ayakkabısı | Örnek Mağaza", own: true, answers: Math.ceil(citedAnswers * 0.3) },
          { key: "https://ornekmagaza.com/blog/ayakkabi-numarasi-secimi", title: "Ayakkabı numarası nasıl seçilir?", own: true, answers: Math.max(1, Math.floor(citedAnswers * 0.1)) },
        ]
      : [],
    last_run: runs ? aiRuns[0]! : null,
    since: new Date(Date.parse(NOW) - days * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z"),
  };
}

const aiRuns: AiRun[] = [
  { id: 100, project_id: 1, scheduled: true, status: "partial", total: 40, checked: 39, failed: 1, cost_usd: 0.86, error: null, started_at: NOW, finished_at: NOW },
  {
    id: 99,
    project_id: 1,
    scheduled: true,
    status: "done",
    total: 40,
    checked: 40,
    failed: 0,
    cost_usd: 0.88,
    error: null,
    started_at: new Date(Date.parse(NOW) - 7 * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z"),
    finished_at: null,
  },
];

function sampleText(prompt: AiPrompt, answer: SampleAnswer, brands: Brand[]): string {
  const named = answer.brands.map((hit) => brands.find((brand) => brand.key === hit.key)?.names[0] ?? hit.key);
  if (named.length === 0) return `"${prompt.prompt}" sorusu için tek bir mağaza öne çıkmıyor. Ürün yorumlarını, iade koşullarını ve kargo süresini karşılaştırmanız önerilir.`;
  const lines = named.map((name, index) => {
    const own = answer.brands[index]?.own;
    return `${index + 1}. ${name}: ${own ? (answer.sentiment !== null && answer.sentiment < 0 ? "fiyatları uygun ama kargo gecikmesi yorumları var." : "geniş model seçeneği, hızlı kargo ve olumlu kullanıcı yorumlarıyla öne çıkıyor.") : "kampanyaları ve bilinen markalarıyla popüler."}`;
  });
  return `"${prompt.prompt}" için Türkiye'de sık önerilen seçenekler:\n\n${lines.join("\n")}\n\nSeçim yaparken numara rehberine ve iade koşullarına bakmanızı öneririm.`;
}

function aiAnswer(projectId: number, promptId: number, engine: AiEngine): AnswerDetail {
  const state = aiOf(projectId);
  const brands = aiBrands(projectId);
  const prompt = state.prompts.find((row) => row.id === promptId) ?? state.prompts[0]!;
  const answers: AiAnswer[] = [0, 1, 2].map((back) => {
    const sample = sampleAnswer(prompt, engine, back, brands);
    const citations = SAMPLE_SOURCES.filter(([url], index) => (url.includes("ornekmagaza") ? sample.cited : chance(`${prompt.id}-${engine}-${back}-${index}`) < 0.4)).map(([url, title]) => ({
      url,
      title,
      domain: new URL(url).hostname,
    }));
    return {
      run_id: 100 - back,
      prompt_id: prompt.id,
      engine,
      via: "credit",
      model: AI_MODELS[engine],
      checked_at: sample.checked_at,
      text: sample.error ? "" : sampleText(prompt, sample, brands),
      mentioned: sample.mentioned,
      position: sample.position,
      cited: sample.cited,
      sentiment: sample.sentiment,
      brands: sample.brands,
      citations: sample.error ? [] : citations,
      error: sample.error,
    };
  });
  return { prompt, engine, brands, answers };
}

function aiSetup(projectId: number): AiSetup {
  const state = aiOf(projectId);
  return {
    settings: state.settings,
    prompts: state.prompts,
    engines: AI_ENGINES.map((engine) => ({
      engine,
      enabled: state.settings.engines.includes(engine),
      via: "credit",
      model: state.settings.models[engine] ?? AI_MODELS[engine],
      provider: engine === "google_ai_mode" ? null : { chatgpt: "openai", claude: "anthropic", gemini: "google-gemini", perplexity: "perplexity" }[engine],
    })),
    brands: aiBrands(projectId),
    next_run_at: state.settings.schedule === "manual" || state.prompts.length === 0 ? null : new Date(Date.parse(NOW) + (state.settings.schedule === "weekly" ? 6 : 28) * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z"),
  };
}

function aiEstimate(projectId: number): AiEstimate {
  const state = aiOf(projectId);
  const count = state.prompts.length;
  const engines = state.settings.engines.map((engine) => ({ engine, via: "credit" as const, model: state.settings.models[engine] ?? AI_MODELS[engine], answers: count, usd: AI_USD[engine] * count, credits: AI_USD[engine] * count * 1350 }));
  const usd = engines.reduce((sum, engine) => sum + engine.usd, 0);
  const runs = state.settings.schedule === "weekly" ? 4 : state.settings.schedule === "monthly" ? 1 : 0;
  return { engines, answers: count * engines.length, usd, credits: usd * 1350, runs_per_month: runs, month_usd: usd * runs, month_credits: usd * 1350 * runs, unavailable: [] };
}

const sampleMetrics = (target: string, mentions: number): MentionMetrics => ({
  target,
  mentions,
  ai_search_volume: mentions * 410,
  platforms: [
    { key: "google", mentions: Math.round(mentions * 0.8), ai_search_volume: mentions * 330 },
    { key: "chat_gpt", mentions: Math.round(mentions * 0.2), ai_search_volume: mentions * 80 },
  ],
  sources: [
    { key: target, mentions: Math.round(mentions * 0.6), ai_search_volume: mentions * 250 },
    { key: "tr.wikipedia.org", mentions: Math.round(mentions * 0.3), ai_search_volume: mentions * 120 },
    { key: "kosu-dergisi.com", mentions: Math.round(mentions * 0.2), ai_search_volume: mentions * 90 },
  ],
  brands: [],
});

const SAMPLE_MENTIONS: [string, string, number][] = [
  ["en iyi spor ayakkabı markaları", "google", 6600],
  ["online ayakkabı nereden alınır", "google", 2900],
  ["örnek mağaza güvenilir mi", "chat_gpt", 880],
  ["koşu ayakkabısı önerisi", "google", 1900],
  ["kışlık bot hangi mağazadan alınır", "google", 1300],
  ["uygun fiyatlı sneaker", "chat_gpt", 720],
];

function mentionsReport(projectId: number, kind: string): unknown {
  const brands = aiBrands(projectId);
  const domain = brands.find((brand) => brand.own)?.key ?? "example.com";
  switch (kind) {
    case "queries":
      return SAMPLE_MENTIONS.map(([question, platform, volume], index): MentionRow => ({
        platform,
        question,
        answer: `${question}: … ${domain} …`,
        sources: SAMPLE_SOURCES.slice(index % 3, (index % 3) + 3).map(([url, title]) => ({ domain: new URL(url).hostname, url, title })),
        ai_search_volume: volume,
        last_response_at: `${NOW.slice(0, 10)} 08:00:00 +00:00`,
      }));
    case "compare":
      return brands.slice(0, 5).map((brand, index) => sampleMetrics(brand.key, [42, 57, 18, 9, 4][index] ?? 3));
    default:
      return sampleMetrics(domain, 42);
  }
}


// ----- Search Console --------------------------------------------------------------------------

const googleStatus: GoogleStatus = { configured: true, own_client_id: null, built_in: true, connected: true, search_console: true, analytics: true };

const gscSettings: Record<number, GscSettings> = {
  1: { property: "sc-domain:ornekmagaza.com", ga4_property: "properties/312345678", ga4_name: "Örnek Mağaza", days: 28, schedule: "weekly", last_sync_at: NOW },
};

function gscSettingsOf(projectId: number): GscSettings {
  return (gscSettings[projectId] ??= { property: null, ga4_property: null, ga4_name: null, days: 28, schedule: "weekly", last_sync_at: null });
}

const isoDay = (back: number) => new Date(Date.parse(NOW) - back * 86_400_000).toISOString().slice(0, 10);

const gscSyncs: GscSync[] = [
  { id: 12, project_id: 1, scheduled: true, status: "done", period_start: isoDay(30), period_end: isoDay(3), rows: 18_420, currency: "TRY", limited: false, error: null, started_at: NOW, finished_at: NOW },
  {
    id: 11,
    project_id: 1,
    scheduled: true,
    status: "done",
    period_start: isoDay(37),
    period_end: isoDay(10),
    rows: 17_905,
    currency: "TRY",
    limited: false,
    error: null,
    started_at: new Date(Date.parse(NOW) - 7 * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z"),
    finished_at: null,
  },
];

const SAMPLE_QUERIES: [string, number, number, number, number, number, boolean][] = [
  // query, clicks, impressions, position, clicks before, position before, tracked
  ["örnek mağaza", 1_840, 4_200, 1.1, 1_710, 1.1, true],
  ["spor ayakkabı", 620, 38_500, 6.8, 540, 7.9, true],
  ["koşu ayakkabısı", 410, 21_300, 5.2, 450, 4.6, true],
  ["beyaz sneaker", 288, 9_800, 4.1, 190, 5.8, false],
  ["kışlık bot", 240, 15_600, 8.4, 120, 11.2, true],
  ["çocuk spor ayakkabı", 156, 12_400, 9.7, 160, 9.5, false],
  ["örnek mağaza iade", 142, 980, 1.3, 120, 1.2, false],
  ["trail koşu ayakkabısı", 98, 6_100, 11.4, 60, 14.0, false],
  ["deri bot erkek", 86, 7_400, 12.8, 101, 12.1, true],
  ["sneaker indirim", 74, 5_900, 7.5, 40, 9.1, false],
  ["yürüyüş ayakkabısı kadın", 63, 8_200, 14.6, 70, 13.9, false],
  ["örnek mağaza kargo", 58, 310, 1.0, 55, 1.0, false],
  ["ayakkabı numarası nasıl seçilir", 51, 4_100, 6.1, 22, 8.4, false],
  ["rakip mağaza ayakkabı", 12, 2_300, 18.2, 15, 17.5, false],
];

function gscReport(projectId: number): GscReport | null {
  const settings = gscSettingsOf(projectId);
  if (!settings.property || projectId !== 1) return null;
  const days = settings.days;
  const daily: GscDay[] = Array.from({ length: days * 2 }, (_, index) => {
    const back = days * 2 + 2 - index;
    const weekday = (index % 7) / 6;
    const now = index >= days;
    const clicks = Math.round((now ? 160 : 135) + 30 * Math.sin(index / 3) - 25 * weekday + (index % 5) * 4);
    return {
      date: isoDay(back),
      period: now ? "now" : "before",
      clicks,
      impressions: clicks * 38 + (index % 7) * 120,
      position: Math.round((now ? 7.4 : 8.1) * 10 - (index % 4)) / 10,
      sessions: Math.round(clicks * 1.22),
      key_events: Math.round(clicks * 0.04),
    };
  });
  const sum = (period: "now" | "before", pick: (day: GscDay) => number) => daily.filter((day) => day.period === period).reduce((total, day) => total + pick(day), 0);
  const totals = (period: "now" | "before") => {
    const clicks = sum(period, (day) => day.clicks);
    const impressions = sum(period, (day) => day.impressions);
    return { clicks, impressions, ctr: Math.round((clicks / impressions) * 1000) / 10, position: period === "now" ? 7.4 : 8.1 };
  };
  const queries: GscQuery[] = SAMPLE_QUERIES.map(([query, clicks, impressions, position, before, positionBefore, tracked]) => ({
    query,
    clicks,
    impressions,
    ctr: Math.round((clicks / impressions) * 1000) / 10,
    position,
    clicks_change: clicks - before,
    position_change: Math.round((positionBefore - position) * 10) / 10,
    pages: query === "spor ayakkabı" || query === "beyaz sneaker" ? 2 : 1,
    tracked,
    near_top: position >= 3.5 && position <= 20 && impressions >= 2_000,
    low_ctr: query === "kışlık bot" || query === "çocuk spor ayakkabı",
  }));
  const page = (path: string, clicks: number, impressions: number, position: number, before: number, sessions: number, events: number, revenue: number, audit: GscPage["audit"]): GscPage => ({
    page: `https://ornekmagaza.com${path}`,
    clicks,
    impressions,
    ctr: Math.round((clicks / impressions) * 1000) / 10,
    position,
    clicks_change: clicks - before,
    sessions,
    key_events: events,
    revenue,
    audit,
  });
  const ok = { status: 200, indexability: "indexable", findings: 0 };
  return {
    sync: gscSyncs[0]!,
    period: [isoDay(days + 2), isoDay(3)],
    previous: [isoDay(days * 2 + 2), isoDay(days + 3)],
    now: totals("now"),
    before: totals("before"),
    traffic_now: { sessions: Math.round(sum("now", (day) => day.sessions ?? 0)), engaged_sessions: Math.round(sum("now", (day) => day.sessions ?? 0) * 0.64), key_events: sum("now", (day) => day.key_events ?? 0), revenue: 184_500 },
    traffic_before: { sessions: Math.round(sum("before", (day) => day.sessions ?? 0)), engaged_sessions: Math.round(sum("before", (day) => day.sessions ?? 0) * 0.61), key_events: sum("before", (day) => day.key_events ?? 0), revenue: 151_200 },
    currency: "TRY",
    daily,
    queries,
    pages: [
      page("/", 2_120, 6_900, 1.4, 1_980, 2_610, 96, 61_200, ok),
      page("/spor-ayakkabi", 940, 52_300, 6.5, 820, 1_150, 41, 48_700, { status: 200, indexability: "indexable", findings: 2 }),
      page("/kosu-ayakkabisi", 455, 24_800, 5.3, 490, 560, 22, 24_300, ok),
      page("/kislik-bot", 251, 16_900, 8.2, 128, 310, 11, 18_900, ok),
      page("/blog/ayakkabi-numarasi-secimi", 188, 9_400, 5.9, 95, 240, 3, 0, ok),
      page("/sneaker/beyaz", 176, 7_100, 4.4, 120, 205, 9, 9_800, ok),
      page("/cocuk-spor-ayakkabi", 156, 12_400, 9.7, 160, 190, 6, 7_400, { status: 200, indexability: "noindex", findings: 1 }),
      page("/kampanya/sneaker", 74, 5_900, 7.5, 40, 92, 2, 3_100, { status: 301, indexability: "non-indexable", findings: 0 }),
      page("/iade", 142, 980, 1.3, 120, 160, 0, 0, ok),
    ],
    devices: [
      { key: "MOBILE", clicks: 3_310, impressions: 98_000, position: 7.9 },
      { key: "DESKTOP", clicks: 1_120, impressions: 36_000, position: 6.4 },
      { key: "TABLET", clicks: 88, impressions: 2_400, position: 8.8 },
    ],
    countries: [
      { key: "tur", clicks: 4_210, impressions: 126_000, position: 7.2 },
      { key: "deu", clicks: 160, impressions: 5_400, position: 12.1 },
      { key: "nld", clicks: 61, impressions: 2_100, position: 13.4 },
      { key: "aze", clicks: 38, impressions: 1_500, position: 11.0 },
    ],
    cannibalised: [
      {
        query: "spor ayakkabı",
        impressions: 38_500,
        pages: [
          { page: "https://ornekmagaza.com/spor-ayakkabi", clicks: 560, impressions: 31_200, position: 6.2 },
          { page: "https://ornekmagaza.com/blog/en-iyi-spor-ayakkabi-markalari", clicks: 60, impressions: 7_300, position: 11.8 },
        ],
      },
      {
        query: "beyaz sneaker",
        impressions: 9_800,
        pages: [
          { page: "https://ornekmagaza.com/sneaker/beyaz", clicks: 250, impressions: 7_100, position: 4.0 },
          { page: "https://ornekmagaza.com/kampanya/sneaker", clicks: 38, impressions: 2_700, position: 9.6 },
        ],
      },
    ],
    audit: { audit_id: 3, finished_at: NOW, problem_pages: 2 },
    tracked: [5, 8],
  };
}

function gscSetup(projectId: number): GscSetup {
  const settings = gscSettingsOf(projectId);
  return {
    settings,
    google: googleStatus,
    syncs: projectId === 1 ? gscSyncs : [],
    next_sync_at: settings.property && settings.schedule !== "manual" ? new Date(Date.parse(NOW) + (settings.schedule === "daily" ? 1 : 6) * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z") : null,
  };
}


// ----- Reports -------------------------------------------------------------------------------

let reportBranding: Branding = { company: "Örnek Ajans", logo: null, accent: "#6d4dff", website: "https://ornekajans.com", footer: "Gizli · yalnızca müşterimiz içindir", hide_octopuson: false };

let sampleReports: ReportRow[] = [
  {
    id: 1,
    project_id: 1,
    name: "Aylık SEO raporu",
    language: "tr",
    days: 28,
    widgets: ["summary", "rankings", "competitors", "search_console", "site_audit", "ai_visibility"],
    schedule: "monthly",
    recipients: ["musteri@ornekmagaza.com"],
    share_token: "demo-paylasim-baglantisi-orneginin-sabit-belirteci",
    share_views: 14,
    last_sent_at: NOW,
    created_at: NOW,
    updated_at: NOW,
  },
];

function reportsSetup(projectId: number): ReportsSetup {
  return {
    reports: sampleReports.filter((report) => report.project_id === projectId),
    branding: reportBranding,
    widgets: ["summary", "rankings", "competitors", "search_console", "site_audit", "ai_visibility", "backlinks"],
    can_print: true,
    can_share: true,
    can_email: true,
    public_url: "https://app.octopuson.com",
  };
}

const escapeHtml = (text: string) => text.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);

/** A sample report page shaped like the program's. */
function sampleReportHtml(report: ReportRow): string {
  const accent = reportBranding.accent ?? "#1e6fd9";
  const tile = (label: string, value: string, change: string, good: boolean) =>
    `<div class="tile"><span>${label}</span><strong>${value}</strong><em class="${good ? "good" : "bad"}">${change}</em></div>`;
  const rows = SAMPLE_QUERIES.slice(0, 8)
    .map(([query, clicks, impressions, position]) => `<tr><td>${escapeHtml(query)}</td><td class="n">${clicks.toLocaleString("tr-TR")}</td><td class="n">${impressions.toLocaleString("tr-TR")}</td><td class="n">${position.toLocaleString("tr-TR")}</td></tr>`)
    .join("");
  const points = [10.8, 11.2, 11.0, 11.9, 12.4, 12.1, 12.9, 13.4];
  const path = points.map((value, index) => `${index ? "L" : "M"}${46 + index * 89},${170 - value * 9}`).join(" ");
  return `<!doctype html><html lang="tr"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'"><style>
:root{--accent:${accent}}*{box-sizing:border-box}body{margin:0;background:#eef2f7;color:#18202e;font:13px/1.5 -apple-system,"Segoe UI",Roboto,Arial,sans-serif}
.report{max-width:920px;margin:24px auto;padding:32px 36px;background:#fff;border-radius:14px}.cover{display:flex;justify-content:space-between;gap:20px;border-bottom:3px solid var(--accent);padding-bottom:18px;margin-bottom:22px}
.cover strong{display:block;font-size:15px}.cover span{color:#5b6476;font-size:12px}.title{text-align:right}.title h1{margin:0 0 4px;font-size:22px}.title p{margin:0;color:#5b6476;font-size:12px}
h2{margin:0 0 8px;font-size:16px;color:var(--accent)}section{margin-bottom:26px}.tiles{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:10px;margin-bottom:14px}
.tile{padding:10px 12px;border:1px solid #e3e8f0;border-radius:10px;background:#f9fbfd}.tile span{display:block;color:#5b6476;font-size:11px;text-transform:uppercase}.tile strong{display:block;font-size:20px}.tile em{font-style:normal;font-size:11px;font-weight:600}
.good{color:#0f8a5f}.bad{color:#c2410c}table{width:100%;border-collapse:collapse;font-size:11.5px}th{text-align:left;color:#5b6476;border-bottom:1px solid #d7dde7;padding:5px 6px}td{border-bottom:1px solid #eef1f6;padding:5px 6px}.n{text-align:right}
footer{margin-top:18px;padding-top:10px;border-top:1px solid #e3e8f0;color:#6b7385;font-size:11px;display:flex;justify-content:space-between}</style></head><body><main class="report">
<header class="cover"><div><strong>${escapeHtml(reportBranding.company ?? "")}</strong><span>ornekajans.com</span></div><div class="title"><h1>${escapeHtml(report.name)}</h1><p>Örnek Mağaza · ornekmagaza.com</p><p>Dönem: ${isoDay(report.days + 2)} – ${isoDay(3)}</p></div></header>
<section><h2>Özet</h2><div class="tiles">${tile("Görünürlük", "%13", "+1,6", true)}${tile("Ortalama sıra", "8,2", "-0,7", true)}${tile("Tıklama", "4.212", "+%9,7", true)}${tile("Gösterim", "169.208", "+%9,1", true)}${tile("Sağlık", "82,5", "+4,5", true)}${tile("Anılma oranı", "%51", "+6,3", true)}</div></section>
<section><h2>Sıralamalar</h2><svg viewBox="0 0 680 190" width="100%"><line x1="46" x2="672" y1="170" y2="170" stroke="#e6ebf2"/><line x1="46" x2="672" y1="89" y2="89" stroke="#e6ebf2"/><path d="${path}" fill="none" stroke="${accent}" stroke-width="2.2"/></svg></section>
<section><h2>Search Console</h2><table><thead><tr><th>Sorgu</th><th class="n">Tıklama</th><th class="n">Gösterim</th><th class="n">Sıra</th></tr></thead><tbody>${rows}</tbody></table></section>
<footer><span>${escapeHtml(reportBranding.footer ?? "")}</span><span>OctopusOn SEO ile hazırlandı</span></footer></main></body></html>`;
}

const providers: ProviderInfo[] = [
  {
    id: "dataforseo",
    name: "DataForSEO",
    modules: ["overview", "keywords", "backlinks", "gap", "rank", "ai"],
    keys: [{ name: "credentials", state: "unset" }],
    mode: "auto",
    via: "credit",
    own_ready: false,
    credit_connector: "dataforseo",
    credits_per_usd: 1350,
    budget_usd: null,
    spent_usd: 0.42,
    signup_url: "https://app.dataforseo.com/register",
  },
  {
    id: "openai",
    name: "OpenAI (ChatGPT)",
    modules: ["ai"],
    keys: [{ name: "api_key", state: "unset" }],
    mode: "auto",
    via: null,
    own_ready: false,
    credit_connector: null,
    credits_per_usd: null,
    budget_usd: null,
    spent_usd: 0,
    signup_url: "https://platform.openai.com/api-keys",
  },
  {
    id: "anthropic",
    name: "Anthropic (Claude)",
    modules: ["ai"],
    keys: [{ name: "api_key", state: "unset" }],
    mode: "auto",
    via: null,
    own_ready: false,
    credit_connector: null,
    credits_per_usd: null,
    budget_usd: null,
    spent_usd: 0,
    signup_url: "https://console.anthropic.com/settings/keys",
  },
  {
    id: "google-gemini",
    name: "Google Gemini",
    modules: ["ai"],
    keys: [{ name: "api_key", state: "unset" }],
    mode: "auto",
    via: null,
    own_ready: false,
    credit_connector: null,
    credits_per_usd: null,
    budget_usd: null,
    spent_usd: 0,
    signup_url: "https://aistudio.google.com/apikey",
  },
  {
    id: "perplexity",
    name: "Perplexity",
    modules: ["ai"],
    keys: [{ name: "api_key", state: "unset" }],
    mode: "auto",
    via: null,
    own_ready: false,
    credit_connector: null,
    credits_per_usd: null,
    budget_usd: null,
    spent_usd: 0,
    signup_url: "https://www.perplexity.ai/account/api",
  },
];

const account: AccountStatus = {
  signed_in: true,
  account: {
    email: "ornek@octopuson.com",
    name: "Örnek",
    balance_micros: 18_500_000,
    held_micros: 0,
    available_micros: 18_500_000,
    credit_unit_micros: 1000,
    low_balance: false,
    portal_url: "https://my.octopuson.com",
  },
  error: null,
  portal_url: "https://my.octopuson.com",
  api: "https://api.octopuson.com",
  host: "desktop",
};

const usage: UsageRow[] = [
  { month: "2026-10", provider: "dataforseo", via: "credit", requests: 34, cost_usd: 0.42 },
  { month: "2026-09", provider: "dataforseo", via: "credit", requests: 120, cost_usd: 1.61 },
];

let settings: Settings = { modes: {}, budgets: {} };

const info: AppInfo = { name: "OctopusOn GEO-SEO Platform", version: "0.1.0", host: "desktop", data_dir: "~/Library/Application Support/com.OctopusOn.OctopusOn-GEO-SEO-Platform", mcp_command: "/Applications/OctopusOn GEO-SEO Platform.app/Contents/MacOS/octopuson-platform-desktop" };

function domainOf(args: Record<string, unknown>): string {
  const project = projects.find((row) => row.id === args.project_id);
  return typeof args.domain === "string" && args.domain.trim() ? args.domain.trim() : (project?.domain ?? "example.com");
}

/** Answers a command with sample data. */
export async function demo(command: string, args: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case "app_info":
      return info;
    case "get_settings":
      return settings;
    case "save_settings":
      settings = args.settings as Settings;
      return settings;
    case "list_access_tokens":
      return [];
    case "create_access_token":
    case "revoke_access_token":
      throw new CallError({ code: "unsupported", message: "Access tokens belong to the web version." });
    case "open_url":
      return { url: args.url, opened: false };
    case "list_projects":
      return projects;
    case "get_project":
      return projects.find((row) => row.id === args.id) ?? Promise.reject(new CallError({ code: "not_found", message: "project not found" }));
    case "create_project": {
      const spec = args.spec as ProjectSpec;
      const row: ProjectRow = { ...spec, name: spec.name || spec.domain, id: Math.max(0, ...projects.map((p) => p.id)) + 1, created_at: NOW, updated_at: NOW };
      projects = [...projects, row];
      return row;
    }
    case "update_project": {
      const spec = args.spec as ProjectSpec;
      projects = projects.map((row) => (row.id === args.id ? { ...row, ...spec, updated_at: NOW } : row));
      return projects.find((row) => row.id === args.id);
    }
    case "delete_project":
      projects = projects.filter((row) => row.id !== args.id);
      return null;
    case "list_providers":
      return providers;
    case "usage":
      return usage;
    case "account_status":
      return account;
    case "account_page":
      return { url: `https://my.octopuson.com/${String(args.page)}`, opened: false };
    case "list_jobs":
      return [];
    case "overview_estimate": {
      const rows = typeof args.limit === "number" ? args.limit : 1;
      const usd = 0.012 + 0.00012 * rows;
      return { usd, credits: usd * 1350, via: "credit", kept: { fetched_at: NOW, fresh: true }, blocked: null } satisfies Estimate;
    }
    case "get_overview":
    case "fetch_overview":
      return { data: report(String(args.report ?? "summary"), domainOf(args)), fetched_at: NOW, fresh: true, cached: true, spent_usd: 0, via: "credit" } satisfies Fetched<unknown>;
    case "keywords_estimate": {
      const rows = typeof args.limit === "number" ? args.limit : Array.isArray(args.keywords) ? args.keywords.length : 1;
      const usd = args.report === "serp" ? 0.002 : 0.012 + 0.00012 * rows;
      return { usd, credits: usd * 1350, via: "credit", kept: { fetched_at: NOW, fresh: true }, blocked: null } satisfies Estimate;
    }
    case "get_keywords":
    case "fetch_keywords":
      return { data: keywordReport(String(args.report ?? "overview"), args), fetched_at: NOW, fresh: true, cached: true, spent_usd: 0, via: "credit" } satisfies Fetched<unknown>;
    case "keyword_autocomplete":
      return ["", " kadın", " erkek", " indirim", " nike", " beyaz", " çocuk"].map((suffix) => `${String(args.seed)}${suffix}`);
    case "keyword_lists":
      return lists.map((list) => ({ ...list, keywords: listEntries[list.id]?.length ?? 0 }));
    case "create_keyword_list": {
      const list: KeywordListRow = { id: Math.max(0, ...lists.map((entry) => entry.id)) + 1, project_id: Number(args.project_id), name: String(args.name), keywords: 0, created_at: NOW };
      lists = [...lists, list];
      listEntries[list.id] = [];
      return list;
    }
    case "rename_keyword_list":
      lists = lists.map((list) => (list.id === args.list_id ? { ...list, name: String(args.name) } : list));
      return lists.find((list) => list.id === args.list_id);
    case "delete_keyword_list":
      lists = lists.filter((list) => list.id !== args.list_id);
      return null;
    case "list_keywords":
      return listEntries[Number(args.list_id)] ?? [];
    case "add_to_keyword_list": {
      const id = Number(args.list_id);
      const known = listEntries[id] ?? [];
      for (const data of args.keywords as KeywordData[]) if (!known.some((entry) => entry.keyword === data.keyword)) known.push({ keyword: data.keyword, data, added_at: NOW });
      listEntries = { ...listEntries, [id]: known };
      return known.length;
    }
    case "remove_from_keyword_list": {
      const id = Number(args.list_id);
      listEntries = { ...listEntries, [id]: (listEntries[id] ?? []).filter((entry) => !(args.keywords as string[]).includes(entry.keyword)) };
      return listEntries[id]?.length ?? 0;
    }
    case "cluster_keyword_list": {
      const entries = listEntries[Number(args.list_id)] ?? [];
      return entries.map((entry) => ({ pillar: entry.keyword, volume: Number(entry.data.volume ?? 0), keywords: [{ keyword: entry.keyword, volume: entry.data.volume ?? null, score: 1 }] })) satisfies Topic[];
    }
    case "backlinks_estimate": {
      const rows = args.report === "summary" ? 1 : typeof args.limit === "number" ? args.limit : 100;
      const usd = 0.024 + 0.000036 * rows;
      return { usd, credits: usd * 1350, via: "credit", kept: { fetched_at: NOW, fresh: true }, blocked: null } satisfies Estimate;
    }
    case "get_backlinks":
    case "fetch_backlinks":
      return { data: backlinkReport(String(args.report ?? "summary"), args), fetched_at: NOW, fresh: true, cached: true, spent_usd: 0, via: "credit" } satisfies Fetched<unknown>;
    case "gap_estimate": {
      const parts = args.report === "bulk" ? 2 : Array.isArray(args.competitors) ? Math.max(1, args.competitors.length) : 2;
      const usd = (0.012 + 0.00012 * (typeof args.limit === "number" ? args.limit : 100)) * parts;
      return { usd, credits: usd * 1350, via: "credit", kept: { fetched_at: NOW, fresh: true }, blocked: null } satisfies Estimate;
    }
    case "get_gap":
    case "fetch_gap":
      return { data: gapReport(String(args.report ?? "missing"), args), fetched_at: NOW, fresh: true, cached: true, spent_usd: 0, via: "credit" } satisfies Fetched<unknown>;
    case "rank_setup":
      return rankSetup(Number(args.project_id));
    case "save_rank_settings": {
      const state = rankOf(Number(args.project_id));
      state.settings = { ...state.settings, schedule: args.schedule as Schedule, depth: Number(args.depth), ai_overview: Boolean(args.ai_overview) };
      return rankSetup(Number(args.project_id));
    }
    case "add_rank_tracker": {
      const state = rankOf(Number(args.project_id));
      const id = Math.max(0, ...state.trackers.map((entry) => entry.id)) + 1;
      const project = projects.find((row) => row.id === args.project_id);
      state.trackers = [
        ...state.trackers,
        {
          id,
          project_id: Number(args.project_id),
          engine: (args.engine as SearchEngine) ?? "google",
          country: String(args.country ?? project?.country ?? "tr"),
          language: String(args.language ?? project?.language ?? "tr"),
          device: (args.device as "desktop" | "mobile") ?? "desktop",
          location: typeof args.location === "string" && args.location.trim() ? args.location.replace(/\s*,\s*/g, ",").trim() : null,
          created_at: NOW,
        },
      ];
      return rankSetup(Number(args.project_id));
    }
    case "remove_rank_tracker": {
      const state = rankOf(Number(args.project_id));
      state.trackers = state.trackers.filter((entry) => entry.id !== args.tracker_id);
      return rankSetup(Number(args.project_id));
    }
    case "track_keywords": {
      const state = rankOf(Number(args.project_id));
      const project = projects.find((row) => row.id === args.project_id);
      if (state.trackers.length === 0) {
        state.trackers = [{ ...tracker(1, "desktop", null), project_id: Number(args.project_id), country: project?.country ?? "tr", language: project?.language ?? "tr" }];
      }
      let added = 0;
      for (const entry of args.keywords as NewKeyword[]) {
        if (state.keywords.some((known) => known.keyword === entry.keyword)) continue;
        added += 1;
        const id = Math.max(0, ...state.keywords.map((known) => known.id)) + 1;
        state.keywords.push({ id, keyword: entry.keyword, volume: entry.volume ?? null, tags: (args.tags as string[]) ?? [], base: 5 + ((id * 7) % 30) });
      }
      return { added, total: state.keywords.length };
    }
    case "untrack_keywords": {
      const state = rankOf(Number(args.project_id));
      state.keywords = state.keywords.filter((keyword) => !(args.keyword_ids as number[]).includes(keyword.id));
      return state.keywords.length;
    }
    case "set_keyword_tags": {
      const state = rankOf(Number(args.project_id));
      for (const keyword of state.keywords) if ((args.keyword_ids as number[]).includes(keyword.id)) keyword.tags = args.tags as string[];
      return null;
    }
    case "rank_estimate": {
      const state = rankOf(Number(args.project_id));
      const checks = state.keywords.length * state.trackers.length;
      const each = 0.002 * Math.ceil(state.settings.depth / 10) + (state.settings.ai_overview ? 0.002 : 0);
      const usd = checks * each;
      const runs = state.settings.schedule === "daily" ? 30 : state.settings.schedule === "weekly" ? 4 : 0;
      return {
        checks,
        run: { usd, credits: usd * 1350, via: "credit", kept: null, blocked: null },
        runs_per_month: runs,
        month_usd: usd * runs,
        month_credits: usd * 1350 * runs,
      } satisfies RankEstimate;
    }
    case "start_rank_check":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to check rankings." });
    case "rank_runs":
      return Number(args.project_id) === 1 ? rankRuns : [];
    case "rank_report":
      return rankReport(Number(args.project_id), typeof args.tracker_id === "number" ? args.tracker_id : null, Number(args.days ?? 90));
    case "rank_keyword":
      return rankKeyword(Number(args.project_id), Number(args.keyword_id), Number(args.tracker_id));
    case "rank_volumes_estimate":
      return { usd: 0.012, credits: 16.2, via: "credit", kept: null, blocked: null } satisfies Estimate;
    case "fetch_rank_volumes": {
      const state = rankOf(Number(args.project_id));
      const missing = state.keywords.filter((keyword) => keyword.volume === null);
      for (const keyword of missing) keyword.volume = 1_000;
      return { updated: missing.length, spent_usd: 0.012 };
    }
    case "audit_setup": {
      const projectId = Number(args.project_id);
      return {
        settings: auditSettings[projectId] ?? { schedule: "manual", max_pages: 500, last_run_at: null },
        max_pages_limit: 100_000,
        next_audit_at: (auditSettings[projectId]?.schedule ?? "manual") === "manual" ? null : new Date(Date.parse(NOW) + 6 * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z"),
        meter: null,
        audits: audits.filter((row) => row.project_id === projectId),
      } satisfies AuditSetup;
    }
    case "save_audit_settings": {
      const projectId = Number(args.project_id);
      auditSettings = { ...auditSettings, [projectId]: { schedule: args.schedule as AuditSchedule, max_pages: Number(args.max_pages), last_run_at: auditSettings[projectId]?.last_run_at ?? null } };
      return demo("audit_setup", { project_id: projectId });
    }
    case "start_audit":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to audit a site." });
    case "audit_report":
      return auditReport(Number(args.project_id), typeof args.audit_id === "number" ? args.audit_id : null, String(args.lang ?? "tr"));
    case "audit_issue": {
      const sample = SAMPLE_ISSUES.find(([id]) => id === args.issue_id);
      const tr = args.lang === "tr";
      const total = sample?.[6] ?? 0;
      const offset = Number(args.offset ?? 0);
      return {
        id: String(args.issue_id),
        title: sample ? (tr ? sample[4] : sample[5]) : String(args.issue_id),
        description: tr ? "Örnek veri: bu kontrolün açıklaması programda gösterilir." : "Sample data: the program shows what this check means.",
        fix: tr ? "Örnek veri: nasıl düzeltileceği programda gösterilir." : "Sample data: the program shows how to fix it.",
        kind: sample?.[2] ?? "warning",
        priority: sample?.[3] ?? "low",
        total,
        offset,
        columns: ["issue_id", "address"],
        rows: Array.from({ length: Math.max(0, Math.min(100, total - offset)) }, (_, index) => [String(args.issue_id), `https://ornekmagaza.com/sayfa-${offset + index + 1}`]),
      } satisfies IssueDetail;
    }
    case "delete_audit":
      audits = audits.filter((row) => !(row.project_id === args.project_id && row.id === args.audit_id));
      return null;
    case "ai_setup":
      return aiSetup(Number(args.project_id));
    case "save_ai_settings": {
      const state = aiOf(Number(args.project_id));
      state.settings = {
        schedule: args.schedule as AiSchedule,
        engines: AI_ENGINES.filter((engine) => (args.engines as AiEngine[]).includes(engine)),
        models: (args.models as AiSettings["models"]) ?? {},
        rivals: (args.rivals as Rival[]) ?? [],
        last_run_at: state.settings.last_run_at,
      };
      return aiSetup(Number(args.project_id));
    }
    case "add_ai_prompts": {
      const state = aiOf(Number(args.project_id));
      const known = new Set(state.prompts.map((prompt) => prompt.prompt));
      let next = Math.max(0, ...state.prompts.map((prompt) => prompt.id)) + 1;
      let added = 0;
      for (const prompt of args.prompts as string[]) {
        if (known.has(prompt)) continue;
        known.add(prompt);
        state.prompts.push({ id: next++, project_id: Number(args.project_id), prompt, tags: (args.tags as string[]) ?? [], added_at: NOW });
        added += 1;
      }
      return { added, total: state.prompts.length };
    }
    case "remove_ai_prompts": {
      const state = aiOf(Number(args.project_id));
      state.prompts = state.prompts.filter((prompt) => !(args.prompt_ids as number[]).includes(prompt.id));
      return state.prompts.length;
    }
    case "suggest_ai_prompts": {
      const known = new Set(aiOf(Number(args.project_id)).prompts.map((prompt) => prompt.prompt));
      return ["Örnek Mağaza güvenilir mi?", "Örnek Mağaza yorumları nasıl?", "Spor ayakkabı için en iyi markalar hangileri?", "Spor ayakkabı nereden alınır?", "En iyi koşu ayakkabısı önerileri neler?"].filter((prompt) => !known.has(prompt));
    }
    case "ai_estimate":
      return aiEstimate(Number(args.project_id));
    case "start_ai_run":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to ask the assistants." });
    case "ai_runs":
      return Number(args.project_id) === 1 ? aiRuns : [];
    case "ai_report":
      return aiReport(Number(args.project_id), Number(args.days ?? 90));
    case "ai_answer":
      return aiAnswer(Number(args.project_id), Number(args.prompt_id), args.engine as AiEngine);
    case "ai_mentions_estimate": {
      const usd = args.report === "queries" ? 0.1 + 0.001 * Number(args.limit ?? 100) : args.report === "compare" ? 0.13 * Math.min(5, aiBrands(Number(args.project_id)).length) : 0.13;
      return { usd, credits: usd * 1350, via: "credit", kept: { fetched_at: NOW, fresh: true }, blocked: null } satisfies Estimate;
    }
    case "get_ai_mentions":
    case "fetch_ai_mentions":
      return { data: mentionsReport(Number(args.project_id), String(args.report ?? "summary")), fetched_at: NOW, fresh: true, cached: true, spent_usd: 0, via: "credit" } satisfies Fetched<unknown>;
    case "google_status":
    case "set_google_client":
    case "google_logout":
      return googleStatus;
    case "start_google_login":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to connect Google." });
    case "google_properties":
      return {
        search_console: [
          { site_url: "https://www.ornekmagaza.com/", permission: "siteFullUser" },
          { site_url: "sc-domain:ornekmagaza.com", permission: "siteOwner" },
        ],
        analytics: [{ id: "properties/312345678", name: "Örnek Mağaza", account: "Örnek" }],
        analytics_error: null,
        suggested: Number(args.project_id) === 1 ? "sc-domain:ornekmagaza.com" : null,
      };
    case "gsc_setup":
      return gscSetup(Number(args.project_id));
    case "save_gsc_settings": {
      const projectId = Number(args.project_id);
      gscSettings[projectId] = {
        property: (args.property as string | null) ?? null,
        ga4_property: (args.ga4_property as string | null) ?? null,
        ga4_name: (args.ga4_name as string | null) ?? null,
        days: Number(args.days),
        schedule: args.schedule as GscSchedule,
        last_sync_at: gscSettingsOf(projectId).last_sync_at,
      };
      return gscSetup(projectId);
    }
    case "start_gsc_sync":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to read Search Console." });
    case "gsc_report":
      return gscReport(Number(args.project_id));
    case "gsc_query": {
      const report = gscReport(Number(args.project_id));
      const shared = report?.cannibalised.find((entry) => entry.query === args.query);
      if (shared) return shared.pages.map((row) => ({ query: shared.query, ...row }));
      const row = report?.queries.find((entry) => entry.query === args.query);
      return row ? [{ query: row.query, page: "https://ornekmagaza.com/", clicks: row.clicks, impressions: row.impressions, position: row.position }] : [];
    }
    case "reports_setup":
      return reportsSetup(Number(args.project_id));
    case "save_report_branding":
      reportBranding = args.branding as Branding;
      return reportBranding;
    case "save_report": {
      const spec = args as unknown as ReportSpec;
      const id = typeof args.report_id === "number" ? args.report_id : Math.max(0, ...sampleReports.map((report) => report.id)) + 1;
      const previous = sampleReports.find((report) => report.id === id);
      const row: ReportRow = {
        id,
        project_id: Number(args.project_id),
        name: spec.name,
        language: spec.language,
        days: spec.days,
        widgets: spec.widgets,
        schedule: spec.schedule,
        recipients: spec.recipients,
        share_token: previous?.share_token ?? null,
        share_views: previous?.share_views ?? 0,
        last_sent_at: previous?.last_sent_at ?? null,
        created_at: previous?.created_at ?? NOW,
        updated_at: new Date().toISOString(),
      };
      sampleReports = [row, ...sampleReports.filter((report) => report.id !== id)];
      return row;
    }
    case "delete_report":
      sampleReports = sampleReports.filter((report) => report.id !== args.report_id);
      return null;
    case "report_html": {
      const report = sampleReports.find((row) => row.id === args.report_id) ?? sampleReports[0]!;
      return { name: "aylik-seo-raporu.html", html: sampleReportHtml(report) };
    }
    case "report_deliveries":
      return [{ id: 1, report_id: 1, project_id: 1, scheduled: true, status: "sent", recipients: 1, error: null, sent_at: NOW }];
    case "report_pdf":
    case "report_xlsx":
    case "share_report":
    case "unshare_report":
    case "send_report":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to make files, links and e-mails." });
    case "set_key":
    case "remove_key":
    case "set_source_mode":
    case "set_budget":
    case "cancel_job":
    case "clear_jobs":
    case "account_logout":
      return null;
    case "start_account_login":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to connect your account." });
    default:
      throw new CallError({ code: "not_found", message: `unknown command \`${command}\`` });
  }
}
