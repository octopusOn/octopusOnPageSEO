//! Scan comparison (`compare_scans`), live issue re-verification (`reverify_scan`), and multi-server redirect rule formatting.

use std::collections::BTreeMap;

use crate::FinderError;
use crate::model::{
    ComparedUrl, DiffStatus, DiscoveredUrl, FindingCategory, IndexEngine, ScanComparison,
    ScanControl, ScanOverview, ScanProgress, VerifiedUrl,
};
use crate::store::FinderStore;
use crate::verify;

/// Compares a baseline scan (`base_id`) with a newer target scan (`target_id`).
///
/// # Errors
/// Returns an error if either scan cannot be loaded from the store.
pub fn compare_scans(
    store: &FinderStore,
    base_id: &str,
    target_id: &str,
) -> Result<ScanComparison, FinderError> {
    let (_, base_meta, base_rows) = store.load_all_rows(base_id)?;
    let (_, target_meta, target_rows) = store.load_all_rows(target_id)?;

    let mut base_map: BTreeMap<String, VerifiedUrl> = BTreeMap::new();
    for row in base_rows {
        base_map.insert(row.url.clone(), row);
    }

    let mut resolved_count = 0u32;
    let mut new_issue_count = 0u32;
    let mut persistent_issue_count = 0u32;
    let mut diff_rows = Vec::new();

    for target in &target_rows {
        let base_entry = base_map.remove(&target.url);
        let base_is_issue = base_entry.as_ref().is_some_and(|b| b.category.is_issue());
        let target_is_issue = target.category.is_issue();

        let diff_kind = match (base_is_issue, target_is_issue) {
            (true, false) => {
                resolved_count = resolved_count.saturating_add(1);
                Some(DiffStatus::Resolved)
            }
            (false, true) => {
                new_issue_count = new_issue_count.saturating_add(1);
                Some(DiffStatus::NewIssue)
            }
            (true, true) => {
                persistent_issue_count = persistent_issue_count.saturating_add(1);
                Some(DiffStatus::PersistentIssue)
            }
            (false, false) => None,
        };

        if let Some(diff) = diff_kind {
            diff_rows.push(ComparedUrl {
                url: target.url.clone(),
                engines: target.engines.clone(),
                diff,
                base_status: base_entry.as_ref().map_or(0, |b| b.final_status),
                target_status: target.final_status,
                base_category: base_entry
                    .as_ref()
                    .map_or(FindingCategory::Healthy200, |b| b.category),
                target_category: target.category,
                suggested_target: target.suggested_target.clone(),
            });
        }
    }

    for base_row in base_map.into_values() {
        if base_row.category.is_issue() {
            resolved_count = resolved_count.saturating_add(1);
            diff_rows.push(ComparedUrl {
                url: base_row.url,
                engines: base_row.engines,
                diff: DiffStatus::Resolved,
                base_status: base_row.final_status,
                target_status: 0,
                base_category: base_row.category,
                target_category: FindingCategory::Healthy200,
                suggested_target: base_row.suggested_target,
            });
        }
    }

    diff_rows.sort_by(|a, b| {
        let rank = |d: DiffStatus| match d {
            DiffStatus::NewIssue => 0u8,
            DiffStatus::PersistentIssue => 1u8,
            DiffStatus::Resolved => 2u8,
        };
        rank(a.diff)
            .cmp(&rank(b.diff))
            .then_with(|| a.url.cmp(&b.url))
    });
    diff_rows.truncate(500);

    Ok(ScanComparison {
        base_scan_id: base_meta.id,
        target_scan_id: target_meta.id,
        domain: target_meta.domain,
        resolved_count,
        new_issue_count,
        persistent_issue_count,
        rows: diff_rows,
    })
}

