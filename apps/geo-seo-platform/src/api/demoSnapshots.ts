// Sample snapshots for the interface outside the program: four moments of the first sample
// project, from where the work began to two days ago, and their comparison.

import { CallError } from "./transport";
import type {
  KeywordMove,
  SnapshotComparison,
  SnapshotEstimate,
  SnapshotHeadline,
  SnapshotKpi,
  SnapshotProgress,
  SnapshotRow,
  SnapshotSettings,
  SnapshotSetup,
  SummarySentence,
} from "./types";

const ago = (days: number) => new Date(Date.now() - days * 86_400_000).toISOString().replace(/\.\d+Z$/, "Z");

const headline = (traffic: number, keywords: number, top10: number, oa: number, domains: number, visibility: number, health: number, mention: number, clicks: number): SnapshotHeadline => ({
  traffic,
  keywords,
  top10,
  oa,
  referring_domains: domains,
  visibility,
  health,
  mention_rate: mention,
  clicks,
  missing: 0,
});

const row = (id: number, days: number, label: string | null, kind: "manual" | "scheduled", cost: number, numbers: SnapshotHeadline): SnapshotRow => ({
  id,
  project_id: 1,
  label,
  kind,
  status: "done",
  taken_at: ago(days),
  finished_at: ago(days),
  cost_usd: cost,
  error: null,
  headline: numbers,
});

let rows: SnapshotRow[] = [
  row(4, 2, null, "scheduled", 0.09, headline(18_420, 2_310, 212, 38, 486, 15.8, 84, 51, 4_212)),
  row(3, 31, null, "scheduled", 0.11, headline(16_900, 2_180, 190, 36, 461, 14.2, 81, 44, 3_840)),
  row(2, 60, null, "scheduled", 0.34, headline(15_200, 2_050, 171, 35, 440, 12.9, 77, 38, 3_510)),
  row(1, 90, "Başlangıç", "manual", 0.34, headline(12_800, 1_870, 142, 33, 412, 10.8, 71, 30, 3_020)),
];

let settings: SnapshotSettings = { schedule: "monthly", competitors: ["rakip-magaza.com"], last_run_at: ago(2) };

function setupOf(projectId: number): SnapshotSetup {
  const own = projectId === 1 ? rows : [];
  return {
    settings: projectId === 1 ? settings : { schedule: "manual", competitors: [], last_run_at: null },
    competitors: projectId === 1 ? ["rakip-magaza.com", "digerdukkan.com.tr"] : [],
    snapshots: own,
    baseline_id: own.length ? own[own.length - 1]!.id : null,
    next_at: projectId === 1 && settings.schedule !== "manual" ? Math.round(Date.now() / 1000 + (settings.schedule === "weekly" ? 4.5 : 27) * 86_400) : null,
  };
}

function kpi(key: string, group: SnapshotKpi["group"], before: number | null, after: number | null, higherBetter = true): SnapshotKpi {
  const change = before !== null && after !== null ? after - before : null;
  return { key, group, before, after, change, percent: change !== null && before ? (change / Math.abs(before)) * 100 : null, higher_better: higherBetter };
}

const move = (keyword: string, before: number | null, after: number | null, trafficBefore: number, trafficAfter: number, path: string): KeywordMove => ({
  keyword,
  before,
  after,
  change: before !== null && after !== null ? before - after : null,
  traffic_before: trafficBefore,
  traffic_after: trafficAfter,
  url: `https://ornekmagaza.com/${path}`,
});

const SUMMARY: Record<string, string[]> = {
  tr: [
    "Organik trafik {pct} arttı ({before} → {after}).",
    "{top10} kelime ilk 10'a girdi.",
    "{domains} yeni alan adı link verdi.",
    "Site sağlığı {health} puana çıktı.",
    "1 alan adının linki artık yok.",
  ],
  en: ["Organic traffic rose {pct} ({before} → {after}).", "{top10} keywords entered the top 10.", "{domains} new domains link to the site.", "Site health rose to {health}.", "1 domain no longer links to the site."],
  de: [
    "Der organische Traffic stieg um {pct} ({before} → {after}).",
    "{top10} Keywords kamen in die Top 10.",
    "{domains} neue Domains verlinken die Website.",
    "Die Website-Gesundheit stieg auf {health}.",
    "1 Domain verlinkt die Website nicht mehr.",
  ],
  es: [
    "El tráfico orgánico subió un {pct} ({before} → {after}).",
    "{top10} palabras clave entraron en el top 10.",
    "{domains} dominios nuevos enlazan al sitio.",
    "La salud del sitio subió a {health}.",
    "1 dominio ya no enlaza al sitio.",
  ],
};

