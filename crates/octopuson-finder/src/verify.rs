//! Live HTTP and HTML content verification for discovered indexed URLs.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use octopuson_crawl::{FetchError, FetchRequest, FetchResponse, Fetcher};
use octopuson_html::{decode_html, parse_directives, parse_page};
use octopuson_net::CrawlFetcher;
use rayon::prelude::*;
use url::Url;

use crate::model::{
    DiscoveredUrl, FindingCategory, IndexEngine, ScanConfig, ScanControl, ScanProgress, VerifiedUrl,
};

/// Maximum response body read per verified URL (2 MiB).
const BODY_LIMIT: u64 = 2 * 1024 * 1024;

/// Verifies all `discovered` URLs concurrently using a bounded worker pool.
pub fn verify_urls(
    discovered: &[DiscoveredUrl],
    config: &ScanConfig,
    control: &ScanControl,
    progress: &mut ScanProgress,
    on_progress: &mut (dyn FnMut(&ScanProgress) + Send),
) -> Vec<VerifiedUrl> {
    let timeout = Duration::from_secs(u64::from(config.timeout_seconds.clamp(3, 60)));
    let fetcher = CrawlFetcher::new(
        &config.user_agent,
        timeout,
        Some("tr-TR,tr;q=0.9,en-US;q=0.8,en;q=0.7".to_owned()),
    );
    verify_urls_with_fetcher(discovered, config, &fetcher, control, progress, on_progress)
}

/// Verifies all `discovered` URLs with any [`Fetcher`] (used by tests and the live engine).
pub fn verify_urls_with_fetcher(
    discovered: &[DiscoveredUrl],
    config: &ScanConfig,
    fetcher: &(dyn Fetcher + Sync),
    control: &ScanControl,
    progress: &mut ScanProgress,
    on_progress: &mut (dyn FnMut(&ScanProgress) + Send),
) -> Vec<VerifiedUrl> {
    let threads = usize::from(config.concurrency.clamp(1, 32));
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build();

    let state = Mutex::new((progress.clone(), Vec::with_capacity(discovered.len())));
    let callback = Mutex::new(on_progress);

    let run = || {
        discovered.par_iter().for_each(|item| {
            if !control.wait_if_paused() {
                return;
            }
            let verified = verify_one(item, config, fetcher);
            if let Ok(mut guard) = state.lock() {
                let (prog, list) = &mut *guard;
                prog.verified_urls = prog.verified_urls.saturating_add(1);
                prog.current_item.clone_from(&verified.url);
                match verified.category {
                    FindingCategory::Error404 => {
                        prog.found_404 = prog.found_404.saturating_add(1);
                    }
                    FindingCategory::Error4xx => {
                        prog.found_4xx = prog.found_4xx.saturating_add(1);
                    }
                    FindingCategory::Error5xx => {
                        prog.found_5xx = prog.found_5xx.saturating_add(1);
                    }
                    FindingCategory::Soft404 | FindingCategory::Thin200 => {
                        prog.found_thin_soft404 = prog.found_thin_soft404.saturating_add(1);
                    }
                    FindingCategory::NetworkError | FindingCategory::Healthy200 => {}
                }
                let snapshot = {
                    upsert_live_row(&mut prog.live_rows, &verified);
                    prog.clone()
                };
                list.push(verified);
                drop(guard);
                if let Ok(mut cb) = callback.lock() {
                    cb(&snapshot);
                }
            }
        });
    };

    if let Ok(pool) = pool {
        pool.install(run);
    } else {
        run();
    }

    let Ok((final_progress, mut results)) = state.into_inner() else {
        return Vec::new();
    };
    *progress = final_progress;
    results.sort_by(|a, b| {
        category_priority(a.category)
            .cmp(&category_priority(b.category))
            .then_with(|| a.url.cmp(&b.url))
    });
    results
}

fn upsert_live_row(live_rows: &mut Vec<VerifiedUrl>, verified: &VerifiedUrl) {
    if let Some(existing) = live_rows.iter_mut().find(|r| r.url == verified.url) {
        existing.clone_from(verified);
    } else {
        live_rows.push(verified.clone());
    }
    live_rows.sort_by(|a, b| {
        let rank_a = if a.reason == "checking" {
            7
        } else {
            category_priority(a.category)
        };
        let rank_b = if b.reason == "checking" {
            7
        } else {
            category_priority(b.category)
        };
        rank_a.cmp(&rank_b).then_with(|| a.url.cmp(&b.url))
    });
    live_rows.truncate(250);
}