/// Re-verifies either a single URL (`url_filter = Some(...)`) or all issue URLs in a saved scan,
/// updating `scan.duckdb` and `scan.json` in place.
///
/// # Errors
/// Returns an error if the scan does not exist or cannot be updated.
pub fn reverify_scan(
    store: &FinderStore,
    scan_id: &str,
    url_filter: Option<&str>,
) -> Result<ScanOverview, FinderError> {
    let (dir, meta, existing_rows) = store.load_all_rows(scan_id)?;
    let filter_trimmed = url_filter.map(str::trim).filter(|s| !s.is_empty());

    let mut targets = Vec::new();
    for row in &existing_rows {
        let should_check = if let Some(target_url) = filter_trimmed {
            row.url == target_url
        } else {
            row.category.is_issue()
        };
        if should_check {
            let parsed_engines: Vec<IndexEngine> = row
                .engines
                .split(',')
                .filter_map(|s| s.parse::<IndexEngine>().ok())
                .collect();
            targets.push(DiscoveredUrl {
                url: row.url.clone(),
                engines: parsed_engines,
                serp_title: row.serp_title.clone(),
                serp_snippet: row.serp_snippet.clone(),
                found_by_query: row.found_by_query.clone(),
            });
        }
    }

    if targets.is_empty() {
        return store.overview(&meta.id);
    }

    let control = ScanControl::new();
    let mut progress = ScanProgress::default();
    let refreshed =
        verify::verify_urls(&targets, &meta.config, &control, &mut progress, &mut |_| {});

    let mut refreshed_map: BTreeMap<String, VerifiedUrl> = BTreeMap::new();
    for item in refreshed {
        refreshed_map.insert(item.url.clone(), item);
    }

    let mut merged: Vec<VerifiedUrl> = existing_rows
        .into_iter()
        .map(|old| refreshed_map.remove(&old.url).unwrap_or(old))
        .collect();
    crate::matcher::enrich_smart_redirect_targets(&mut merged);

    let queries_executed = meta.queries_executed;
    let discovered_urls = meta.discovered_urls;
    let saved_id = meta.id.clone();
    store.finish_scan(
        &dir,
        meta,
        "completed",
        queries_executed,
        discovered_urls,
        &merged,
    )?;

    store.overview(&saved_id)
}

/// Formats 301 redirect rules, clean XML sitemaps, and redirect chain CSVs across server platforms.
pub fn format_redirect_rules(rows: &[VerifiedUrl], format: &str) -> String {
    let fmt = format.trim().to_ascii_lowercase();
    let actionable: Vec<(String, &VerifiedUrl)> = rows
        .iter()
        .filter(|r| {
            matches!(
                r.category,
                FindingCategory::Error404 | FindingCategory::Error4xx | FindingCategory::Soft404
            )
        })
        .filter_map(|r| {
            let parsed = url::Url::parse(&r.url).ok()?;
            let path = parsed.path();
            if path.is_empty() || path == "/" {
                return None;
            }
            Some((path.to_owned(), r))
        })
        .collect();

    match fmt.as_str() {
        "cloudflare" | "netlify" | "_redirects" => format_cloudflare_rules(&actionable),
        "cloudflare_csv" | "cloudflare_bulk" => format_cloudflare_bulk_csv(&actionable),
        "vercel" | "vercel.json" => format_vercel_rules(&actionable),
        "nextjs" | "next.config.js" | "next.config.mjs" => format_nextjs_rules(&actionable),
        "caddy" | "caddyfile" => format_caddy_rules(&actionable),
        "iis" | "web.config" => format_iis_rules(&actionable),
        "apache" | "htaccess" | ".htaccess" => format_apache_rules(&actionable),
        "apache_410" | "htaccess_410" => format_apache_410_rules(&actionable),
        "nginx_410" | "410" | "gone" => format_nginx_410_rules(&actionable),
        "nginx_cluster" | "cluster_301" => format_nginx_cluster_rules(rows),
        "wordpress" | "wordpress_csv" | "rankmath" => format_wordpress_csv(&actionable),
        "shopify" | "shopify_csv" => format_shopify_csv(&actionable),
        "removal" | "removal_list" | "gsc" | "urls" => format_removal_url_list(&actionable),
        "clean_sitemap" | "sitemap" | "sitemap.xml" => format_clean_sitemap_xml(rows),
        "redirect_chains" | "redirect_chains_csv" | "chains" => format_redirect_chains_csv(rows),
        "indexnow" | "indexnow_json" => format_indexnow_json(&actionable),
        "markdown" | "markdown_report" | "md" => format_markdown_report(rows),
        _ => format_nginx_rules(&actionable),
    }
}

