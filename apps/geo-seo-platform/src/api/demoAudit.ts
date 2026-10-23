// Sample views of the first sample project's latest audit (internal links, pages, hreflang,
// findings as CSV) and its Core Web Vitals, for the interface outside the program.

import { CallError } from "./transport";
import type { CrawledPageDetail, FieldData, FieldHistory, FindingsFile, HreflangReport, LabReport, LinkedPage, LinksReport, PagesView, VitalsReport, VitalsSetup } from "./types";

const SITE = "https://ornekmagaza.com";
const day = (back: number) => new Date(Date.now() - back * 86_400_000).toISOString().slice(0, 10);

const PATHS: [string, string, number, number, number, number][] = [
  // path, title, depth, linking pages, words, response ms
  ["/", "Örnek Mağaza — Ayakkabı ve Sneaker", 0, 842, 640, 180],
  ["/spor-ayakkabi", "Spor Ayakkabı Modelleri | Örnek Mağaza", 1, 812, 410, 220],
  ["/kosu-ayakkabisi", "Koşu Ayakkabısı | Örnek Mağaza", 1, 806, 640, 205],
  ["/kislik-bot", "Kışlık Bot Modelleri", 1, 790, 380, 240],
  ["/sneaker/beyaz", "Beyaz Sneaker", 2, 64, 290, 260],
  ["/blog/ayakkabi-numarasi-secimi", "Ayakkabı numarası nasıl seçilir?", 2, 12, 1_480, 310],
  ["/blog/kosu-ayakkabisi-secimi", "Koşu ayakkabısı seçerken 7 ipucu", 3, 3, 1_820, 290],
  ["/kampanya/sneaker", "Sneaker kampanyası", 3, 2, 120, 340],
  ["/kategori/bot/deri", "Deri bot", 4, 1, 210, 420],
  ["/kategori/bot/deri/erkek", "Erkek deri bot", 5, 1, 180, 460],
  ["/hakkimizda", "Hakkımızda", 1, 838, 520, 150],
  ["/iletisim", "İletişim", 1, 838, 90, 140],
];

const page = ([path, title, depth, inlinks]: (typeof PATHS)[number], rank: number): LinkedPage => ({ url: `${SITE}${path}`, title, depth, inlinks, rank, indexability: "indexable" });

const RANKS = [100, 86.4, 84.9, 82.1, 41.7, 22.5, 9.8, 7.1, 3.2, 0, 79.3, 78.8];

const LINKS: LinksReport = {
  audit_id: 2,
  pages: 512,
  depth: [
    [0, 1],
    [1, 38],
    [2, 214],
    [3, 181],
    [4, 52],
    [5, 19],
    [6, 7],
  ],
  average_depth: 2.6,
  deep: 78,
  sitemaps: true,
  orphan_total: 6,
  orphans: [
    { url: `${SITE}/blog/yaz-sandalet-rehberi`, status: 200, indexability: "indexable" },
    { url: `${SITE}/kampanya/okula-donus`, status: 200, indexability: "indexable" },
    { url: `${SITE}/kategori/terlik/cocuk`, status: 200, indexability: "indexable" },
    { url: `${SITE}/eski-koleksiyon`, status: 301, indexability: "redirected" },
    { url: `${SITE}/blog/kis-bakimi`, status: null, indexability: "" },
    { url: `${SITE}/magazalar/izmir`, status: 404, indexability: "client-error" },
  ],
  weak_total: 23,
  weak: PATHS.map((row, index) => page(row, RANKS[index]!)).filter((row) => row.inlinks <= 3 && row.depth !== 0).sort((a, b) => a.rank - b.rank),
  top: PATHS.map((row, index) => page(row, RANKS[index]!)).sort((a, b) => b.rank - a.rank),
};

const HREFLANG: HreflangReport = {
  audit_id: 2,
  pages: 148,
  languages: [
    ["tr", 148],
    ["en", 141],
    ["de", 96],
    ["x-default", 52],
  ],
  issues: [
    { id: "hreflang-missing-return", title: "Karşılık linki olmayan hreflang", priority: "high", count: 14 },
    { id: "hreflang-missing-x-default", title: "x-default yok", priority: "low", count: 96 },
    { id: "hreflang-to-non-200", title: "200 dönmeyen sayfaya hreflang", priority: "high", count: 3 },
    { id: "hreflang-invalid-code", title: "Geçersiz dil kodu", priority: "medium", count: 0 },
    { id: "hreflang-missing-self", title: "Kendine hreflang yok", priority: "medium", count: 0 },
    { id: "hreflang-to-non-indexable", title: "Dizine eklenemeyen sayfaya hreflang", priority: "medium", count: 2 },
    { id: "hreflang-lang-mismatch", title: "Sayfa dili hreflang ile uyuşmuyor", priority: "low", count: 5 },
  ],
  total: 437,
  columns: ["page", "lang", "target", "target_status", "return_link"],
  rows: [
    [`${SITE}/`, "en", `${SITE}/en/`, "200", "true"],
    [`${SITE}/`, "de", `${SITE}/de/`, "200", "true"],
    [`${SITE}/spor-ayakkabi`, "en", `${SITE}/en/sneakers`, "200", "true"],
    [`${SITE}/spor-ayakkabi`, "de", `${SITE}/de/turnschuhe`, "200", "false"],
    [`${SITE}/kislik-bot`, "en", `${SITE}/en/winter-boots`, "404", "false"],
  ],
};