const fn category_priority(cat: FindingCategory) -> u8 {
    match cat {
        FindingCategory::Error404 => 0,
        FindingCategory::Soft404 => 1,
        FindingCategory::Thin200 => 2,
        FindingCategory::Error4xx => 3,
        FindingCategory::Error5xx => 4,
        FindingCategory::NetworkError => 5,
        FindingCategory::Healthy200 => 6,
    }
}

/// Verifies a single discovered URL, following up to `config.max_redirects` hops.
pub fn verify_one(item: &DiscoveredUrl, config: &ScanConfig, fetcher: &dyn Fetcher) -> VerifiedUrl {
    let engines_str = item
        .engines
        .iter()
        .map(|e| IndexEngine::as_str(*e))
        .collect::<Vec<_>>()
        .join(",");
    let suggested = suggest_redirect_target(&item.url);

    let started = Instant::now();
    let max_hops = config.max_redirects.min(10);
    let (current_url, initial_status, redirect_hops, final_response) =
        fetch_following_redirects(fetcher, &item.url, max_hops);
    let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);

    let resp = match final_response {
        Ok(resp) => resp,
        Err(err) => {
            return VerifiedUrl {
                url: item.url.clone(),
                engines: engines_str,
                serp_title: item.serp_title.clone(),
                serp_snippet: item.serp_snippet.clone(),
                found_by_query: item.found_by_query.clone(),
                initial_status,
                final_status: 0,
                redirect_hops,
                final_url: current_url,
                response_time_ms: elapsed_ms,
                content_type: String::new(),
                live_title: String::new(),
                h1: String::new(),
                word_count: 0,
                canonical: String::new(),
                noindex: false,
                category: FindingCategory::NetworkError,
                reason: "network_error".to_owned(),
                suggested_target: suggested,
                error_message: err.to_string(),
            };
        }
    };

    let final_status = resp.status;
    let content_type = resp.header("content-type").unwrap_or_default().to_owned();
    let is_html = content_type.is_empty()
        || content_type.to_ascii_lowercase().contains("html")
        || content_type.to_ascii_lowercase().contains("xhtml");
    let header_noindex = resp
        .header("x-robots-tag")
        .is_some_and(|v| parse_directives(v, &["googlebot", "yandex"]).noindex);

    let html = extract_html_facts(
        &resp.body,
        &content_type,
        &current_url,
        is_html,
        header_noindex,
    );

    let (category, reason) = classify_outcome(
        &item.url,
        &current_url,
        redirect_hops,
        final_status,
        is_html,
        &html,
        config,
    );

    VerifiedUrl {
        url: item.url.clone(),
        engines: engines_str,
        serp_title: item.serp_title.clone(),
        serp_snippet: item.serp_snippet.clone(),
        found_by_query: item.found_by_query.clone(),
        initial_status,
        final_status,
        redirect_hops,
        final_url: current_url,
        response_time_ms: elapsed_ms,
        content_type,
        live_title: html.live_title,
        h1: html.h1,
        word_count: html.word_count,
        canonical: html.canonical,
        noindex: html.noindex,
        category,
        reason,
        suggested_target: suggested,
        error_message: String::new(),
    }
}

#[derive(Default)]
struct ExtractedHtmlFacts {
    live_title: String,
    h1: String,
    word_count: u32,
    canonical: String,
    meta_refresh_url: String,
    noindex: bool,
}