fn format_markdown_report(rows: &[VerifiedUrl]) -> String {
    let total = rows.len();
    let count_404 = rows
        .iter()
        .filter(|r| r.category == FindingCategory::Error404)
        .count();
    let count_soft = rows
        .iter()
        .filter(|r| r.category == FindingCategory::Soft404)
        .count();
    let count_thin = rows
        .iter()
        .filter(|r| r.category == FindingCategory::Thin200)
        .count();
    let count_4xx_5xx = rows
        .iter()
        .filter(|r| {
            matches!(
                r.category,
                FindingCategory::Error4xx | FindingCategory::Error5xx
            )
        })
        .count();
    let count_healthy = rows
        .iter()
        .filter(|r| r.category == FindingCategory::Healthy200)
        .count();
    let health_pct = (count_healthy * 100) / total.max(1);
    let clusters = crate::cluster::build_path_clusters(rows);

    let mut lines = vec![
        "# OctopusOn Index Checker — İndeksli 404 & İçerik Sağlığı Denetim Raporu".to_owned(),
        String::new(),
        format!(
            "- **Toplam İndeksli URL:** {total} | **İndeks Sağlığı:** %{health_pct} ({count_healthy} sağlıklı 200 OK)"
        ),
        format!(
            "- **404 / 410 Kırık:** {count_404} | **Soft 404:** {count_soft} | **200 İnce İçerik:** {count_thin} | **4xx / 5xx:** {count_4xx_5xx}"
        ),
        String::new(),
        "## Dizin Kümesi Sağlığı (Cluster Analizi)".to_owned(),
        String::new(),
        "| Dizin | Toplam | 404/410 | Soft/İnce | Ort. Kelime | Önerilen Aksiyon |".to_owned(),
        "|---|---:|---:|---:|---:|---|".to_owned(),
    ];
    for cl in clusters {
        lines.push(format!(
            "| `{}` | {} | {} | {} | {} | `{}` |",
            cl.prefix,
            cl.total_urls,
            cl.count_404,
            cl.count_soft_thin,
            cl.avg_word_count,
            cl.action_hint
        ));
    }
    lines.push(String::new());
    lines.push("## Müdahale Gerektiren İndeksli URL'ler ve Önerilen 301 Hedefleri".to_owned());
    lines.push(String::new());
    lines.push("| İndeksli URL | Motorlar | Durum | Sınıf | Önerilen 301 Hedefi |".to_owned());
    lines.push("|---|---|---:|---|---|".to_owned());
    for row in rows.iter().filter(|r| r.category.is_issue()).take(200) {
        lines.push(format!(
            "| `{}` | {} | {} | `{}` | `{}` |",
            row.url,
            row.engines,
            row.final_status,
            row.category.as_str(),
            row.suggested_target
        ));
    }
    lines.join("\n")
}

fn format_nextjs_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "// OctopusOn Index Checker — next.config.mjs async redirects()".to_owned(),
        "export async function redirects() {".to_owned(),
        "  return [".to_owned(),
    ];
    for (clean_path, row) in items {
        lines.push(format!(
            "    {{ source: {:?}, destination: {:?}, permanent: true }},",
            clean_path, row.suggested_target
        ));
    }
    lines.push("  ];".to_owned());
    lines.push("}".to_owned());
    lines.join("\n")
}

fn format_caddy_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec!["# OctopusOn Index Checker — Caddyfile 301 Redirect Rules".to_owned()];
    for (clean_path, row) in items {
        lines.push(format!(
            "redir {} {} permanent # {} ({})",
            clean_path,
            row.suggested_target,
            row.category.as_str(),
            row.engines
        ));
    }
    lines.join("\n")
}