const COLUMNS = ["address", "content", "status_code", "indexability", "title", "crawl_depth", "unique_inlinks", "word_count", "response_time_ms", "h1"];

function pagesView(args: Record<string, unknown>): PagesView {
  const search = String(args.search ?? "").toLocaleLowerCase("tr");
  const status = Number(args.status_class ?? 0);
  const indexability = String(args.indexability ?? "");
  const extra: (typeof PATHS)[number][] = [
    ["/eski-koleksiyon", "", 2, 4, 0, 90],
    ["/magazalar/izmir", "", 3, 2, 0, 120],
  ];
  let rows = [...PATHS, ...extra].map(([path, title, depth, inlinks, words, ms]) => {
    const code = path === "/eski-koleksiyon" ? "301" : path === "/magazalar/izmir" ? "404" : "200";
    const index = code === "301" ? "redirected" : code === "404" ? "client-error" : path === "/kampanya/sneaker" ? "noindex" : "indexable";
    return [`${SITE}${path}`, "html", code, index, title, String(depth), String(inlinks), String(words), String(ms), title.split(" | ")[0] ?? ""];
  });
  rows = rows.filter((row) => (!search || row[0]!.includes(search) || row[4]!.toLocaleLowerCase("tr").includes(search)) && (!status || row[2]!.startsWith(String(status))) && (!indexability || row[3] === indexability));
  const sort = COLUMNS.indexOf(String(args.sort ?? "address"));
  if (sort >= 0) {
    rows.sort((a, b) => {
      const left = a[sort] ?? "";
      const right = b[sort] ?? "";
      const order = /^\d+$/.test(left) && /^\d+$/.test(right) ? Number(left) - Number(right) : left.localeCompare(right);
      return args.descending ? -order : order;
    });
  }
  const offset = Number(args.offset ?? 0);
  return { audit_id: 2, total: rows.length, offset, columns: COLUMNS, rows: rows.slice(offset, offset + Number(args.limit ?? 100)) };
}

function pageDetail(url: string): CrawledPageDetail {
  const row = pagesView({}).rows.find((entry) => entry[0] === url);
  if (!row) throw new CallError({ code: "not_found", message: `page ${url} not found` });
  const facts: [string, string][] = COLUMNS.map((name, index) => [name, row[index] ?? ""]);
  facts.push(["title_length", String((row[4] ?? "").length)], ["meta_description", "Örnek Mağaza'da yüzlerce model."], ["h2_count", "4"]);
  return {
    url,
    facts,
    findings: url.endsWith("/kampanya/sneaker")
      ? [
          { id: "noindex", title: "Noindex sayfa", priority: "high", kind: "issue" },
          { id: "content-thin", title: "İnce içerik", priority: "medium", kind: "warning" },
        ]
      : [{ id: "meta-description-short", title: "Meta açıklama kısa", priority: "low", kind: "opportunity" }],
    inlinks_total: Number(row[6]),
    inlinks: [
      { url: `${SITE}/`, anchor: row[4]?.split(" ")[0] ?? "" },
      { url: `${SITE}/spor-ayakkabi`, anchor: "Tümünü gör" },
    ],
  };
}

const metric = (p75: number, good: number, needs: number) => ({ p75, good, needs, poor: Math.round((1 - good - needs) * 100) / 100 });

const PHONE: FieldData = {
  lcp: metric(2_840, 0.68, 0.21),
  inp: metric(230, 0.71, 0.2),
  cls: metric(0.06, 0.88, 0.07),
  fcp: metric(1_920, 0.72, 0.19),
  ttfb: metric(910, 0.62, 0.27),
  first_date: day(31),
  last_date: day(3),
  origin_fallback: false,
};

