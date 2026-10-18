import type {
  FinderJob,
  FinderPageRequest,
  FinderTablePage,
  PathCluster,
  RedirectCandidate,
  RedirectServerFormat,
  ScanComparison,
  ScanConfig,
  ScanMeta,
  ScanOverview,
  VerifiedUrl,
} from "./types";
import type { Translate } from "../i18n";

export const defaultConfig: ScanConfig = {
  domain: "",
  include_subdomains: false,
  engines: ["google", "yandex", "bing", "duckduckgo"],
  use_wayback: false,
  use_commoncrawl: false,
  use_sitemaps: false,
  expand_subpaths: false,
  pages_per_query: 3,
  seed_queries: [],
  extra_urls: [],
  api: {
    google_api_key: "",
    google_cx: "",
    yandex_api_key: "",
    yandex_user: "",
    serper_api_key: "",
    serpapi_key: "",
  },
  detect_all_4xx: true,
  detect_5xx: true,
  detect_thin_200: true,
  min_word_count: 60,
  concurrency: 8,
  timeout_seconds: 15,
  max_redirects: 5,
  user_agent:
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 (compatible; octopusSEO-Finder/0.1)",
};

const demoRows: VerifiedUrl[] = [
  {
    url: "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2024",
    engines: "google,yandex,wayback,sitemap",
    serp_title: "Yazlık Keten Gömlek 2024 Koleksiyonu | Örnek Mağaza",
    serp_snippet: "2024 yaz sezonunun en çok tercih edilen %100 doğal keten gömlek modelleri...",
    found_by_query: "site:magaza.ornek.com.tr/urunler",
    initial_status: 404,
    final_status: 404,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2024",
    response_time_ms: 84,
    content_type: "text/html; charset=utf-8",
    live_title: "404 Sayfa Bulunamadı",
    h1: "Aradığınız sayfa kaldırılmış olabilir",
    word_count: 14,
    canonical: "",
    noindex: true,
    category: "error_404",
    reason: "http_404",
    suggested_target: "https://magaza.ornek.com.tr/urunler/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/kampanya/kara-cuma-2023-firsatlari",
    engines: "google,bing,commoncrawl",
    serp_title: "Kara Cuma 2023 İndirimleri Başladı - %50'ye Varan Fırsatlar",
    serp_snippet: "Kasım ayı boyunca geçerli sepette ek indirim kampanyasını kaçırmayın.",
    found_by_query: "site:magaza.ornek.com.tr/kampanya",
    initial_status: 301,
    final_status: 404,
    redirect_hops: 2,
    final_url: "https://magaza.ornek.com.tr/kampanya/arsiv-2023",
    response_time_ms: 192,
    content_type: "text/html; charset=utf-8",
    live_title: "404 Not Found",
    h1: "Sayfa Bulunamadı",
    word_count: 9,
    canonical: "",
    noindex: false,
    category: "error_404",
    reason: "redirect_to_404",
    suggested_target: "https://magaza.ornek.com.tr/kampanya/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/urunler/deri-canta-limitli-seri",
    engines: "google,yandex,sitemap",
    serp_title: "El Yapımı Deri Çanta Limitli Seri | Örnek Mağaza",
    serp_snippet: "Hakiki deri omuz çantası şimdi ücretsiz kargo avantajıyla.",
    found_by_query: "site:magaza.ornek.com.tr",
    initial_status: 200,
    final_status: 200,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/urunler/deri-canta-limitli-seri",
    response_time_ms: 118,
    content_type: "text/html; charset=utf-8",
    live_title: "Aradığınız Ürün Bulunamadı | Örnek Mağaza",
    h1: "Ürün Bulunamadı",
    word_count: 24,
    canonical: "https://magaza.ornek.com.tr/urunler/deri-canta-limitli-seri",
    noindex: false,
    category: "soft_404",
    reason: "soft_404_phrase",
    suggested_target: "https://magaza.ornek.com.tr/urunler/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/blog/eski-stil-onerileri",
    engines: "yandex,duckduckgo,commoncrawl",
    serp_title: "Sonbahar Kombin ve Stil Önerileri — Blog",
    serp_snippet: "Gardırobunuzu yenilerken dikkat etmeniz gereken 7 temel parça.",
    found_by_query: "site:magaza.ornek.com.tr/blog",
    initial_status: 302,
    final_status: 200,
    redirect_hops: 1,
    final_url: "https://magaza.ornek.com.tr/",
    response_time_ms: 145,
    content_type: "text/html; charset=utf-8",
    live_title: "Örnek Mağaza — Online Alışveriş",
    h1: "Yeni Sezon Ürünleri",
    word_count: 340,
    canonical: "https://magaza.ornek.com.tr/",
    noindex: false,
    category: "soft_404",
    reason: "redirect_to_root",
    suggested_target: "https://magaza.ornek.com.tr/blog/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/kategori/aksesuar/saat-kordonlari",
    engines: "google,yandex,bing",
    serp_title: "Saat Kordonları ve Aksesuar Modelleri",
    serp_snippet: "Farklı renk ve materyal seçenekleriyle saat kordonları.",
    found_by_query: "site:magaza.ornek.com.tr",
    initial_status: 200,
    final_status: 200,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/kategori/aksesuar/saat-kordonlari",
    response_time_ms: 95,
    content_type: "text/html; charset=utf-8",
    live_title: "Saat Kordonları",
    h1: "Saat Kordonları (0 Ürün)",
    word_count: 18,
    canonical: "https://magaza.ornek.com.tr/kategori/aksesuar/saat-kordonlari",
    noindex: false,
    category: "thin_200",
    reason: "thin_content_200",
    suggested_target: "https://magaza.ornek.com.tr/kategori/aksesuar/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/kurumsal/bayilik-basvurusu-eski",
    engines: "yandex,wayback",
    serp_title: "Bayilik Başvuru Formu — Örnek Mağaza",
    serp_snippet: "Türkiye genelinde bayilik başvurularımız devam etmektedir.",
    found_by_query: "wayback:cdx",
    initial_status: 410,
    final_status: 410,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/kurumsal/bayilik-basvurusu-eski",
    response_time_ms: 62,
    content_type: "text/html; charset=utf-8",
    live_title: "410 Gone",
    h1: "Bu sayfa kalıcı olarak kaldırıldı",
    word_count: 11,
    canonical: "",
    noindex: true,
    category: "error_404",
    reason: "http_410",
    suggested_target: "https://magaza.ornek.com.tr/kurumsal/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/bayi-ozel/fiyat-listesi-2025",
    engines: "google",
    serp_title: "2025 Toptan Bayi Fiyat Listesi",
    serp_snippet: "Yetkili bayilerimiz için güncel iskonto ve fiyat tablosu.",
    found_by_query: "site:magaza.ornek.com.tr",
    initial_status: 403,
    final_status: 403,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/bayi-ozel/fiyat-listesi-2025",
    response_time_ms: 71,
    content_type: "text/html; charset=utf-8",
    live_title: "403 Erişim Engellendi",
    h1: "Yetkisiz Erişim",
    word_count: 15,
    canonical: "",
    noindex: false,
    category: "error_4xx",
    reason: "http_403",
    suggested_target: "https://magaza.ornek.com.tr/bayi-ozel/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/odeme/taksit-hesaplayici",
    engines: "google,yandex",
    serp_title: "Kredi Kartı Taksit Hesaplayıcı",
    serp_snippet: "Bankalara göre vade farkı ve taksit seçeneklerini hesaplayın.",
    found_by_query: "site:magaza.ornek.com.tr",
    initial_status: 502,
    final_status: 502,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/odeme/taksit-hesaplayici",
    response_time_ms: 412,
    content_type: "text/html; charset=utf-8",
    live_title: "502 Bad Gateway",
    h1: "Sunucu Hatası",
    word_count: 12,
    canonical: "",
    noindex: false,
    category: "error_5xx",
    reason: "http_502",
    suggested_target: "https://magaza.ornek.com.tr/odeme/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/urunler/klasik-oxford-ayakkabi",
    engines: "google,yandex,bing,duckduckgo",
    serp_title: "Klasik Oxford Deri Ayakkabı | Örnek Mağaza",
    serp_snippet: "Hakiki dana derisinden üretilen el işçiliği klasik erkek ayakkabısı.",
    found_by_query: "site:magaza.ornek.com.tr/urunler",
    initial_status: 200,
    final_status: 200,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/urunler/klasik-oxford-ayakkabi",
    response_time_ms: 110,
    content_type: "text/html; charset=utf-8",
    live_title: "Klasik Oxford Deri Ayakkabı | Örnek Mağaza",
    h1: "Klasik Oxford Deri Ayakkabı",
    word_count: 485,
    canonical: "https://magaza.ornek.com.tr/urunler/klasik-oxford-ayakkabi",
    noindex: false,
    category: "healthy_200",
    reason: "healthy_200",
    suggested_target: "https://magaza.ornek.com.tr/urunler/",
    error_message: "",
  },
  {
    url: "https://magaza.ornek.com.tr/blog/keten-kumasi-nasil-yikanir",
    engines: "google,yandex,bing",
    serp_title: "Keten Kumaş Nasıl Yıkanır ve Ütülenir? — Bakım Rehberi",
    serp_snippet: "Keten kıyafetinizin ömrünü uzatacak yıkama ve kurutma ipuçları.",
    found_by_query: "site:magaza.ornek.com.tr/blog",
    initial_status: 200,
    final_status: 200,
    redirect_hops: 0,
    final_url: "https://magaza.ornek.com.tr/blog/keten-kumasi-nasil-yikanir",
    response_time_ms: 98,
    content_type: "text/html; charset=utf-8",
    live_title: "Keten Kumaş Nasıl Yıkanır ve Ütülenir?",
    h1: "Keten Kumaş Bakım Rehberi",
    word_count: 920,
    canonical: "https://magaza.ornek.com.tr/blog/keten-kumasi-nasil-yikanir",
    noindex: false,
    category: "healthy_200",
    reason: "healthy_200",
    suggested_target: "https://magaza.ornek.com.tr/blog/",
    error_message: "",
  },
];