fn format_nginx_cluster_rules(rows: &[VerifiedUrl]) -> String {
    let origin = rows
        .first()
        .and_then(|r| url::Url::parse(&r.url).ok())
        .and_then(|u| u.host_str().map(|host| format!("{}://{host}", u.scheme())))
        .unwrap_or_else(|| "https://example.com".to_owned());
    let clusters = crate::cluster::build_path_clusters(rows);
    let mut lines =
        vec!["# OctopusOn Index Checker — Nginx Directory Cluster Wildcard 301 Rules".to_owned()];
    for cl in clusters {
        if cl.issue_urls > 0 && cl.prefix != "/" {
            lines.push(format!(
                "rewrite ^{}/.*$ {origin}{}/ permanent; # {} kırık / {} toplam URL",
                regex::escape(&cl.prefix),
                cl.prefix,
                cl.issue_urls,
                cl.total_urls
            ));
        }
    }
    lines.join("\n")
}

fn format_cloudflare_bulk_csv(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "source_url,target_url,status_code,include_subdomains,subpath_matching,preserve_query_string,preserve_path_suffix"
            .to_owned(),
    ];
    for (_, row) in items {
        let source_no_scheme = row
            .url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        lines.push(format!(
            "\"{}\",\"{}\",301,false,false,false,false",
            source_no_scheme.replace('"', "\"\""),
            row.suggested_target.replace('"', "\"\"")
        ));
    }
    lines.join("\n")
}

fn format_apache_410_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "# OctopusOn Index Checker — Apache .htaccess 410 Gone Rules".to_owned(),
        "RewriteEngine On".to_owned(),
    ];
    for (clean_path, row) in items {
        lines.push(format!(
            "Redirect gone {} # {} ({})",
            clean_path,
            row.category.as_str(),
            row.engines
        ));
    }
    lines.join("\n")
}

fn format_indexnow_json(items: &[(String, &VerifiedUrl)]) -> String {
    let host = items
        .first()
        .and_then(|(_, r)| url::Url::parse(&r.url).ok())
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "example.com".to_owned());
    let url_list: Vec<&str> = items.iter().map(|(_, r)| r.url.as_str()).collect();
    let payload = serde_json::json!({
        "host": host,
        "key": "YOUR_INDEXNOW_KEY",
        "urlList": url_list
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_owned())
}

fn format_clean_sitemap_xml(rows: &[VerifiedUrl]) -> String {
    let mut lines = vec![
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_owned(),
        "<!-- OctopusOn Index Checker — Clean Indexable Sitemap (200 OK, Direct, Indexable) -->"
            .to_owned(),
        "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">".to_owned(),
    ];
    for row in rows {
        if row.category == FindingCategory::Healthy200 && !row.noindex && row.redirect_hops == 0 {
            let escaped = row
                .url
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;");
            lines.push(format!("  <url><loc>{escaped}</loc></url>"));
        }
    }
    lines.push("</urlset>".to_owned());
    lines.join("\n")
}

fn format_redirect_chains_csv(rows: &[VerifiedUrl]) -> String {
    let mut lines =
        vec!["indexed_url,initial_status,final_status,redirect_hops,final_url,engines".to_owned()];
    for row in rows.iter().filter(|r| r.redirect_hops > 0) {
        lines.push(format!(
            "\"{}\",{},{},{},\"{}\",\"{}\"",
            row.url.replace('"', "\"\""),
            row.initial_status,
            row.final_status,
            row.redirect_hops,
            row.final_url.replace('"', "\"\""),
            row.engines.replace('"', "\"\"")
        ));
    }
    lines.join("\n")
}

fn format_wordpress_csv(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec!["source,target,type,match".to_owned()];
    for (clean_path, row) in items {
        lines.push(format!(
            "\"{}\",\"{}\",\"301\",\"url\"",
            clean_path.replace('"', "\"\""),
            row.suggested_target.replace('"', "\"\"")
        ));
    }
    lines.join("\n")
}

fn format_shopify_csv(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec!["Redirect from,Redirect to".to_owned()];
    for (clean_path, row) in items {
        lines.push(format!(
            "\"{}\",\"{}\"",
            clean_path.replace('"', "\"\""),
            row.suggested_target.replace('"', "\"\"")
        ));
    }
    lines.join("\n")
}