function summaryOf(lang: string, before: SnapshotHeadline, after: SnapshotHeadline): SummarySentence[] {
  const locale = { tr: "tr-TR", en: "en-GB", de: "de-DE", es: "es-ES" }[lang] ?? "en-GB";
  const whole = (value: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value);
  const pct = new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 }).format(((after.traffic ?? 0) - (before.traffic ?? 0)) / Math.max(1, before.traffic ?? 1));
  const values: Record<string, string> = {
    pct,
    before: whole(before.traffic ?? 0),
    after: whole(after.traffic ?? 0),
    top10: whole(Math.max(1, (after.top10 ?? 0) - (before.top10 ?? 0))),
    domains: whole(Math.max(1, (after.referring_domains ?? 0) - (before.referring_domains ?? 0))),
    health: whole(after.health ?? 0),
  };
  const texts = SUMMARY[lang] ?? SUMMARY.en!;
  return texts.map((text, index) => ({ tone: index === texts.length - 1 ? "bad" : "good", text: text.replace(/\{(\w+)\}/g, (_, name: string) => values[name] ?? "") }));
}

function compare(projectId: number, args: Record<string, unknown>): SnapshotComparison | null {
  const own = projectId === 1 ? rows.filter((entry) => entry.status === "done" || entry.status === "partial") : [];
  if (own.length < 2) return null;
  const toId = typeof args.to === "number" ? args.to : own[0]!.id;
  const toIndex = own.findIndex((entry) => entry.id === toId);
  const fromId = typeof args.from === "number" ? args.from : args.previous ? own[toIndex + 1]?.id : own[own.length - 1]!.id;
  let from = own.find((entry) => entry.id === fromId);
  let to = own.find((entry) => entry.id === toId);
  if (!from || !to) return null;
  if (from.id === to.id) throw new CallError({ code: "invalid", message: "choose two different snapshots" });
  if (Date.parse(to.taken_at) < Date.parse(from.taken_at)) [from, to] = [to, from];
  const [a, b] = [from.headline!, to.headline!];
  const share = (traffic: number | null) => (traffic === null ? null : traffic * 0.42);
  return {
    from: { id: from.id, label: from.label, taken_at: from.taken_at },
    to: { id: to.id, label: to.label, taken_at: to.taken_at },
    days: Math.round((Date.parse(to.taken_at) - Date.parse(from.taken_at)) / 86_400_000),
    kpis: [
      kpi("traffic", "organic", a.traffic, b.traffic),
      kpi("traffic_value", "organic", share(a.traffic), share(b.traffic)),
      kpi("keywords", "organic", a.keywords, b.keywords),
      kpi("top10", "organic", a.top10, b.top10),
      kpi("oa", "links", a.oa, b.oa),
      kpi("referring_domains", "links", a.referring_domains, b.referring_domains),
      kpi("visibility", "rank", a.visibility, b.visibility),
      kpi("average_position", "rank", 11.4, 8.2, false),
      kpi("health", "audit", a.health, b.health),
      kpi("issues", "audit", 64, 37, false),
      kpi("mention_rate", "ai", a.mention_rate, b.mention_rate),
      kpi("share_of_voice", "ai", 18.2, 28.6),
      kpi("clicks", "gsc", a.clicks, b.clicks),
      kpi("impressions", "gsc", 132_400, 169_208),
      kpi("local_solv", "local", 31, 46),
      kpi("local_arp", "local", 7.8, 4.2, false),
      kpi("local_rating", "local", 4.2, 4.4),
      kpi("risky_links", "links", 6, 2, false),
    ],
    keywords: {
      gained: 214,
      lost: 61,
      improved: 388,
      declined: 142,
      entered_top10: Math.max(1, (b.top10 ?? 0) - (a.top10 ?? 0)),
      left_top10: 9,
      new_keywords: [move("koşu ayakkabısı kadın", null, 7, 0, 240, "kosu-kadin"), move("spor ayakkabı indirim", null, 9, 0, 180, "indirim"), move("bot modelleri 2026", null, 14, 0, 60, "bot")],
      lost_keywords: [move("ayakkabı bakımı", 18, null, 40, 0, "blog/bakim"), move("terlik", 42, null, 6, 0, "terlik")],
      improved_keywords: [move("spor ayakkabı", 12, 4, 380, 1_420, "spor-ayakkabi"), move("koşu ayakkabısı", 9, 3, 520, 1_180, "kosu"), move("sneaker", 21, 11, 40, 160, "sneaker")],
      declined_keywords: [move("yürüyüş ayakkabısı", 5, 8, 610, 380, "yuruyus"), move("çocuk ayakkabı", 14, 19, 90, 40, "cocuk")],
    },
    pages: {
      up: [
        { url: "https://ornekmagaza.com/spor-ayakkabi", before: 2_100, after: 3_480, change: 1_380 },
        { url: "https://ornekmagaza.com/kosu", before: 1_400, after: 2_210, change: 810 },
      ],
      down: [{ url: "https://ornekmagaza.com/yuruyus", before: 1_120, after: 860, change: -260 }],
    },
    domains: {
      gained: Math.max(1, (b.referring_domains ?? 0) - (a.referring_domains ?? 0)),
      lost: 1,
      gained_domains: [
        { domain: "modablog.com.tr", rank: 52, backlinks: 3 },
        { domain: "kosuforum.net", rank: 41, backlinks: 2 },
        { domain: "haberler.example.com", rank: 38, backlinks: 1 },
      ],
      lost_domains: [{ domain: "eskidizin.net", rank: 12, backlinks: 4 }],
    },
    tracked: {
      improved: 14,
      declined: 4,
      moves: [
        { keyword: "spor ayakkabı", before: 12, after: 4, change: 8 },
        { keyword: "koşu ayakkabısı", before: 9, after: 3, change: 6 },
        { keyword: "yürüyüş ayakkabısı", before: 5, after: 8, change: -3 },
      ],
    },
    audit: {
      fixed: [
        { id: "title_missing", title: "Başlığı olmayan sayfalar", before: 12, after: 0 },
        { id: "broken_links", title: "Kırık iç linkler", before: 8, after: 0 },
      ],
      new_issues: [{ id: "h1_missing", title: "H1 etiketi olmayan sayfalar", before: 0, after: 3 }],
      better: [{ id: "slow", title: "Yavaş sayfalar", before: 21, after: 9 }],
      worse: [],
    },
    ai: {
      gained: [
        { prompt: "En iyi koşu ayakkabısı markaları hangileri?", engine: "chatgpt", before: null, after: 2 },
        { prompt: "Spor ayakkabı nereden alınır?", engine: "perplexity", before: null, after: 1 },
      ],
      lost: [{ prompt: "Uygun fiyatlı sneaker önerileri", engine: "gemini", before: 3, after: null }],
      brands: [
        { key: "ornekmagaza.com", own: true, before: 18.2, after: 28.6 },
        { key: "rakip-magaza.com", own: false, before: 34.1, after: 30.2 },
      ],
    },
    local: {
      improved: 2,
      declined: 1,
      keywords: [
        { keyword: "koşu ayakkabısı", arp_before: 9.6, arp_after: 3.1, solv_before: 18, solv_after: 61 },
        { keyword: "spor ayakkabı mağazası", arp_before: 6.2, arp_after: 4.0, solv_before: 35, solv_after: 47 },
        { keyword: "ayakkabıcı", arp_before: 3.4, arp_after: 4.1, solv_before: 66, solv_after: 58 },
      ],
    },
    link_audit: { new_risky: ["bedava-dizin.top"], cleared: ["seo-links-4u.xyz", "linkciftligi.net", "spamdizin.icu", "ucuzlink.biz", "rss-ciftligi.xyz"] },
    sites: [
      {
        domain: "ornekmagaza.com",
        own: true,
        kpis: [kpi("traffic", "organic", a.traffic, b.traffic), kpi("keywords", "organic", a.keywords, b.keywords), kpi("top10", "organic", a.top10, b.top10), kpi("oa", "links", a.oa, b.oa), kpi("referring_domains", "links", a.referring_domains, b.referring_domains)],
      },
      {
        domain: "rakip-magaza.com",
        own: false,
        kpis: [kpi("traffic", "organic", 41_200, 43_900), kpi("keywords", "organic", 5_400, 5_620), kpi("top10", "organic", 610, 640), kpi("oa", "links", 47, 47), kpi("referring_domains", "links", 1_210, 1_236)],
      },
    ],
    summary: summaryOf(String(args.lang ?? "tr"), a, b),
  };
}