fn extract_html_facts(
    body: &[u8],
    content_type: &str,
    current_url: &str,
    is_html: bool,
    header_noindex: bool,
) -> ExtractedHtmlFacts {
    if !is_html || body.is_empty() {
        return ExtractedHtmlFacts {
            noindex: header_noindex,
            ..ExtractedHtmlFacts::default()
        };
    }
    let decoded = decode_html(body, Some(content_type));
    let Ok(parsed_url) = Url::parse(current_url) else {
        return ExtractedHtmlFacts {
            noindex: header_noindex,
            ..ExtractedHtmlFacts::default()
        };
    };
    let facts = parse_page(&decoded, &parsed_url, &["googlebot", "yandex"]);
    let live_title = facts.title().unwrap_or_default().to_owned();
    let h1 = facts.headings_of(1).next().unwrap_or_default().to_owned();
    let word_count = facts.word_count;
    let canonical = facts.canonicals.first().cloned().unwrap_or_default();
    let meta_refresh_url = facts.meta_refresh.and_then(|mr| mr.url).unwrap_or_default();
    let noindex = header_noindex || facts.directives.noindex;
    ExtractedHtmlFacts {
        live_title,
        h1,
        word_count,
        canonical,
        meta_refresh_url,
        noindex,
    }
}

fn fetch_following_redirects(
    fetcher: &dyn Fetcher,
    start_url: &str,
    max_hops: u8,
) -> (String, u16, u8, Result<FetchResponse, FetchError>) {
    let mut current_url = start_url.to_owned();
    let mut visited = std::collections::BTreeSet::new();
    visited.insert(current_url.clone());
    let mut initial_status = 0u16;
    let mut redirect_hops = 0u8;

    let final_response = loop {
        let req = FetchRequest {
            url: &current_url,
            body_limit: BODY_LIMIT,
        };
        match fetcher.fetch(&req) {
            Ok(resp) => {
                if initial_status == 0 {
                    initial_status = resp.status;
                }
                if matches!(resp.status, 301 | 302 | 303 | 307 | 308)
                    && redirect_hops < max_hops
                    && let Some(loc) = resp.header("location")
                    && let Ok(base) = Url::parse(&current_url)
                    && let Ok(mut next) = base.join(loc.trim())
                {
                    next.set_fragment(None);
                    let next_str = next.to_string();
                    if visited.insert(next_str.clone()) {
                        current_url = next_str;
                        redirect_hops = redirect_hops.saturating_add(1);
                        continue;
                    }
                }
                break Ok(resp);
            }
            Err(err) => break Err(err),
        }
    };

    (current_url, initial_status, redirect_hops, final_response)
}

fn classify_outcome(
    original_url: &str,
    final_url: &str,
    redirect_hops: u8,
    final_status: u16,
    is_html: bool,
    html: &ExtractedHtmlFacts,
    config: &ScanConfig,
) -> (FindingCategory, String) {
    // 1. Definite 404 or 410
    if final_status == 404 {
        let reason = if redirect_hops > 0 {
            "redirect_to_404"
        } else {
            "http_404"
        };
        return (FindingCategory::Error404, reason.to_owned());
    }
    if final_status == 410 {
        let reason = if redirect_hops > 0 {
            "redirect_to_410"
        } else {
            "http_410"
        };
        return (FindingCategory::Error404, reason.to_owned());
    }

    // 2. Unresolved 3xx redirect loop / too many redirects
    if (300..400).contains(&final_status) {
        if config.detect_all_4xx {
            return (FindingCategory::Error4xx, "redirect_loop".to_owned());
        }
        return (
            FindingCategory::Healthy200,
            format!("ignored_{final_status}"),
        );
    }

    // 3. Other 4xx client errors
    if (400..500).contains(&final_status) {
        if config.detect_all_4xx {
            return (FindingCategory::Error4xx, format!("http_{final_status}"));
        }
        return (
            FindingCategory::Healthy200,
            format!("ignored_{final_status}"),
        );
    }

    // 3. 5xx server errors
    if (500..600).contains(&final_status) {
        if config.detect_5xx {
            return (FindingCategory::Error5xx, format!("http_{final_status}"));
        }
        return (
            FindingCategory::Healthy200,
            format!("ignored_{final_status}"),
        );
    }

    // 4. 200 OK Soft-404 and Thin Content checks
    if final_status == 200 && config.detect_thin_200 {
        if redirect_hops > 0 && is_deep_to_root_redirect(original_url, final_url) {
            return (FindingCategory::Soft404, "redirect_to_root".to_owned());
        }
        if redirect_hops > 0 && is_redirect_to_error_slug(original_url, final_url) {
            return (FindingCategory::Soft404, "redirect_to_404_slug".to_owned());
        }
        if is_html
            && !html.meta_refresh_url.is_empty()
            && (is_deep_to_root_redirect(final_url, &html.meta_refresh_url)
                || is_redirect_to_error_slug(final_url, &html.meta_refresh_url))
        {
            return (FindingCategory::Soft404, "meta_refresh_soft_404".to_owned());
        }
        if is_html && !html.canonical.is_empty() {
            if is_deep_to_root_redirect(final_url, &html.canonical) {
                return (FindingCategory::Soft404, "canonical_to_root".to_owned());
            }
            if is_redirect_to_error_slug(final_url, &html.canonical) {
                return (FindingCategory::Soft404, "canonical_to_404_slug".to_owned());
            }
            if is_external_canonical(final_url, &html.canonical) {
                return (FindingCategory::Soft404, "canonical_external".to_owned());
            }
        }
        if is_html && looks_like_soft_404_text(&html.live_title, &html.h1) {
            return (FindingCategory::Soft404, "soft_404_phrase".to_owned());
        }
        if is_html && html.word_count < config.min_word_count {
            return (FindingCategory::Thin200, "thin_content_200".to_owned());
        }
    }

    (FindingCategory::Healthy200, "healthy_200".to_owned())
}