fn format_nginx_410_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "# OctopusOn Index Checker — Nginx 410 Gone Rules (Fast De-indexing)".to_owned(),
        "# Kalıcı olarak kaldırılan URL'ler için arama motorlarına 410 Gone döndürür".to_owned(),
    ];
    for (clean_path, row) in items {
        lines.push(format!(
            "location = {} {{ return 410; }} # {} ({})",
            clean_path,
            row.category.as_str(),
            row.engines
        ));
    }
    lines.join("\n")
}

fn format_removal_url_list(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "# OctopusOn Index Checker — Google Search Console / Yandex Webmaster URL Kaldırma Listesi"
            .to_owned(),
    ];
    for (_, row) in items {
        lines.push(row.url.clone());
    }
    lines.join("\n")
}

fn format_nginx_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec!["# OctopusOn Index Checker — Nginx 301 Redirect Rules".to_owned()];
    for (clean_path, row) in items {
        lines.push(format!(
            "rewrite ^{}$ {} permanent; # {} ({})",
            regex::escape(clean_path),
            row.suggested_target,
            row.category.as_str(),
            row.engines
        ));
    }
    lines.join("\n")
}

fn format_apache_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "# OctopusOn Index Checker — Apache .htaccess 301 Redirect Rules".to_owned(),
        "RewriteEngine On".to_owned(),
    ];
    for (clean_path, row) in items {
        lines.push(format!(
            "Redirect 301 {} {} # {} ({})",
            clean_path,
            row.suggested_target,
            row.category.as_str(),
            row.engines
        ));
    }
    lines.join("\n")
}

fn format_cloudflare_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines =
        vec!["# OctopusOn Index Checker — Cloudflare Pages / Netlify _redirects".to_owned()];
    for (clean_path, row) in items {
        lines.push(format!("{clean_path} {} 301", row.suggested_target));
    }
    lines.join("\n")
}

fn format_vercel_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let redirects: Vec<serde_json::Value> = items
        .iter()
        .map(|(clean_path, row)| {
            serde_json::json!({
                "source": clean_path,
                "destination": row.suggested_target,
                "permanent": true
            })
        })
        .collect();
    let root = serde_json::json!({ "redirects": redirects });
    serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{\"redirects\":[]}".to_owned())
}