const demoMeta: ScanMeta = {
  id: "demo-magaza-ornek-com-tr",
  domain: "magaza.ornek.com.tr",
  created_at_ms: 1792270000000,
  finished_at_ms: 1792270042000,
  status: "completed",
  config: defaultConfig,
  queries_executed: 18,
  discovered_urls: 10,
  verified_urls: 10,
  count_404: 3,
  count_4xx: 1,
  count_5xx: 1,
  count_soft_404: 2,
  count_thin_200: 1,
  count_network_error: 0,
  count_healthy: 2,
};

const olderDemoMeta: ScanMeta = {
  id: "demo-magaza-ornek-onceki",
  domain: "magaza.ornek.com.tr",
  created_at_ms: 1792180000000,
  finished_at_ms: 1792180038000,
  status: "completed",
  config: defaultConfig,
  queries_executed: 16,
  discovered_urls: 10,
  verified_urls: 10,
  count_404: 4,
  count_4xx: 1,
  count_5xx: 0,
  count_soft_404: 2,
  count_thin_200: 1,
  count_network_error: 0,
  count_healthy: 2,
};

let scansStore: ScanMeta[] = [demoMeta, olderDemoMeta];
const rowsByScan: Record<string, VerifiedUrl[]> = {
  [demoMeta.id]: demoRows.map((r) => ({ ...r })),
  [olderDemoMeta.id]: demoRows.map((r) =>
    r.url.includes("klasik-oxford-ayakkabi")
      ? {
          ...r,
          initial_status: 404,
          final_status: 404,
          category: "error_404",
          reason: "http_404",
        }
      : { ...r },
  ),
};