const DESKTOP: FieldData = {
  lcp: metric(1_760, 0.86, 0.1),
  inp: metric(90, 0.93, 0.05),
  cls: metric(0.04, 0.92, 0.05),
  fcp: metric(1_120, 0.88, 0.09),
  ttfb: metric(620, 0.78, 0.16),
  first_date: day(31),
  last_date: day(3),
  origin_fallback: false,
};

function history(base: number, drift: number, inp: number, cls: number): FieldHistory {
  const weeks = 25;
  return {
    dates: Array.from({ length: weeks }, (_, index) => day(3 + (weeks - 1 - index) * 7)),
    lcp: Array.from({ length: weeks }, (_, index) => Math.round(base + drift * (weeks - index) + (index % 3) * 40)),
    inp: Array.from({ length: weeks }, (_, index) => Math.round(inp + (weeks - index) * 2)),
    cls: Array.from({ length: weeks }, (_, index) => Math.round((cls + (weeks - index) * 0.002) * 100) / 100),
    fcp: [],
    ttfb: [],
  };
}

const lab = (path: string, score: number, lcp: number, cls: number, tbt: number): LabReport => ({
  url: `${SITE}${path}`,
  score,
  lcp_ms: lcp,
  cls,
  tbt_ms: tbt,
  fcp_ms: Math.round(lcp * 0.55),
  speed_index_ms: Math.round(lcp * 1.2),
  ttfb_ms: 420,
  opportunities: [
    { id: "unused-javascript", title: "Kullanılmayan JavaScript'i azaltın", savings_ms: 1_650 },
    { id: "render-blocking-resources", title: "Oluşturmayı engelleyen kaynakları ortadan kaldırın", savings_ms: 920 },
    { id: "modern-image-formats", title: "Resimleri yeni nesil biçimlerde yayınlayın", savings_ms: 610 },
  ].slice(0, score > 80 ? 1 : 3),
  field: null,
});

const VITALS: VitalsReport = {
  origins: [
    { origin: SITE, device: "phone", day: day(0), field: PHONE, history: history(2_600, 14, 205, 0.04), passes: false },
    { origin: SITE, device: "desktop", day: day(0), field: DESKTOP, history: history(1_650, 6, 80, 0.03), passes: true },
  ],
  pages: [
    { url: `${SITE}/`, device: "phone", day: day(0), lab: lab("/", 58, 3_900, 0.08, 480), field: PHONE, passes: false },
    { url: `${SITE}/spor-ayakkabi`, device: "phone", day: day(0), lab: lab("/spor-ayakkabi", 41, 5_200, 0.21, 910), field: null, passes: null },
    { url: `${SITE}/kosu-ayakkabisi`, device: "phone", day: day(0), lab: lab("/kosu-ayakkabisi", 72, 2_900, 0.05, 260), field: null, passes: null },
    { url: `${SITE}/blog/ayakkabi-numarasi-secimi`, device: "phone", day: day(0), lab: lab("/blog/ayakkabi-numarasi-secimi", 91, 1_800, 0.01, 90), field: null, passes: null },
  ],
  scores: [
    [day(14), `${SITE}/`, "phone", 52],
    [day(7), `${SITE}/`, "phone", 55],
    [day(0), `${SITE}/`, "phone", 58],
  ],
};

export function auditDemo(command: string, args: Record<string, unknown>): unknown {
  const first = Number(args.project_id) === 1;
  switch (command) {
    case "audit_links":
      return first ? LINKS : null;
    case "audit_hreflang":
      return first ? HREFLANG : null;
    case "audit_pages":
      return first ? pagesView(args) : null;
    case "audit_page":
      return pageDetail(String(args.url ?? ""));
    case "audit_findings_csv": {
      const csv = ["check,title,priority,url", "noindex,Noindex sayfa,high,https://ornekmagaza.com/kampanya/sneaker", "title-too-long,Başlık çok uzun,medium,https://ornekmagaza.com/spor-ayakkabi", "meta-description-short,Meta açıklama kısa,low,https://ornekmagaza.com/iletisim"].join("\n");
      return { name: "audit-2-findings.csv", csv: `${csv}\n`, rows: 3, cut: false } satisfies FindingsFile;
    }
    case "vitals_setup":
      return { key_ready: first, pages: [`${SITE}/`, `${SITE}/spor-ayakkabi`, `${SITE}/kosu-ayakkabisi`, `${SITE}/blog/ayakkabi-numarasi-secimi`], last_checked: first ? new Date(Date.now() - 3_600_000).toISOString() : null } satisfies VitalsSetup;
    case "start_vitals":
      throw new CallError({ code: "unsupported", message: "Sample data: open the program to check Core Web Vitals." });
    case "vitals_report":
      return first ? VITALS : { origins: [], pages: [], scores: [] };
    default:
      return undefined;
  }
}