fn format_iis_rules(items: &[(String, &VerifiedUrl)]) -> String {
    let mut lines = vec![
        "<!-- OctopusOn Index Checker — IIS web.config URL Rewrite Rules -->".to_owned(),
        "<configuration><system.webServer><rewrite><rules>".to_owned(),
    ];
    for (idx, (url_path, row)) in items.iter().enumerate() {
        let clean_path = url_path.trim_start_matches('/');
        lines.push(format!(
            "  <rule name=\"octopusSEO_301_{}\" stopProcessing=\"true\"><match url=\"^{}$\" /><action type=\"Redirect\" url=\"{}\" redirectType=\"Permanent\" /></rule>",
            idx + 1,
            regex::escape(clean_path),
            row.suggested_target
        ));
    }
    lines.push("</rules></rewrite></system.webServer></configuration>".to_owned());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_redirect_rows() -> Vec<VerifiedUrl> {
        vec![
            VerifiedUrl {
                url: "https://acme.com/urun/eski-model".to_owned(),
                engines: "google,yandex".to_owned(),
                serp_title: "Eski Model".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 404,
                final_status: 404,
                redirect_hops: 0,
                final_url: "https://acme.com/urun/eski-model".to_owned(),
                response_time_ms: 40,
                content_type: "text/html".to_owned(),
                live_title: "404".to_owned(),
                h1: "404".to_owned(),
                word_count: 5,
                canonical: String::new(),
                noindex: false,
                category: FindingCategory::Error404,
                reason: "http_404".to_owned(),
                suggested_target: "https://acme.com/urun/".to_owned(),
                error_message: String::new(),
            },
            VerifiedUrl {
                url: "https://acme.com/urun/yeni-model".to_owned(),
                engines: "google".to_owned(),
                serp_title: "Yeni Model".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 200,
                final_status: 200,
                redirect_hops: 0,
                final_url: "https://acme.com/urun/yeni-model".to_owned(),
                response_time_ms: 35,
                content_type: "text/html".to_owned(),
                live_title: "Yeni Model".to_owned(),
                h1: "Yeni Model".to_owned(),
                word_count: 600,
                canonical: "https://acme.com/urun/yeni-model".to_owned(),
                noindex: false,
                category: FindingCategory::Healthy200,
                reason: "healthy_200".to_owned(),
                suggested_target: "https://acme.com/urun/".to_owned(),
                error_message: String::new(),
            },
            VerifiedUrl {
                url: "https://acme.com/eski-yol".to_owned(),
                engines: "yandex".to_owned(),
                serp_title: "Eski Yol".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 301,
                final_status: 200,
                redirect_hops: 2,
                final_url: "https://acme.com/urun/yeni-model".to_owned(),
                response_time_ms: 80,
                content_type: "text/html".to_owned(),
                live_title: "Yeni Model".to_owned(),
                h1: "Yeni Model".to_owned(),
                word_count: 600,
                canonical: "https://acme.com/urun/yeni-model".to_owned(),
                noindex: false,
                category: FindingCategory::Healthy200,
                reason: "healthy_200".to_owned(),
                suggested_target: "https://acme.com/".to_owned(),
                error_message: String::new(),
            },
        ]
    }

    #[test]
    fn formats_cloudflare_and_vercel_redirect_rules() {
        let sample = sample_redirect_rows();

        let cf = format_redirect_rules(&sample, "cloudflare");
        assert!(cf.contains("/urun/eski-model https://acme.com/urun/ 301"));

        let vercel = format_redirect_rules(&sample, "vercel");
        assert!(vercel.contains("\"/urun/eski-model\""));
        assert!(vercel.contains("\"permanent\": true"));

        let gone = format_redirect_rules(&sample, "nginx_410");
        assert!(gone.contains("location = /urun/eski-model { return 410; }"));

        let wp = format_redirect_rules(&sample, "wordpress_csv");
        assert!(wp.contains("\"/urun/eski-model\",\"https://acme.com/urun/\",\"301\",\"url\""));

        let shopify = format_redirect_rules(&sample, "shopify_csv");
        assert!(shopify.contains("\"/urun/eski-model\",\"https://acme.com/urun/\""));

        let removal = format_redirect_rules(&sample, "removal_list");
        assert!(removal.contains("https://acme.com/urun/eski-model"));

        let sitemap = format_redirect_rules(&sample, "clean_sitemap");
        assert!(sitemap.contains("<loc>https://acme.com/urun/yeni-model</loc>"));
        assert!(!sitemap.contains("eski-model"));
        assert!(!sitemap.contains("eski-yol"));

        let chains = format_redirect_rules(&sample, "redirect_chains_csv");
        assert!(chains.contains("indexed_url,initial_status,final_status,redirect_hops"));
        assert!(chains.contains("\"https://acme.com/eski-yol\",301,200,2"));

        let indexnow = format_redirect_rules(&sample, "indexnow_json");
        assert!(indexnow.contains("\"host\": \"acme.com\""));
        assert!(indexnow.contains("https://acme.com/urun/eski-model"));

        let cf_csv = format_redirect_rules(&sample, "cloudflare_csv");
        assert!(
            cf_csv.contains("\"acme.com/urun/eski-model\",\"https://acme.com/urun/\",301,false")
        );

        let ap_410 = format_redirect_rules(&sample, "apache_410");
        assert!(ap_410.contains("Redirect gone /urun/eski-model"));

        let ng_cluster = format_redirect_rules(&sample, "nginx_cluster");
        assert!(ng_cluster.contains("rewrite ^/urun/.*$ https://acme.com/urun/ permanent;"));

        let nextjs = format_redirect_rules(&sample, "nextjs");
        assert!(nextjs.contains("source: \"/urun/eski-model\""));

        let caddy = format_redirect_rules(&sample, "caddy");
        assert!(caddy.contains("redir /urun/eski-model https://acme.com/urun/ permanent"));

        let md = format_redirect_rules(&sample, "markdown_report");
        assert!(md.contains("# OctopusOn Index Checker"));
        assert!(md.contains("| `https://acme.com/urun/eski-model` |"));
    }
}