export const demoApi = {
  listScans(): ScanMeta[] {
    return scansStore;
  },
  overview(id: string): ScanOverview {
    const meta = scansStore.find((s) => s.id === id) ?? demoMeta;
    const rows = rowsByScan[meta.id] ?? demoRows;
    const engineMap = new Map<string, { total: number; issues: number }>();
    for (const r of rows) {
      for (const eng of r.engines.split(",").filter(Boolean)) {
        const cur = engineMap.get(eng) ?? { total: 0, issues: 0 };
        cur.total += 1;
        if (r.category !== "healthy_200") cur.issues += 1;
        engineMap.set(eng, cur);
      }
    }
    const clusterMap = new Map<
      string,
      {
        total: number;
        issues: number;
        c404: number;
        softThin: number;
        c4xx5xx: number;
        words: number;
        time: number;
      }
    >();
    for (const r of rows) {
      const seg = new URL(r.url).pathname.split("/").filter(Boolean)[0] ?? "";
      const prefix = seg ? `/${seg.toLowerCase()}` : "/";
      const cur = clusterMap.get(prefix) ?? {
        total: 0,
        issues: 0,
        c404: 0,
        softThin: 0,
        c4xx5xx: 0,
        words: 0,
        time: 0,
      };
      cur.total += 1;
      if (r.category !== "healthy_200") cur.issues += 1;
      if (r.category === "error_404") cur.c404 += 1;
      else if (r.category === "soft_404" || r.category === "thin_200")
        cur.softThin += 1;
      else if (r.category !== "healthy_200") cur.c4xx5xx += 1;
      cur.words += r.word_count;
      cur.time += r.response_time_ms;
      clusterMap.set(prefix, cur);
    }
    const clusters: PathCluster[] = Array.from(clusterMap.entries())
      .map(([prefix, v]) => ({
        prefix,
        total_urls: v.total,
        issue_urls: v.issues,
        count_404: v.c404,
        count_soft_thin: v.softThin,
        count_4xx_5xx: v.c4xx5xx,
        avg_word_count: Math.round(v.words / Math.max(1, v.total)),
        avg_response_ms: Math.round(v.time / Math.max(1, v.total)),
        action_hint:
          v.issues === 0
            ? "healthy_section"
            : v.c404 >= v.softThin && v.c404 >= v.c4xx5xx
              ? "redirect_section_301"
              : v.softThin >= v.c4xx5xx
                ? "thin_content_audit"
                : "server_5xx_check",
      }))
      .sort((a, b) => b.issue_urls - a.issue_urls || b.total_urls - a.total_urls);

    return {
      meta,
      health_score: Math.round(
        (meta.count_healthy / Math.max(1, meta.verified_urls)) * 100,
      ),
      engines: Array.from(engineMap.entries()).map(([engine, v]) => ({
        engine,
        total_urls: v.total,
        issue_urls: v.issues,
      })),
      clusters,
      top_issues: rows.filter((r) => r.category !== "healthy_200"),
    };
  },
  queryPage(id: string, req: FinderPageRequest): FinderTablePage {
    const rows = rowsByScan[id] ?? demoRows;
    const filtered = rows.filter((r) => {
      if (req.category === "issues" && r.category === "healthy_200") return false;
      if (
        req.category === "thin_soft_404" &&
        r.category !== "thin_200" &&
        r.category !== "soft_404"
      ) {
        return false;
      }
      if (req.category === "redirected" && r.redirect_hops === 0) {
        return false;
      }
      if (req.category === "noindex" && !r.noindex) {
        return false;
      }
      if (
        req.category === "sitemap_issues" &&
        (r.category === "healthy_200" || !r.engines.includes("sitemap"))
      ) {
        return false;
      }
      if (req.category === "orphan_indexed" && r.engines.includes("sitemap")) {
        return false;
      }
      if (
        req.category &&
        req.category !== "all" &&
        req.category !== "issues" &&
        req.category !== "thin_soft_404" &&
        req.category !== "redirected" &&
        req.category !== "noindex" &&
        req.category !== "sitemap_issues" &&
        req.category !== "orphan_indexed" &&
        r.category !== req.category
      ) {
        return false;
      }
      if (req.engine && req.engine !== "all") {
        if (req.engine === "multi") {
          if (!r.engines.includes(",")) return false;
        } else if (!r.engines.includes(req.engine)) {
          return false;
        }
      }
      if (req.search) {
        const q = req.search.toLowerCase();
        if (
          !r.url.toLowerCase().includes(q) &&
          !r.serp_title.toLowerCase().includes(q) &&
          !r.live_title.toLowerCase().includes(q)
        ) {
          return false;
        }
      }
      if (req.max_words != null && r.word_count > req.max_words) {
        return false;
      }
      return true;
    });

    const sorted = [...filtered];
    switch (req.sort_by) {
      case "words_asc":
        sorted.sort((a, b) => a.word_count - b.word_count);
        break;
      case "words_desc":
        sorted.sort((a, b) => b.word_count - a.word_count);
        break;
      case "time_desc":
        sorted.sort((a, b) => b.response_time_ms - a.response_time_ms);
        break;
      case "status_desc":
        sorted.sort((a, b) => b.final_status - a.final_status);
        break;
      case "hops_desc":
        sorted.sort((a, b) => b.redirect_hops - a.redirect_hops);
        break;
      case "engines_desc":
        sorted.sort(
          (a, b) =>
            b.engines.split(",").length - a.engines.split(",").length,
        );
        break;
      case "url_asc":
        sorted.sort((a, b) => a.url.localeCompare(b.url));
        break;
      default:
        break;
    }

    return {
      total: sorted.length,
      rows: sorted.slice(req.offset, req.offset + req.limit),
    };
  },
  redirectRules(id: string, server: RedirectServerFormat): string {
    const rows = (rowsByScan[id] ?? demoRows).filter(
      (r) =>
        r.category === "error_404" ||
        r.category === "error_4xx" ||
        r.category === "soft_404",
    );
    if (server === "nginx") {
      return [
        "# OctopusOn Index Checker — Nginx 301 Redirect Rules",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `rewrite ^${path}$ ${r.suggested_target} permanent; # ${r.category} (${r.engines})`;
        }),
      ].join("\n");
    }
    if (server === "nginx_cluster") {
      const overview = this.overview(id);
      return [
        "# OctopusOn Index Checker — Nginx Directory Cluster Wildcard 301 Rules",
        ...overview.clusters
          .filter((cl) => cl.issue_urls > 0 && cl.prefix !== "/")
          .map(
            (cl) =>
              `rewrite ^${cl.prefix}/.*$ https://${overview.meta.domain}${cl.prefix}/ permanent; # ${cl.issue_urls} kırık / ${cl.total_urls} toplam URL`,
          ),
      ].join("\n");
    }
    if (server === "cloudflare") {
      return [
        "# OctopusOn Index Checker — Cloudflare Pages / Netlify _redirects",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `${path} ${r.suggested_target} 301`;
        }),
      ].join("\n");
    }
    if (server === "cloudflare_csv") {
      return [
        "source_url,target_url,status_code,include_subdomains,subpath_matching,preserve_query_string,preserve_path_suffix",
        ...rows.map((r) => {
          const src = r.url.replace(/^https?:\/\//, "");
          return `"${src}","${r.suggested_target}",301,false,false,false,false`;
        }),
      ].join("\n");
    }
    if (server === "apache_410") {
      return [
        "# OctopusOn Index Checker — Apache .htaccess 410 Gone Rules",
        "RewriteEngine On",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `Redirect gone ${path} # ${r.category} (${r.engines})`;
        }),
      ].join("\n");
    }
    if (server === "vercel") {
      return JSON.stringify(
        {
          redirects: rows.map((r) => ({
            source: new URL(r.url).pathname,
            destination: r.suggested_target,
            permanent: true,
          })),
        },
        null,
        2,
      );
    }
    if (server === "nextjs") {
      return [
        "// OctopusOn Index Checker — next.config.mjs async redirects()",
        "export async function redirects() {",
        "  return [",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `    { source: ${JSON.stringify(path)}, destination: ${JSON.stringify(r.suggested_target)}, permanent: true },`;
        }),
        "  ];",
        "}",
      ].join("\n");
    }
    if (server === "caddy") {
      return [
        "# OctopusOn Index Checker — Caddyfile 301 Redirect Rules",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `redir ${path} ${r.suggested_target} permanent # ${r.category} (${r.engines})`;
        }),
      ].join("\n");
    }
    if (server === "iis") {
      return [
        "<!-- OctopusOn Index Checker — IIS web.config URL Rewrite Rules -->",
        "<configuration><system.webServer><rewrite><rules>",
        ...rows.map((r, i) => {
          const path = new URL(r.url).pathname.replace(/^\//, "");
          return `  <rule name="octopusSEO_301_${i + 1}" stopProcessing="true"><match url="^${path}$" /><action type="Redirect" url="${r.suggested_target}" redirectType="Permanent" /></rule>`;
        }),
        "</rules></rewrite></system.webServer></configuration>",
      ].join("\n");
    }
    if (server === "nginx_410") {
      return [
        "# OctopusOn Index Checker — Nginx 410 Gone Rules (Fast De-indexing)",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `location = ${path} { return 410; } # ${r.category} (${r.engines})`;
        }),
      ].join("\n");
    }
    if (server === "wordpress_csv") {
      return [
        "source,target,type,match",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `"${path}","${r.suggested_target}","301","url"`;
        }),
      ].join("\n");
    }
    if (server === "shopify_csv") {
      return [
        "Redirect from,Redirect to",
        ...rows.map((r) => {
          const path = new URL(r.url).pathname;
          return `"${path}","${r.suggested_target}"`;
        }),
      ].join("\n");
    }
    if (server === "removal_list") {
      return [
        "# OctopusOn Index Checker — Google Search Console / Yandex Webmaster URL Kaldırma Listesi",
        ...rows.map((r) => r.url),
      ].join("\n");
    }
    if (server === "clean_sitemap") {
      const allRows = rowsByScan[id] ?? demoRows;
      const healthyDirect = allRows.filter(
        (r) => r.category === "healthy_200" && !r.noindex && r.redirect_hops === 0,
      );
      return [
        '<?xml version="1.0" encoding="UTF-8"?>',
        "<!-- OctopusOn Index Checker — Clean Indexable Sitemap (200 OK, Direct, Indexable) -->",
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
        ...healthyDirect.map((r) => `  <url><loc>${r.url}</loc></url>`),
        "</urlset>",
      ].join("\n");
    }
    if (server === "redirect_chains_csv") {
      const allRows = rowsByScan[id] ?? demoRows;
      const chains = allRows.filter((r) => r.redirect_hops > 0);
      return [
        "indexed_url,initial_status,final_status,redirect_hops,final_url,engines",
        ...chains.map(
          (r) =>
            `"${r.url}",${r.initial_status},${r.final_status},${r.redirect_hops},"${r.final_url}","${r.engines}"`,
        ),
      ].join("\n");
    }
    if (server === "indexnow_json") {
      const host = rows[0] ? new URL(rows[0].url).hostname : "magaza.ornek.com.tr";
      return JSON.stringify(
        {
          host,
          key: "YOUR_INDEXNOW_KEY",
          urlList: rows.map((r) => r.url),
        },
        null,
        2,
      );
    }
    if (server === "markdown_report") {
      const ov = this.overview(id);
      return [
        "# OctopusOn Index Checker — İndeksli 404 & İçerik Sağlığı Denetim Raporu",
        "",
        `- **Toplam İndeksli URL:** ${ov.meta.verified_urls} | **İndeks Sağlığı:** %${ov.health_score}`,
        `- **404 / 410 Kırık:** ${ov.meta.count_404} | **Soft 404:** ${ov.meta.count_soft_404} | **200 İnce İçerik:** ${ov.meta.count_thin_200}`,
        "",
        "## Dizin Kümesi Sağlığı (Cluster Analizi)",
        "",
        "| Dizin | Toplam | 404/410 | Soft/İnce | Ort. Kelime | Önerilen Aksiyon |",
        "|---|---:|---:|---:|---:|---|",
        ...ov.clusters.map(
          (cl) =>
            `| \`${cl.prefix}\` | ${cl.total_urls} | ${cl.count_404} | ${cl.count_soft_thin} | ${cl.avg_word_count} | \`${cl.action_hint}\` |`,
        ),
        "",
        "## Müdahale Gerektiren İndeksli URL'ler ve Önerilen 301 Hedefleri",
        "",
        "| İndeksli URL | Motorlar | Durum | Sınıf | Önerilen 301 Hedefi |",
        "|---|---|---:|---|---|",
        ...rows.map(
          (r) =>
            `| \`${r.url}\` | ${r.engines} | ${r.final_status} | \`${r.category}\` | \`${r.suggested_target}\` |`,
        ),
      ].join("\n");
    }
    return [
      "# OctopusOn Index Checker — Apache .htaccess 301 Redirect Rules",
      "RewriteEngine On",
      ...rows.map((r) => {
        const path = new URL(r.url).pathname;
        return `Redirect 301 ${path} ${r.suggested_target} # ${r.category} (${r.engines})`;
      }),
    ].join("\n");
  },
  reverifyScan(id: string, url?: string): ScanOverview {
    return this.overview(id || (url ? demoMeta.id : demoMeta.id));
  },
  compareScans(baseId: string, targetId: string): ScanComparison {
    return {
      base_scan_id: baseId,
      target_scan_id: targetId,
      domain: "magaza.ornek.com.tr",
      resolved_count: 2,
      new_issue_count: 1,
      persistent_issue_count: 7,
      rows: [
        {
          url: "https://magaza.ornek.com.tr/odeme/taksit-hesaplayici",
          engines: "google,yandex",
          diff: "new_issue",
          base_status: 200,
          target_status: 502,
          base_category: "healthy_200",
          target_category: "error_5xx",
          suggested_target: "https://magaza.ornek.com.tr/odeme/",
        },
        {
          url: "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2024",
          engines: "google,yandex,wayback,sitemap",
          diff: "persistent_issue",
          base_status: 404,
          target_status: 404,
          base_category: "error_404",
          target_category: "error_404",
          suggested_target: "https://magaza.ornek.com.tr/urunler/",
        },
        {
          url: "https://magaza.ornek.com.tr/urunler/klasik-oxford-ayakkabi",
          engines: "google,yandex,bing,duckduckgo",
          diff: "resolved",
          base_status: 404,
          target_status: 200,
          base_category: "error_404",
          target_category: "healthy_200",
          suggested_target: "https://magaza.ornek.com.tr/urunler/",
        },
        {
          url: "https://magaza.ornek.com.tr/kampanya/eski-bahar-firsati-2022",
          engines: "google,wayback",
          diff: "resolved",
          base_status: 410,
          target_status: 0,
          base_category: "error_404",
          target_category: "healthy_200",
          suggested_target: "https://magaza.ornek.com.tr/kampanya/",
        },
      ],
    };
  },
  redirectCandidates(id: string, url: string): RedirectCandidate[] {
    const rows = rowsByScan[id] ?? demoRows;
    const foldTr = (s: string) =>
      s
        .toLowerCase()
        .replace(/[ç]/g, "c")
        .replace(/[ğ]/g, "g")
        .replace(/[ıi̇]/g, "i")
        .replace(/[ö]/g, "o")
        .replace(/[ş]/g, "s")
        .replace(/[ü]/g, "u");
    const sourceRow = rows.find((r) => r.url === url);
    const healthy = rows.filter(
      (r) => r.category === "healthy_200" && r.url !== url,
    );
    const sourceTokens = Array.from(
      new Set(
        `${new URL(url).pathname} ${sourceRow?.serp_title ?? ""}`
          .split(/[^a-zA-Z0-9çğıöşüÇĞİÖŞÜ]+/)
          .map(foldTr)
          .filter((tok) => tok.length >= 2 && !/^\d+$/.test(tok)),
      ),
    );
    const candidates: RedirectCandidate[] = healthy.map((h) => {
      const targetTokens = Array.from(
        new Set(
          `${new URL(h.url).pathname} ${h.live_title} ${h.serp_title}`
            .split(/[^a-zA-Z0-9çğıöşüÇĞİÖŞÜ]+/)
            .map(foldTr)
            .filter((tok) => tok.length >= 2 && !/^\d+$/.test(tok)),
        ),
      );
      const matched = sourceTokens.filter((t) => targetTokens.includes(t));
      const similarity = Math.min(
        95,
        Math.max(30, matched.length * 32 + (h.url.includes("/urunler/") ? 20 : 10)),
      );
      return {
        target_url: h.url,
        title: h.live_title || h.serp_title,
        similarity,
        matched_tokens: matched,
        kind: "smart_slug",
      };
    });
    candidates.sort((a, b) => b.similarity - a.similarity);
    const parent = url.replace(/\/[^/]+\/?$/, "/");
    if (!candidates.some((c) => c.target_url === parent)) {
      candidates.push({
        target_url: parent,
        title: "Üst Kategori / Dizin",
        similarity: 25,
        matched_tokens: [],
        kind: "category_fallback",
      });
    }
    return candidates;
  },
  updateRedirectTarget(id: string, url: string, target: string): void {
    const list = rowsByScan[id];
    if (!list) return;
    const trimmedTarget = target.trim();
    const selector = url.trim();
    if (selector.endsWith("*")) {
      const prefix = selector.slice(0, -1).trim();
      for (const r of list) {
        if (prefix && r.url.includes(prefix) && r.category !== "healthy_200") {
          r.suggested_target = trimmedTarget;
        }
      }
      return;
    }
    const urlSet = new Set(
      selector
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
    );
    for (const r of list) {
      if (urlSet.has(r.url)) {
        r.suggested_target = trimmedTarget;
      }
    }
  },
  deleteScan(id: string): void {
    scansStore = scansStore.filter((s) => s.id !== id);
  },
};

