import { describe, expect, it } from "vitest";
import { demoApi, extractUrlsFromText } from "./api/demo";
import { categoryBadgeClass } from "./pages/Overview";

describe("OctopusOn Index Checker demo & helpers", () => {
  it("returns demo overview with 404, 4xx, 5xx, soft-404 and thin-200 counts", () => {
    const scans = demoApi.listScans();
    expect(scans.length).toBeGreaterThan(0);
    const overview = demoApi.overview(scans[0]!.id);
    expect(overview.meta.count_404).toBe(3);
    expect(overview.meta.count_soft_404).toBe(2);
    expect(overview.meta.count_thin_200).toBe(1);
    expect(overview.engines.some((e) => e.engine === "google")).toBe(true);
    expect(overview.engines.some((e) => e.engine === "yandex")).toBe(true);
    expect(overview.engines.some((e) => e.engine === "commoncrawl")).toBe(true);
    expect(overview.engines.some((e) => e.engine === "sitemap")).toBe(true);
  });

  it("filters thin_soft_404, redirected, and engine-specific URLs", () => {
    const scans = demoApi.listScans();
    const id = scans[0]!.id;
    const thinPage = demoApi.queryPage(id, {
      category: "thin_soft_404",
      engine: "",
      search: "",
      offset: 0,
      limit: 50,
    });
    expect(thinPage.total).toBe(3);

    const redirectedPage = demoApi.queryPage(id, {
      category: "redirected",
      engine: "",
      search: "",
      offset: 0,
      limit: 50,
    });
    expect(redirectedPage.total).toBeGreaterThan(0);
    expect(redirectedPage.rows.every((r) => r.redirect_hops > 0)).toBe(true);

    const yandexIssues = demoApi.queryPage(id, {
      category: "issues",
      engine: "yandex",
      search: "",
      offset: 0,
      limit: 50,
    });
    expect(yandexIssues.total).toBeGreaterThan(0);
    expect(yandexIssues.rows.every((r) => r.engines.includes("yandex"))).toBe(
      true,
    );

    const sitemapIssues = demoApi.queryPage(id, {
      category: "sitemap_issues",
      engine: "",
      search: "",
      offset: 0,
      limit: 50,
    });
    expect(sitemapIssues.total).toBeGreaterThan(0);
    expect(
      sitemapIssues.rows.every(
        (r) => r.category !== "healthy_200" && r.engines.includes("sitemap"),
      ),
    ).toBe(true);

    const orphanIndexed = demoApi.queryPage(id, {
      category: "orphan_indexed",
      engine: "",
      search: "",
      offset: 0,
      limit: 50,
    });
    expect(orphanIndexed.total).toBeGreaterThan(0);
    expect(
      orphanIndexed.rows.every((r) => !r.engines.includes("sitemap")),
    ).toBe(true);

    const multiEngineIssues = demoApi.queryPage(id, {
      category: "issues",
      engine: "multi",
      search: "",
      sort_by: "engines_desc",
      offset: 0,
      limit: 50,
    });
    expect(multiEngineIssues.total).toBeGreaterThan(0);
    expect(
      multiEngineIssues.rows.every((r) => r.engines.includes(",")),
    ).toBe(true);
  });

  it("extracts and deduplicates URLs from GSC / Yandex CSV and plain text exports", () => {
    const csvSample = [
      "URL,Last crawled",
      '"https://magaza.ornek.com.tr/urunler/eski-1",2026-10-01',
      '"https://magaza.ornek.com.tr/urunler/eski-2",2026-10-02',
      "https://magaza.ornek.com.tr/urunler/eski-1",
      "<loc>https://magaza.ornek.com.tr/urunler/eski-3?a=1&amp;b=2</loc>",
    ].join("\n");
    const urls = extractUrlsFromText(csvSample);
    expect(urls).toEqual([
      "https://magaza.ornek.com.tr/urunler/eski-1",
      "https://magaza.ornek.com.tr/urunler/eski-2",
      "https://magaza.ornek.com.tr/urunler/eski-3?a=1&b=2",
    ]);
  });

  it("formats redirect rules, clean XML sitemaps, redirect chains CSV, and IndexNow JSON", () => {
    const id = demoApi.listScans()[0]!.id;
    expect(demoApi.redirectRules(id, "nginx")).toContain("permanent;");
    expect(demoApi.redirectRules(id, "cloudflare")).toContain("301");
    expect(demoApi.redirectRules(id, "vercel")).toContain('"permanent": true');
    expect(demoApi.redirectRules(id, "nextjs")).toContain("export async function redirects()");
    expect(demoApi.redirectRules(id, "caddy")).toContain("redir /");
    expect(demoApi.redirectRules(id, "iis")).toContain("redirectType=\"Permanent\"");
    expect(demoApi.redirectRules(id, "nginx_410")).toContain("return 410;");
    expect(demoApi.redirectRules(id, "wordpress_csv")).toContain("source,target,type,match");
    expect(demoApi.redirectRules(id, "shopify_csv")).toContain("Redirect from,Redirect to");
    expect(demoApi.redirectRules(id, "removal_list")).toContain(
      "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2024",
    );
    expect(demoApi.redirectRules(id, "clean_sitemap")).toContain("<urlset");
    expect(demoApi.redirectRules(id, "redirect_chains_csv")).toContain(
      "indexed_url,initial_status,final_status,redirect_hops",
    );
    expect(demoApi.redirectRules(id, "indexnow_json")).toContain(
      '"urlList":',
    );
    expect(demoApi.redirectRules(id, "cloudflare_csv")).toContain(
      "source_url,target_url,status_code",
    );
    expect(demoApi.redirectRules(id, "apache_410")).toContain(
      "Redirect gone /",
    );
    expect(demoApi.redirectRules(id, "nginx_cluster")).toContain(
      "rewrite ^/urunler/.*$",
    );
    expect(demoApi.redirectRules(id, "markdown_report")).toContain(
      "# OctopusOn Index Checker",
    );
  });

  it("ranks smart 301 redirect candidates and updates custom target URLs", () => {
    const id = demoApi.listScans()[0]!.id;
    const brokenUrl = "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2024";
    const candidates = demoApi.redirectCandidates(id, brokenUrl);
    expect(candidates.length).toBeGreaterThan(0);
    expect(candidates[0]!.similarity).toBeGreaterThanOrEqual(30);

    demoApi.updateRedirectTarget(
      id,
      brokenUrl,
      "https://magaza.ornek.com.tr/urunler/yeni-koleksiyon",
    );
    expect(demoApi.redirectRules(id, "nginx")).toContain(
      "https://magaza.ornek.com.tr/urunler/yeni-koleksiyon",
    );

    demoApi.updateRedirectTarget(
      id,
      "/kampanya/*",
      "https://magaza.ornek.com.tr/kampanyalar/aktif",
    );
    expect(demoApi.redirectRules(id, "nginx")).toContain(
      "https://magaza.ornek.com.tr/kampanyalar/aktif",
    );
  });

  it("compares two scans and returns resolved, new, and persistent issues", () => {
    const scans = demoApi.listScans();
    expect(scans.length).toBeGreaterThanOrEqual(2);
    const diff = demoApi.compareScans(scans[1]!.id, scans[0]!.id);
    expect(diff.resolved_count).toBeGreaterThan(0);
    expect(diff.new_issue_count).toBeGreaterThan(0);
    expect(diff.persistent_issue_count).toBeGreaterThan(0);
  });

  it("computes directory clusters and supports sorting & max_words filtering", () => {
    const id = demoApi.listScans()[0]!.id;
    const overview = demoApi.overview(id);
    expect(overview.clusters.length).toBeGreaterThan(0);
    expect(overview.clusters.some((c) => c.prefix === "/urunler")).toBe(true);

    const sortedByWords = demoApi.queryPage(id, {
      category: "all",
      engine: "",
      search: "",
      max_words: 50,
      sort_by: "words_asc",
      offset: 0,
      limit: 50,
    });
    expect(sortedByWords.total).toBeGreaterThan(0);
    expect(sortedByWords.rows.every((r) => r.word_count <= 50)).toBe(true);
    if (sortedByWords.rows.length >= 2) {
      expect(sortedByWords.rows[0]!.word_count).toBeLessThanOrEqual(
        sortedByWords.rows[1]!.word_count,
      );
    }

    const sortedByHops = demoApi.queryPage(id, {
      category: "redirected",
      engine: "",
      search: "",
      max_words: null,
      sort_by: "hops_desc",
      offset: 0,
      limit: 50,
    });
    expect(sortedByHops.total).toBeGreaterThan(0);
    if (sortedByHops.rows.length >= 2) {
      expect(sortedByHops.rows[0]!.redirect_hops).toBeGreaterThanOrEqual(
        sortedByHops.rows[1]!.redirect_hops,
      );
    }
  });

  it("maps finding categories to Chromatophore status badge classes", () => {
    expect(categoryBadgeClass("error_404")).toContain("critical");
    expect(categoryBadgeClass("thin_200")).toContain("warning");
    expect(categoryBadgeClass("healthy_200")).toContain("good");
  });
});