fn is_external_canonical(final_url: &str, canonical_url: &str) -> bool {
    let (Ok(fin), Ok(can)) = (Url::parse(final_url), Url::parse(canonical_url)) else {
        return false;
    };
    let Some(fin_host) = fin.host_str().map(|h| {
        h.to_ascii_lowercase()
            .strip_prefix("www.")
            .unwrap_or(&h.to_ascii_lowercase())
            .to_owned()
    }) else {
        return false;
    };
    let Some(can_host) = can.host_str().map(|h| {
        h.to_ascii_lowercase()
            .strip_prefix("www.")
            .unwrap_or(&h.to_ascii_lowercase())
            .to_owned()
    }) else {
        return false;
    };
    fin_host != can_host
        && !can_host.ends_with(&format!(".{fin_host}"))
        && !fin_host.ends_with(&format!(".{can_host}"))
}

fn is_deep_to_root_redirect(original_url: &str, final_url: &str) -> bool {
    let (Ok(orig), Ok(fin)) = (Url::parse(original_url), Url::parse(final_url)) else {
        return false;
    };
    let orig_trimmed = orig.path().trim_matches('/');
    let fin_trimmed = fin.path().trim_matches('/');
    !orig_trimmed.is_empty() && fin_trimmed.is_empty() && fin.query().is_none()
}

fn is_redirect_to_error_slug(original_url: &str, final_url: &str) -> bool {
    let (Ok(orig), Ok(fin)) = (Url::parse(original_url), Url::parse(final_url)) else {
        return false;
    };
    let orig_slug = orig.path().trim_matches('/').to_ascii_lowercase();
    let fin_slug = fin.path().trim_matches('/').to_ascii_lowercase();
    if orig_slug == fin_slug && orig.query() == fin.query() {
        return false;
    }
    if let Some(q) = fin.query() {
        let q_lower = q.to_ascii_lowercase();
        if q_lower.contains("error/not_found")
            || q_lower.contains("error=404")
            || q_lower.contains("page=404")
            || q_lower.contains("status=404")
        {
            return true;
        }
    }
    matches!(
        fin_slug.as_str(),
        "404"
            | "404.html"
            | "404.php"
            | "404.aspx"
            | "not-found"
            | "sayfa-bulunamadi"
            | "sayfa-bulunamadi.html"
            | "bulunamadi"
            | "hata"
            | "hata-sayfasi"
            | "error"
            | "error/404"
    )
}