export function extractUrlsFromText(raw: string): string[] {
  const normalized = raw.replace(/&amp;/gi, "&");
  const matches = normalized.match(/https?:\/\/[^\s"',;<>()]+/gi) ?? [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const m of matches) {
    const cleaned = m.replace(/[.,;]+$/, "");
    if (!seen.has(cleaned)) {
      seen.add(cleaned);
      out.push(cleaned);
    }
  }
  return out;
}

export function formatVerifiedUrlsAsCsv(rows: VerifiedUrl[]): string {
  const esc = (v: string | number | boolean) =>
    `"${String(v).replace(/"/g, '""')}"`;
  const header = [
    "url",
    "engines",
    "category",
    "reason",
    "initial_status",
    "final_status",
    "redirect_hops",
    "final_url",
    "word_count",
    "response_time_ms",
    "serp_title",
    "live_title",
    "h1",
    "canonical",
    "noindex",
    "suggested_target",
  ].join(",");
  return [
    header,
    ...rows.map((r) =>
      [
        esc(r.url),
        esc(r.engines),
        esc(r.category),
        esc(r.reason),
        r.initial_status,
        r.final_status,
        r.redirect_hops,
        esc(r.final_url),
        r.word_count,
        r.response_time_ms,
        esc(r.serp_title),
        esc(r.live_title),
        esc(r.h1),
        esc(r.canonical),
        r.noindex,
        esc(r.suggested_target),
      ].join(","),
    ),
  ].join("\n");
}

export function downloadTextFile(
  filename: string,
  content: string,
  mime = "text/plain;charset=utf-8",
): void {
  if (typeof document === "undefined") return;
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function buildLiveOverviewFromJob(
  job: FinderJob,
  config: ScanConfig,
): ScanOverview {
  const rows = job.progress.live_rows ?? [];
  const p = job.progress;
  const countSoft = rows.filter((r) => r.category === "soft_404").length;
  const countThin = Math.max(0, p.found_thin_soft404 - countSoft);
  const countNet = rows.filter((r) => r.category === "network_error").length;
  const issues =
    p.found_404 + p.found_4xx + p.found_5xx + p.found_thin_soft404 + countNet;
  const verified = p.verified_urls;
  const healthy = Math.max(0, verified - issues);
  const healthScore =
    verified > 0 ? Math.round((healthy / Math.max(1, verified)) * 100) : 100;

  const engineMap = new Map<string, { total: number; issues: number }>();
  const clusterMap = new Map<
    string,
    {
      total: number;
      issues: number;
      c404: number;
      softThin: number;
      c4xx5xx: number;
      words: number;
      time: number;
    }
  >();

  for (const r of rows) {
    const isIssue = r.reason !== "checking" && r.category !== "healthy_200";
    for (const eng of r.engines.split(",").filter(Boolean)) {
      const cur = engineMap.get(eng) ?? { total: 0, issues: 0 };
      cur.total += 1;
      if (isIssue) cur.issues += 1;
      engineMap.set(eng, cur);
    }
    let prefix = "/";
    try {
      const seg = new URL(r.url).pathname.split("/").filter(Boolean)[0] ?? "";
      prefix = seg ? `/${seg.toLowerCase()}` : "/";
    } catch {
      prefix = "/";
    }
    const cur = clusterMap.get(prefix) ?? {
      total: 0,
      issues: 0,
      c404: 0,
      softThin: 0,
      c4xx5xx: 0,
      words: 0,
      time: 0,
    };
    cur.total += 1;
    if (isIssue) cur.issues += 1;
    if (r.category === "error_404") cur.c404 += 1;
    else if (r.category === "soft_404" || r.category === "thin_200")
      cur.softThin += 1;
    else if (isIssue) cur.c4xx5xx += 1;
    cur.words += r.word_count;
    cur.time += r.response_time_ms;
    clusterMap.set(prefix, cur);
  }

  const clusters: PathCluster[] = Array.from(clusterMap.entries())
    .map(([prefix, v]) => ({
      prefix,
      total_urls: v.total,
      issue_urls: v.issues,
      count_404: v.c404,
      count_soft_thin: v.softThin,
      count_4xx_5xx: v.c4xx5xx,
      avg_word_count: Math.round(v.words / Math.max(1, v.total)),
      avg_response_ms: Math.round(v.time / Math.max(1, v.total)),
      action_hint:
        v.issues === 0
          ? "healthy_section"
          : v.c404 >= v.softThin && v.c404 >= v.c4xx5xx
            ? "redirect_section_301"
            : v.softThin >= v.c4xx5xx
              ? "thin_content_audit"
              : "server_5xx_check",
    }))
    .sort((a, b) => b.issue_urls - a.issue_urls || b.total_urls - a.total_urls);

  return {
    meta: {
      id: job.scan_id ?? job.id,
      domain: job.domain,
      created_at_ms: Date.now(),
      finished_at_ms: Date.now(),
      status: job.status,
      config: { ...config, domain: job.domain },
      queries_executed: p.queries_completed,
      discovered_urls: p.discovered_urls,
      verified_urls: p.verified_urls || p.discovered_urls,
      count_404: p.found_404,
      count_4xx: p.found_4xx,
      count_5xx: p.found_5xx,
      count_soft_404: countSoft,
      count_thin_200: countThin,
      count_network_error: countNet,
      count_healthy: healthy,
    },
    health_score: healthScore,
    engines: Array.from(engineMap.entries()).map(([engine, v]) => ({
      engine,
      total_urls: v.total,
      issue_urls: v.issues,
    })),
    clusters,
    top_issues: rows.slice(0, 80),
  };
}

export function queryLiveRows(
  rows: VerifiedUrl[],
  req: FinderPageRequest,
): FinderTablePage {
  const filtered = rows.filter((r) => {
    if (
      req.category === "issues" &&
      r.category === "healthy_200" &&
      r.reason !== "checking"
    ) {
      return false;
    }
    if (
      req.category === "thin_soft_404" &&
      r.category !== "thin_200" &&
      r.category !== "soft_404"
    ) {
      return false;
    }
    if (req.category === "redirected" && r.redirect_hops === 0) {
      return false;
    }
    if (req.category === "noindex" && !r.noindex) {
      return false;
    }
    if (
      req.category === "sitemap_issues" &&
      (r.category === "healthy_200" || !r.engines.includes("sitemap"))
    ) {
      return false;
    }
    if (req.category === "orphan_indexed" && r.engines.includes("sitemap")) {
      return false;
    }
    if (
      req.category &&
      req.category !== "all" &&
      req.category !== "issues" &&
      req.category !== "thin_soft_404" &&
      req.category !== "redirected" &&
      req.category !== "noindex" &&
      req.category !== "sitemap_issues" &&
      req.category !== "orphan_indexed" &&
      r.category !== req.category
    ) {
      return false;
    }
    if (req.engine && req.engine !== "all") {
      if (req.engine === "multi") {
        if (!r.engines.includes(",")) return false;
      } else if (!r.engines.includes(req.engine)) {
        return false;
      }
    }
    if (req.search) {
      const q = req.search.toLowerCase();
      if (
        !r.url.toLowerCase().includes(q) &&
        !r.serp_title.toLowerCase().includes(q) &&
        !r.live_title.toLowerCase().includes(q)
      ) {
        return false;
      }
    }
    if (req.max_words != null && r.word_count > req.max_words) {
      return false;
    }
    return true;
  });

  return {
    total: filtered.length,
    rows: filtered.slice(req.offset, req.offset + req.limit),
  };
}

export function computeScanProgress(
  job: FinderJob,
  t: Translate,
): {
  percent: number;
  phaseTitle: string;
  detailText: string;
} {
  const p = job.progress;
  if (job.phase === "starting" || job.phase === "discovering") {
    const queryRatio =
      p.queries_total > 0 ? p.queries_completed / p.queries_total : 0;
    const percent = Math.min(45, Math.max(4, Math.round(queryRatio * 45)));
    return {
      percent,
      phaseTitle: t("phaseDiscover"),
      detailText: t("phaseDiscoverDetail", {
        done: p.queries_completed,
        total: Math.max(1, p.queries_total),
        found: p.discovered_urls,
      }),
    };
  }
  if (job.phase === "verifying") {
    const verifyRatio =
      p.discovered_urls > 0 ? p.verified_urls / p.discovered_urls : 0;
    const percent = Math.min(98, 45 + Math.round(verifyRatio * 53));
    return {
      percent,
      phaseTitle: t("phaseVerify"),
      detailText: t("phaseVerifyDetail", {
        checked: p.verified_urls,
        total: p.discovered_urls,
        broken: p.found_404,
        thin: p.found_thin_soft404,
      }),
    };
  }
  if (job.phase === "saving") {
    return {
      percent: 99,
      phaseTitle: t("phaseSaving"),
      detailText: t("checkedUrls", { count: p.verified_urls }),
    };
  }
  return {
    percent: 100,
    phaseTitle: t("phaseDone"),
    detailText: t("checkedUrls", { count: p.verified_urls }),
  };
}