/** Answers a snapshot command with sample data; `undefined` for other commands. */
export function snapshotDemo(command: string, args: Record<string, unknown>): unknown {
  const projectId = Number(args.project_id);
  switch (command) {
    case "snapshot_setup":
      return setupOf(projectId);
    case "list_snapshots":
      return setupOf(projectId).snapshots;
    case "save_snapshot_settings": {
      const next = args.settings as SnapshotSettings;
      if (projectId === 1) settings = { ...next, last_run_at: settings.last_run_at };
      return projectId === 1 ? settings : { ...next, last_run_at: null };
    }
    case "snapshot_estimate": {
      const competitors = Array.isArray(args.competitors) ? args.competitors.length : projectId === 1 ? settings.competitors.length : 0;
      const usd = 0.25 + 0.05 * competitors;
      return { usd, credits: usd * 1350, full_credits: usd * 1350, via: "credit", kept: projectId === 1 ? 2 : 0, paid: 5 + 2 * competitors - (projectId === 1 ? 2 : 0), blocked: null } satisfies SnapshotEstimate;
    }
    case "start_snapshot":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to take a snapshot." });
    case "delete_snapshot":
      rows = rows.filter((entry) => entry.id !== args.snapshot_id);
      return null;
    case "compare_snapshots":
      return compare(projectId, args);
    case "snapshot_xlsx":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to make files." });
    case "snapshot_progress": {
      const comparison = compare(1, {});
      if (!comparison) return [];
      const keys = ["traffic", "keywords", "oa", "health", "mention_rate"];
      return [
        {
          project_id: 1,
          from: comparison.from,
          to: comparison.to,
          days: comparison.days,
          kpis: keys.map((key) => comparison.kpis.find((entry) => entry.key === key)).filter((entry): entry is SnapshotKpi => Boolean(entry)),
        },
      ] satisfies SnapshotProgress[];
    }
    default:
      return undefined;
  }
}