fn looks_like_soft_404_text(title: &str, h1: &str) -> bool {
    let combined = format!("{} {}", title.to_lowercase(), h1.to_lowercase());
    [
        "404 not found",
        "404 -",
        "sayfa bulunamadı",
        "sayfa bulunamadi",
        "aradığınız sayfa bulunamadı",
        "aradığınız sayfaya ulaşılamıyor",
        "aradığınız ürün bulunamadı",
        "bu ürün satıştan kaldırılmıştır",
        "satıştan kaldırıldı",
        "yayından kaldırıldı",
        "sayfa kaldırılmış",
        "0 ürün bulundu",
        "sonuç bulunamadı",
        "hiç ürün bulunamadı",
        "page not found",
        "page cannot be found",
        "no longer available",
        "content not found",
        "no results found",
        "no products found",
        "ürün bulunamadı",
        "içerik bulunamadı",
        "bu sayfa mevcut değil",
    ]
    .iter()
    .any(|phrase| combined.contains(phrase))
}

/// Suggests a parent category path or root URL for a 301 redirect rule.
pub fn suggest_redirect_target(raw_url: &str) -> String {
    let Ok(mut parsed) = Url::parse(raw_url) else {
        return "/".to_owned();
    };
    parsed.set_query(None);
    parsed.set_fragment(None);
    let segments: Vec<&str> = parsed.path().split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() <= 1 {
        parsed.set_path("/");
    } else {
        let parent = format!("/{}/", segments[..segments.len() - 1].join("/"));
        parsed.set_path(&parent);
    }
    parsed.into()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::*;

    type MockResponse = (u16, Vec<(String, String)>, String);

    struct FakeFetcher {
        responses: HashMap<String, MockResponse>,
    }

    impl Fetcher for FakeFetcher {
        fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
            let Some((status, headers, body)) = self.responses.get(request.url) else {
                return Err(FetchError::Connection("missing test route".to_owned()));
            };
            Ok(FetchResponse {
                status: *status,
                headers: headers.clone(),
                body: body.as_bytes().to_vec(),
                truncated: false,
                http_version: "HTTP/1.1".to_owned(),
                response_time: Duration::from_millis(25),
            })
        }
    }

    #[test]
    fn classifies_404_500_soft404_and_thin_200_pages() {
        let mut responses = HashMap::new();
        responses.insert(
            "https://acme.com/eski-urun".to_owned(),
            (
                404,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><head><title>404</title></head><body>Not found</body></html>".to_owned(),
            ),
        );
        responses.insert(
            "https://acme.com/soft-404".to_owned(),
            (
                200,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><head><title>Aradığınız Sayfa Bulunamadı - Acme</title></head><body><h1>Sayfa Bulunamadı</h1></body></html>".to_owned(),
            ),
        );
        responses.insert(
            "https://acme.com/az-icerik".to_owned(),
            (
                200,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><head><title>Kısa Sayfa</title></head><body><p>Çok az kelime var.</p></body></html>".to_owned(),
            ),
        );
        responses.insert(
            "https://acme.com/sunucu-hatasi".to_owned(),
            (
                503,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><body>Service Unavailable</body></html>".to_owned(),
            ),
        );

        let fetcher = FakeFetcher { responses };
        let config = ScanConfig {
            domain: "acme.com".to_owned(),
            min_word_count: 30,
            ..ScanConfig::default()
        };

        let discovered = vec![
            DiscoveredUrl {
                url: "https://acme.com/eski-urun".to_owned(),
                engines: vec![IndexEngine::Google, IndexEngine::Yandex],
                serp_title: "Eski Ürün".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            DiscoveredUrl {
                url: "https://acme.com/soft-404".to_owned(),
                engines: vec![IndexEngine::Google],
                serp_title: "Soft 404".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            DiscoveredUrl {
                url: "https://acme.com/az-icerik".to_owned(),
                engines: vec![IndexEngine::Yandex],
                serp_title: "Kısa Sayfa".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            DiscoveredUrl {
                url: "https://acme.com/sunucu-hatasi".to_owned(),
                engines: vec![IndexEngine::Bing],
                serp_title: "Sunucu Hatası".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
        ];

        let control = ScanControl::new();
        let mut progress = ScanProgress::default();
        let verified = verify_urls_with_fetcher(
            &discovered,
            &config,
            &fetcher,
            &control,
            &mut progress,
            &mut |_| {},
        );

        assert_eq!(verified.len(), 4);
        assert_eq!(progress.found_404, 1);
        assert_eq!(progress.found_5xx, 1);
        assert_eq!(progress.found_thin_soft404, 2);
    }

    #[test]
    fn detects_redirect_to_404_slug_and_canonical_to_root_as_soft_404() {
        let mut responses = HashMap::new();
        responses.insert(
            "https://acme.com/urunler/silinmis".to_owned(),
            (
                302,
                vec![("location".to_owned(), "https://acme.com/404".to_owned())],
                String::new(),
            ),
        );
        responses.insert(
            "https://acme.com/404".to_owned(),
            (
                200,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><head><title>Acme</title></head><body><p>Ornek metin kelimeleri burada.</p></body></html>".to_owned(),
            ),
        );
        responses.insert(
            "https://acme.com/urunler/eski-kampanya".to_owned(),
            (
                200,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><head><title>Eski Kampanya</title><link rel=\"canonical\" href=\"https://acme.com/\"></head><body><p>Yeterli sayida kelime iceren uzun metin.</p></body></html>".to_owned(),
            ),
        );
        responses.insert(
            "https://acme.com/urunler/meta-yonlendirme".to_owned(),
            (
                200,
                vec![("content-type".to_owned(), "text/html".to_owned())],
                "<html><head><meta http-equiv=\"refresh\" content=\"0; url=https://acme.com/\"></head><body></body></html>".to_owned(),
            ),
        );

        let fetcher = FakeFetcher { responses };
        let config = ScanConfig {
            domain: "acme.com".to_owned(),
            min_word_count: 3,
            ..ScanConfig::default()
        };

        let r1 = verify_one(
            &DiscoveredUrl {
                url: "https://acme.com/urunler/silinmis".to_owned(),
                engines: vec![IndexEngine::Google],
                serp_title: String::new(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            &config,
            &fetcher,
        );
        assert_eq!(r1.category, FindingCategory::Soft404);
        assert_eq!(r1.reason, "redirect_to_404_slug");

        let r2 = verify_one(
            &DiscoveredUrl {
                url: "https://acme.com/urunler/eski-kampanya".to_owned(),
                engines: vec![IndexEngine::Yandex],
                serp_title: String::new(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            &config,
            &fetcher,
        );
        assert_eq!(r2.category, FindingCategory::Soft404);
        assert_eq!(r2.reason, "canonical_to_root");

        let r3 = verify_one(
            &DiscoveredUrl {
                url: "https://acme.com/urunler/meta-yonlendirme".to_owned(),
                engines: vec![IndexEngine::Bing],
                serp_title: String::new(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            &config,
            &fetcher,
        );
        assert_eq!(r3.category, FindingCategory::Soft404);
        assert_eq!(r3.reason, "meta_refresh_soft_404");
        assert!(is_external_canonical(
            "https://acme.com/urun/1",
            "https://other.org/urun/1"
        ));
        assert!(!is_external_canonical(
            "https://www.acme.com/urun/1",
            "https://acme.com/urun/1"
        ));
        assert!(is_redirect_to_error_slug(
            "https://acme.com/urun/eski",
            "https://acme.com/index.php?route=error/not_found"
        ));
        assert!(looks_like_soft_404_text("Arama", "0 Ürün Bulundu"));
    }

    #[test]
    fn detects_cyclic_redirect_loop_as_error_4xx() {
        let mut responses = HashMap::new();
        responses.insert(
            "https://acme.com/dongu-a".to_owned(),
            (
                301,
                vec![("location".to_owned(), "https://acme.com/dongu-b".to_owned())],
                String::new(),
            ),
        );
        responses.insert(
            "https://acme.com/dongu-b".to_owned(),
            (
                302,
                vec![("location".to_owned(), "https://acme.com/dongu-a".to_owned())],
                String::new(),
            ),
        );
        let fetcher = FakeFetcher { responses };
        let config = ScanConfig {
            domain: "acme.com".to_owned(),
            ..ScanConfig::default()
        };
        let row = verify_one(
            &DiscoveredUrl {
                url: "https://acme.com/dongu-a".to_owned(),
                engines: vec![IndexEngine::Google],
                serp_title: "Döngü Sayfası".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
            },
            &config,
            &fetcher,
        );
        assert_eq!(row.category, FindingCategory::Error4xx);
        assert_eq!(row.reason, "redirect_loop");
        assert_eq!(row.redirect_hops, 1);
    }
}
