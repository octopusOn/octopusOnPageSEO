use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Write as _;

use super::*;
use crate::SitemapEntry;
use crate::config::CrawlConfig;
use crate::fetch::FetchError;

mod exclusions;

#[test]
fn unusual_http_responses_are_neither_5xx_errors_nor_missing_responses() {
    let mut record = UrlRecord::new(0, "https://example.com/".into(), Some(0), true);
    for (status, expected) in [
        (199, Indexability::OtherHttp),
        (200, Indexability::Indexable),
        (299, Indexability::Indexable),
        (300, Indexability::OtherHttp),
        (301, Indexability::Redirected),
        (304, Indexability::OtherHttp),
        (308, Indexability::Redirected),
        (399, Indexability::OtherHttp),
        (400, Indexability::ClientError),
        (499, Indexability::ClientError),
        (500, Indexability::ServerError),
        (599, Indexability::ServerError),
        (600, Indexability::OtherHttp),
        (999, Indexability::OtherHttp),
    ] {
        record.status = Some(status);
        assert_eq!(indexability(&record), expected, "HTTP {status}");
        assert!(!expected.is_indexable() || (200..=299).contains(&status));
    }
    record.status = None;
    assert_eq!(indexability(&record), Indexability::NoResponse);
    record.status = Some(600);
    record.blocked_by_robots = true;
    assert_eq!(indexability(&record), Indexability::BlockedByRobots);
    assert_eq!(Indexability::OtherHttp.as_str(), "other-http");
}

#[test]
fn only_automatic_navigation_statuses_create_and_follow_location_edges() {
    for status in 300..=399 {
        let navigates = matches!(status, 301 | 302 | 303 | 307 | 308);
        let site = FakeSite::default()
            .respond(
                "https://example.com/",
                status,
                &[("Content-Type", "text/html"), ("Location", "/target")],
                "<title>Not a document</title><a href='/body-link'>Ignored</a>",
            )
            .html("https://example.com/target", "<title>Destination</title>");
        let (collected, summary) = crawl(&site, |config| {
            config.mode = crate::CrawlMode::List;
            config.robots = RobotsPolicy::Ignore;
            config.follow.sitemaps = false;
        });
        let source = collected.record("https://example.com/");
        assert_eq!(source.status, Some(status));
        assert_eq!(
            source.redirect_url.as_deref(),
            navigates.then_some("https://example.com/target"),
            "HTTP {status}"
        );
        assert_eq!(
            source.indexability,
            if navigates {
                Indexability::Redirected
            } else {
                Indexability::OtherHttp
            }
        );
        assert!(source.page.is_none() && source.rendering.is_none());
        assert_eq!(
            site.requested("https://example.com/target"),
            usize::from(navigates)
        );
        assert_eq!(site.requested("https://example.com/body-link"), 0);
        assert_eq!(collected.links.len(), usize::from(navigates));
        assert_eq!(summary.progress.crawled, if navigates { 2 } else { 1 });
        assert_eq!(summary.progress.errors, 0);
    }
}

#[test]
fn empty_or_invalid_page_locations_do_not_invent_self_redirects() {
    for location in [
        None,
        Some(""),
        Some(" \t "),
        Some("https://[invalid/target"),
    ] {
        let headers = location.map(|value| ("Location", value));
        let site = FakeSite::default().respond("https://example.com/", 302, headers.as_slice(), "");
        let (collected, summary) = crawl(&site, |config| {
            config.mode = crate::CrawlMode::List;
            config.robots = RobotsPolicy::Ignore;
        });
        let source = collected.record("https://example.com/");
        assert_eq!(source.status, Some(302));
        assert!(source.redirect_url.is_none(), "Location {location:?}");
        assert_eq!(collected.links, [] as [LinkRecord; 0]);
        assert_eq!(summary.progress.crawled, 1);
        assert_eq!(site.requested("https://example.com/"), 1);
    }
}

/// An in-memory site; unknown URLs answer 404.
#[derive(Default)]
struct FakeSite {
    pages: HashMap<String, FetchResponse>,
    requests: Mutex<Vec<String>>,
}

impl FakeSite {
    fn html(mut self, url: &str, body: &str) -> Self {
        let response =
            FetchResponse::new(200, &[("Content-Type", "text/html; charset=utf-8")], body);
        self.pages.insert(url.to_owned(), response);
        self
    }

    fn respond(mut self, url: &str, status: u16, headers: &[(&str, &str)], body: &str) -> Self {
        self.pages
            .insert(url.to_owned(), FetchResponse::new(status, headers, body));
        self
    }

    fn requested(&self, url: &str) -> usize {
        let requests = self.requests.lock().unwrap();
        requests.iter().filter(|r| *r == url).count()
    }
}

impl Fetcher for FakeSite {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.requests.lock().unwrap().push(request.url.to_owned());
        if request.url.starts_with("https://down.example/") {
            return Err(FetchError::Dns);
        }
        let mut response = self.pages.get(request.url).cloned().unwrap_or_else(|| {
            FetchResponse::new(404, &[("Content-Type", "text/html")], "missing")
        });
        if request.body_limit == 0 {
            response.body.clear();
        }
        Ok(response)
    }
}

#[derive(Default)]
struct Collected {
    urls: Vec<UrlRecord>,
    links: Vec<LinkRecord>,
    robots: Vec<RobotsRecord>,
    sitemaps: Vec<SitemapRecord>,
    site_files: Vec<SiteFileRecord>,
    activity: Vec<CrawlActivity>,
    progress: Vec<CrawlProgress>,
}

impl Collected {
    fn record(&self, url: &str) -> &UrlRecord {
        self.urls
            .iter()
            .find(|r| r.url == url)
            .unwrap_or_else(|| panic!("{url} not crawled"))
    }

    fn crawled(&self, url: &str) -> bool {
        self.urls.iter().any(|r| r.url == url)
    }
}

impl CrawlSink for Collected {
    type Error = Infallible;

    fn url(&mut self, record: &UrlRecord, links: &[LinkRecord]) -> Result<(), Infallible> {
        self.urls.push(record.clone());
        self.links.extend_from_slice(links);
        Ok(())
    }

    fn robots(&mut self, record: &RobotsRecord) -> Result<(), Infallible> {
        self.robots.push(record.clone());
        Ok(())
    }

    fn sitemap(&mut self, record: &SitemapRecord) -> Result<(), Infallible> {
        self.sitemaps.push(record.clone());
        Ok(())
    }

    fn site_file(&mut self, record: &SiteFileRecord) -> Result<(), Infallible> {
        self.site_files.push(record.clone());
        Ok(())
    }

    fn activity(&mut self, activity: &CrawlActivity) {
        self.activity.push(activity.clone());
    }

    fn progress(&mut self, progress: &CrawlProgress) {
        assert_eq!(
            self.activity.last().unwrap().active.len(),
            progress.in_flight
        );
        self.progress.push(*progress);
    }
}

#[test]
fn reports_active_urls_and_completed_outcomes_before_storage_is_closed() {
    let (crawl, _) = crawl(&site(), |_| {});
    let first = &crawl.activity[0];
    assert_ne!(first.active, Vec::<UrlActivity>::new());
    assert_eq!(first.recent, Vec::<UrlActivity>::new());
    assert!(
        first
            .active
            .iter()
            .all(|entry| entry.state == ActivityState::Processing)
    );
    let last = crawl.activity.last().unwrap();
    assert_eq!(last.active, Vec::<UrlActivity>::new());
    assert_eq!(last.recent.len(), crawl.urls.len() + 3);
    let find = |url: &str| last.recent.iter().find(|entry| entry.url == url).unwrap();
    assert_eq!(find("https://example.com/").title.as_deref(), Some("Home"));
    assert_eq!(find("https://example.com/c").state, ActivityState::Error);
    assert_eq!(find("https://example.com/a").status, Some(301));
    assert_eq!(
        find("https://example.com/private/x").state,
        ActivityState::Blocked
    );
    assert_eq!(find("https://down.example/").error.as_deref(), Some("dns"));
    assert_eq!(
        find("https://example.com/logo.png").kind,
        ActivityKind::Resource
    );
    assert_eq!(
        find("https://example.com/robots.txt").kind,
        ActivityKind::Robots
    );
    assert_eq!(last.blocked, 1);
    assert_eq!(last.request_errors, 3);
}

fn site() -> FakeSite {
    FakeSite::default()
        .respond(
            "https://example.com/robots.txt",
            200,
            &[],
            "User-agent: *\nDisallow: /private/\nSitemap: https://example.com/sitemap.xml\n",
        )
        .html(
            "https://example.com/",
            r#"<title>Home</title><link rel="canonical" href="https://example.com/">
            <a href="/a">A</a> <a href="/b" rel="nofollow">B</a> <a href="/private/x">P</a>
            <a href="https://other.org/">Other</a> <a href="https://down.example/">Down</a>
            <img src="/logo.png" alt="Logo">"#,
        )
        .respond("https://example.com/a", 301, &[("Location", "/a/")], "")
        .html(
            "https://example.com/a/",
            r#"<meta name="robots" content="noindex"><a href="/c#frag">C</a>"#,
        )
        .respond(
            "https://example.com/logo.png",
            200,
            &[("Content-Type", "image/png")],
            "PNG",
        )
        .html(
            "https://other.org/",
            "<title>Other</title><a href='/deep'>deep</a>",
        )
        .respond(
            "https://example.com/sitemap.xml",
            200,
            &[("Content-Type", "application/xml")],
            "<urlset><url><loc>https://example.com/</loc></url>\
             <url><loc>https://example.com/orphan</loc></url></urlset>",
        )
        .html("https://example.com/orphan", "<title>Orphan</title>")
}

fn crawl(site: &FakeSite, edit: impl FnOnce(&mut CrawlConfig)) -> (Collected, CrawlSummary) {
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    };
    config.speed.threads = 3;
    edit(&mut config);
    let plan = CrawlPlan::new(config).unwrap();
    let mut sink = Collected::default();
    let summary = Crawler::new(&plan, site)
        .run(&mut sink, &CrawlControl::new())
        .unwrap();
    (sink, summary)
}

#[test]
fn crawls_a_site_like_a_search_engine() {
    let site = site();
    let (crawl, summary) = crawl(&site, |_| {});

    let home = crawl.record("https://example.com/");
    assert_eq!((home.id, home.depth), (0, Some(0)));
    assert_eq!(home.indexability, Indexability::Indexable);
    assert_eq!(home.page.as_ref().and_then(|p| p.title()), Some("Home"));

    let redirect = crawl.record("https://example.com/a");
    assert_eq!(redirect.status, Some(301));
    assert_eq!(
        redirect.redirect_url.as_deref(),
        Some("https://example.com/a/")
    );
    assert_eq!(redirect.indexability, Indexability::Redirected);
    // A redirect hop does not add a click.
    assert_eq!(crawl.record("https://example.com/a/").depth, Some(1));
    assert_eq!(
        crawl.record("https://example.com/a/").indexability,
        Indexability::Noindex
    );

    // Fragments are stripped before crawling.
    let missing = crawl.record("https://example.com/c");
    assert_eq!((missing.status, missing.depth), (Some(404), Some(2)));
    assert_eq!(missing.indexability, Indexability::ClientError);

    let blocked = crawl.record("https://example.com/private/x");
    assert!(blocked.blocked_by_robots && blocked.status.is_none());
    assert_eq!(blocked.robots_rule.as_deref(), Some("Disallow: /private/"));
    assert_eq!(site.requested("https://example.com/private/x"), 0);

    assert!(
        !crawl.crawled("https://example.com/b"),
        "nofollow links are not followed"
    );
    let nofollow = crawl
        .links
        .iter()
        .find(|l| l.target_url == "https://example.com/b")
        .unwrap();
    assert!(nofollow.nofollow && nofollow.internal);

    let external = crawl.record("https://other.org/");
    assert!(!external.internal && external.page.is_none());
    assert!(
        !crawl.crawled("https://other.org/deep"),
        "external pages are not crawled"
    );
    assert_eq!(
        crawl.record("https://down.example/").error.as_deref(),
        Some("dns")
    );

    let logo = crawl.record("https://example.com/logo.png");
    assert_eq!(logo.content_kind, ContentKind::Image);
    let image_link = crawl
        .links
        .iter()
        .find(|l| l.kind == CrawlLinkKind::Image)
        .unwrap();
    assert_eq!(image_link.text, "Logo");

    // Only sitemaps reach the orphan; it is crawled last, without a depth.
    let orphan = crawl.record("https://example.com/orphan");
    assert_eq!(orphan.depth, None);
    assert_eq!(
        crawl.urls.last().map(|r| r.url.as_str()),
        Some("https://example.com/orphan")
    );

    assert_eq!(crawl.robots.len(), 1);
    assert_eq!(crawl.site_files.len(), 1);
    assert_eq!(crawl.site_files[0].url, "https://example.com/llms.txt");
    assert_eq!(crawl.site_files[0].status, Some(404));
    assert_eq!(site.requested("https://example.com/robots.txt"), 1);
    assert_eq!(crawl.sitemaps.len(), 1);
    assert_eq!(crawl.sitemaps[0].entries.len(), 2);
    assert_eq!(summary.progress.crawled, 9);
    assert!(!summary.stopped && !summary.limit_reached);
}

#[test]
fn honors_limits_and_settings() {
    let site = site();
    let (limited, summary) = crawl(&site, |c| {
        c.limits.max_urls = 2;
        c.speed.threads = 1;
    });
    assert_eq!(limited.urls.len(), 2);
    assert!(summary.limit_reached);

    let (shallow, _) = crawl(&site, |c| c.limits.max_depth = Some(0));
    assert!(!shallow.crawled("https://example.com/a"));
    assert!(
        shallow.crawled("https://example.com/logo.png"),
        "resources of depth-0 pages are checked"
    );

    let (ignoring, _) = crawl(&site, |c| {
        c.robots = RobotsPolicy::IgnoreButReport;
        c.follow.internal_nofollow = true;
        c.follow.check_external = false;
        c.follow.sitemaps = false;
    });
    let private = ignoring.record("https://example.com/private/x");
    assert!(private.blocked_by_robots && private.status == Some(404));
    assert!(ignoring.crawled("https://example.com/b"));
    assert!(!ignoring.crawled("https://other.org/"));
    assert!(!ignoring.crawled("https://example.com/orphan"));
}

#[test]
fn list_mode_fetches_only_the_list_and_redirects() {
    let site = site();
    let (list, _) = crawl(&site, |c| {
        c.mode = CrawlMode::List;
        c.start_urls = vec![
            "https://example.com/a".into(),
            "https://example.com/orphan".into(),
        ];
    });
    let crawled: Vec<_> = list.urls.iter().map(|r| r.url.as_str()).collect();
    assert_eq!(crawled.len(), 3);
    assert!(list.crawled("https://example.com/a/"));
    assert!(!list.crawled("https://example.com/c"));
    assert_eq!(list.sitemaps.len(), 0);
}

#[test]
fn stop_ends_the_crawl() {
    let site = site();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    };
    config.speed.threads = 1;
    let plan = CrawlPlan::new(config).unwrap();
    let control = CrawlControl::new();
    control.stop();
    let mut sink = Collected::default();
    let summary = Crawler::new(&plan, &site).run(&mut sink, &control).unwrap();
    assert!(summary.stopped);
    assert_eq!(sink.urls.len(), 0);
}

fn stop_during_rate_wait(job: Job) {
    let site = FakeSite::default();
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    })
    .unwrap();
    let shared = Shared {
        robots: RobotsCache::new(&[]),
        limiter: RateLimiter::new(1.0),
    };
    let control = CrawlControl::new();
    assert!(
        shared
            .limiter
            .wait_until(Instant::now() + Duration::from_secs(1), &control)
    );
    let started = Instant::now();
    let outcome = thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(20));
            control.stop();
        });
        Crawler::new(&plan, &site).work(&shared, job, &control)
    });
    assert!(
        matches!(outcome, Outcome::Skipped(_)),
        "unsent work must be cancelled, not fetched"
    );
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(site.requests.lock().unwrap().is_empty());
    let mut activity = ActivityTracker::default();
    activity.start(0, "https://example.com/".into(), ActivityKind::Page, 0);
    activity.finish(0, 50, |entry| outcome.describe_activity(entry));
    let final_activity = activity.snapshot(50);
    assert_eq!(final_activity.recent[0].state, ActivityState::Cancelled);
    assert_eq!(final_activity.request_errors, 0);
}

#[test]
fn stop_cancels_a_page_waiting_for_its_http_rate_slot() {
    stop_during_rate_wait(Job::Fetch(Task {
        id: 0,
        url: Url::parse("https://example.com/").unwrap(),
        depth: Some(0),
        kind: TaskKind::Page,
        internal: true,
    }));
}

#[test]
fn stop_cancels_a_sitemap_waiting_for_its_http_rate_slot() {
    stop_during_rate_wait(Job::Sitemap(
        Url::parse("https://example.com/sitemap.xml").unwrap(),
    ));
}

#[test]
fn a_truncated_sitemap_does_not_publish_partial_discovery() {
    let mut site = site();
    site.pages
        .get_mut("https://example.com/sitemap.xml")
        .unwrap()
        .truncated = true;
    let (crawl, _) = crawl(&site, |_| {});
    assert!(!crawl.crawled("https://example.com/orphan"));
    let sitemap = &crawl.sitemaps[0];
    assert_eq!(sitemap.status, Some(200));
    assert!(
        sitemap
            .error
            .as_deref()
            .is_some_and(|error| error.contains("truncated"))
    );
    assert_eq!(sitemap.entries, Vec::<SitemapEntry>::new());
}

#[test]
fn prefixed_sitemap_indexes_discover_only_complete_core_entries() {
    let site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond("https://example.com/robots.txt", 200, &[], "User-agent: *\nSitemap: https://example.com/index.xml\n")
        .respond("https://example.com/index.xml", 200, &[], r#"<s:sitemapindex xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9">
          <!-- <s:sitemap><s:loc>https://example.com/fake.xml</s:loc></s:sitemap> -->
          <s:sitemap><s:loc>https://example.com/pages.xml</s:loc></s:sitemap>
          <s:sitemap><s:loc>https://example.com/incomplete.xml</s:loc></s:sitemap>
        </s:sitemapindex>"#)
        .respond("https://example.com/pages.xml", 200, &[], r#"<s:urlset xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:img="urn:images">
          <s:url><img:loc>https://example.com/phantom</img:loc><s:loc>https://example.com/orphan?x=1&#38;y=2</s:loc><s:lastmod>2026-10-17</s:lastmod></s:url>
          <img:url><s:loc>https://example.com/extension</s:loc></img:url>
        </s:urlset>"#)
        .respond("https://example.com/incomplete.xml", 200, &[], "<urlset><url><loc>https://example.com/incomplete</loc></url>")
        .html("https://example.com/orphan?x=1&y=2", "<title>Discovered orphan</title>");
    let (crawl, summary) = crawl(&site, |_| {});
    assert_eq!(summary.progress.crawled, 2);
    assert_eq!(summary.progress.errors, 0);
    assert_eq!(summary.progress.sitemaps, 3);
    assert_eq!(
        crawl.record("https://example.com/orphan?x=1&y=2").depth,
        None
    );
    for url in ["fake.xml", "phantom", "extension", "incomplete"] {
        assert_eq!(site.requested(&format!("https://example.com/{url}")), 0);
    }
    let pages = crawl
        .sitemaps
        .iter()
        .find(|map| map.url.ends_with("/pages.xml"))
        .unwrap();
    assert_eq!(pages.entries.len(), 1);
    assert_eq!(pages.entries[0].lastmod.as_deref(), Some("2026-10-17"));
    let incomplete = crawl
        .sitemaps
        .iter()
        .find(|map| map.url.ends_with("/incomplete.xml"))
        .unwrap();
    assert!(incomplete.error.is_some());
    assert_eq!(incomplete.entries, Vec::<SitemapEntry>::new());
}

#[test]
fn sitemap_fragments_do_not_consume_the_budget_or_merge_distinct_queries() {
    let site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond(
            "https://example.com/robots.txt",
            200,
            &[],
            "User-agent: *\nSitemap: https://example.com/index.xml#robots\n",
        )
        .respond(
            "https://example.com/index.xml",
            200,
            &[],
            "<sitemapindex>\
          <sitemap><loc>https://example.com/index.xml#cycle</loc></sitemap>\
          <sitemap><loc>https://example.com/pages.xml#one</loc></sitemap>\
          <sitemap><loc>https://example.com/pages.xml#two</loc></sitemap>\
          <sitemap><loc>https://example.com/pages.xml?part=2#two</loc></sitemap>\
          </sitemapindex>",
        )
        .respond(
            "https://example.com/pages.xml",
            200,
            &[],
            "<urlset><url><loc>https://example.com/one</loc></url></urlset>",
        )
        .respond(
            "https://example.com/pages.xml?part=2",
            200,
            &[],
            "<urlset><url><loc>https://example.com/two</loc></url></urlset>",
        )
        .html("https://example.com/one", "<title>One</title>")
        .html("https://example.com/two", "<title>Two</title>");
    let (crawl, summary) = crawl(&site, |config| {
        config.follow.sitemap_urls = vec![
            "https://example.com/index.xml#config-a".into(),
            "https://example.com/index.xml#config-b".into(),
        ];
        config.limits.max_sitemaps = 3;
    });
    assert_eq!(summary.progress.sitemaps, 3);
    assert_eq!(summary.progress.crawled, 3);
    assert_eq!(summary.progress.errors, 0);
    for url in [
        "https://example.com/index.xml",
        "https://example.com/pages.xml",
        "https://example.com/pages.xml?part=2",
    ] {
        assert_eq!(site.requested(url), 1, "{url}");
    }
    assert!(
        crawl
            .sitemaps
            .iter()
            .all(|map| !map.url.contains('#') && map.error.is_none())
    );
    assert!(
        site.requests
            .lock()
            .unwrap()
            .iter()
            .all(|url| !url.contains('#'))
    );
    assert_eq!(crawl.record("https://example.com/one").depth, None);
    assert_eq!(crawl.record("https://example.com/two").depth, None);
    assert_eq!(crawl.robots[0].sitemaps, ["https://example.com/index.xml"]);
}

#[test]
fn unsupported_sitemap_schemes_do_not_consume_the_http_map_budget() {
    let site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond("https://example.com/robots.txt", 200, &[], "User-agent: *\nSitemap: ftp://example.com/map.xml\nSitemap: https://example.com/index.xml\n")
        .respond("https://example.com/index.xml", 200, &[], "<sitemapindex>\
          <sitemap><loc>data:text/plain,unused</loc></sitemap>\
          <sitemap><loc>file:///tmp/unused.xml</loc></sitemap>\
          <sitemap><loc>https://example.com/pages.xml</loc></sitemap></sitemapindex>")
        .respond("https://example.com/pages.xml", 200, &[], "<urlset><url><loc>https://example.com/orphan</loc></url></urlset>")
        .html("https://example.com/orphan", "<title>Orphan</title>");
    let (crawl, summary) = crawl(&site, |config| {
        config.follow.sitemap_urls = vec!["mailto:unused@example.com".into()];
        config.limits.max_sitemaps = 2;
    });
    assert_eq!(summary.progress.sitemaps, 2);
    assert!(crawl.crawled("https://example.com/orphan"));
    assert!(crawl.sitemaps.iter().all(|map| map.error.is_none()));
    assert!(
        site.requests
            .lock()
            .unwrap()
            .iter()
            .all(|url| url.starts_with("https://"))
    );
}

#[test]
fn unsupported_sitemap_hints_do_not_disable_the_default_http_discovery() {
    let site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond(
            "https://example.com/robots.txt",
            200,
            &[],
            "User-agent: *\nSitemap: ftp://example.com/map.xml\n",
        )
        .respond(
            "https://example.com/sitemap.xml",
            200,
            &[],
            "<urlset><url><loc>https://example.com/orphan</loc></url></urlset>",
        )
        .html("https://example.com/orphan", "<title>Orphan</title>");
    let (crawl, summary) = crawl(&site, |config| {
        config.follow.sitemap_urls = vec!["data:text/plain,unused".into()];
        config.limits.max_sitemaps = 1;
    });
    assert_eq!(summary.progress.sitemaps, 1);
    assert!(crawl.crawled("https://example.com/orphan"));
    assert_eq!(site.requested("https://example.com/sitemap.xml"), 1);
    assert!(crawl.sitemaps.iter().all(|map| map.error.is_none()));
}

#[test]
fn stop_cancels_a_site_file_waiting_for_its_http_rate_slot() {
    stop_during_rate_wait(Job::SiteFile(
        Url::parse("https://example.com/llms.txt").unwrap(),
    ));
}

#[test]
fn sitemap_redirect_hops_preserve_statuses_queries_and_cross_origin_discovery() {
    let maps = [
        (
            "https://example.com/sitemap.xml",
            301,
            "/hop-302.xml?token=a%23b#first",
        ),
        (
            "https://example.com/hop-302.xml?token=a%23b",
            302,
            "//maps.example.com/hop-303.xml?part=2#second",
        ),
        (
            "https://maps.example.com/hop-303.xml?part=2",
            303,
            "next/hop-307.xml?keep=3#third",
        ),
        (
            "https://maps.example.com/next/hop-307.xml?keep=3",
            307,
            "/hop-308.xml",
        ),
        (
            "https://maps.example.com/hop-308.xml",
            308,
            "/final.xml?version=2#last",
        ),
    ];
    let mut site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond(
            "https://example.com/robots.txt",
            200,
            &[],
            "User-agent: *\n",
        )
        .respond(
            "https://maps.example.com/final.xml?version=2",
            200,
            &[],
            "<urlset><url><loc>https://example.com/orphan?utm_source=map</loc></url></urlset>",
        )
        .html(
            "https://example.com/orphan",
            "<title>Found after redirects</title>",
        );
    for (url, status, location) in maps {
        site = site.respond(
            url,
            status,
            &[("Location", location)],
            "ignored redirect body",
        );
    }
    let (crawl, summary) = crawl(&site, |config| {
        config.urls.strip_all_params = true;
        config.limits.max_sitemaps = 6;
    });
    assert_eq!(summary.progress.sitemaps, 6);
    assert_eq!(summary.progress.crawled, 2);
    assert_eq!(summary.progress.errors, 0);
    for (url, status, _) in maps {
        let row = crawl.sitemaps.iter().find(|row| row.url == url).unwrap();
        assert_eq!(row.status, Some(status));
        assert!(row.error.is_none());
        assert!(row.kind.is_none());
        assert_eq!(row.entries, Vec::<SitemapEntry>::new());
        assert_eq!(site.requested(url), 1);
    }
    assert_eq!(
        site.requested("https://maps.example.com/final.xml?version=2"),
        1
    );
    assert_eq!(crawl.record("https://example.com/orphan").depth, None);
}

#[test]
fn sitemap_redirect_cycles_and_convergence_share_the_unique_map_budget() {
    let site=FakeSite::default().html("https://example.com/","<title>Home</title>")
        .respond("https://example.com/robots.txt",200,&[],"User-agent: *\n")
        .respond("https://example.com/sitemap.xml",200,&[],"<sitemapindex><sitemap><loc>https://example.com/a.xml</loc></sitemap><sitemap><loc>https://example.com/b.xml</loc></sitemap><sitemap><loc>https://example.com/one.xml</loc></sitemap><sitemap><loc>https://example.com/two.xml</loc></sitemap></sitemapindex>")
        .respond("https://example.com/a.xml",301,&[("Location","/b.xml#cycle")],"")
        .respond("https://example.com/b.xml",302,&[("Location","/a.xml#cycle")],"")
        .respond("https://example.com/one.xml",307,&[("Location","/shared.xml#one")],"")
        .respond("https://example.com/two.xml",308,&[("Location","/shared.xml#two")],"")
        .respond("https://example.com/shared.xml",200,&[],"<urlset><url><loc>https://example.com/orphan</loc></url></urlset>")
        .html("https://example.com/orphan","<title>Shared target</title>");
    let (crawl, summary) = crawl(&site, |config| config.limits.max_sitemaps = 6);
    assert_eq!(summary.progress.sitemaps, 6);
    assert_eq!(summary.progress.crawled, 2);
    assert_eq!(summary.progress.errors, 0);
    for name in ["a.xml", "b.xml", "shared.xml"] {
        assert_eq!(site.requested(&format!("https://example.com/{name}")), 1);
    }
    assert_eq!(crawl.record("https://example.com/orphan").depth, None);
}

#[test]
fn sitemap_redirect_targets_respect_the_map_limit_without_inventing_http_failures() {
    let site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond(
            "https://example.com/robots.txt",
            200,
            &[],
            "User-agent: *\n",
        )
        .respond(
            "https://example.com/sitemap.xml",
            301,
            &[("Location", "/middle.xml")],
            "",
        )
        .respond(
            "https://example.com/middle.xml",
            302,
            &[("Location", "/unfetched.xml")],
            "",
        );
    let (crawl, summary) = crawl(&site, |config| config.limits.max_sitemaps = 2);
    assert_eq!(summary.progress.sitemaps, 2);
    assert_eq!(summary.progress.errors, 0);
    assert_eq!(site.requested("https://example.com/unfetched.xml"), 0);
    assert!(crawl.sitemaps.iter().all(|row| row.error.is_none()));
}

#[test]
fn non_http_sitemap_entries_cannot_spend_the_page_budget() {
    let site = FakeSite::default()
        .html("https://example.com/", "<title>Home</title>")
        .respond(
            "https://example.com/robots.txt",
            200,
            &[],
            "User-agent: *\n",
        )
        .respond(
            "https://example.com/sitemap.xml",
            200,
            &[],
            "<urlset>\
          <url><loc>ftp://example.com/file</loc></url>\
          <url><loc>ws://example.com/socket</loc></url>\
          <url><loc>wss://example.com/socket</loc></url>\
          <url><loc>file://example.com/file</loc></url>\
          <url><loc>custom://example.com/file</loc></url>\
          <url><loc>mailto:hello@example.com</loc></url>\
          <url><loc>https://example.com/one</loc></url>\
          <url><loc>https://example.com/two</loc></url></urlset>",
        )
        .html("https://example.com/one", "<title>One</title>")
        .html("https://example.com/two", "<title>Two</title>");
    let (crawl, summary) = crawl(&site, |config| {
        config.limits.max_urls = 3;
    });
    assert_eq!(summary.progress.crawled, 3);
    assert_eq!(summary.progress.errors, 0);
    assert_eq!(crawl.record("https://example.com/one").depth, None);
    assert_eq!(crawl.record("https://example.com/two").depth, None);
    assert_eq!(crawl.sitemaps[0].entries.len(), 8);
    assert!(
        site.requests
            .lock()
            .unwrap()
            .iter()
            .all(|raw| raw.starts_with("https://"))
    );
}

#[test]
fn non_http_redirects_remain_evidence_without_requests_or_budget_loss() {
    let targets = [
        "ftp://example.com/from-redirect",
        "ws://example.com/from-redirect",
        "wss://other.example/from-redirect",
        "file://example.com/from-redirect",
        "custom://example.com/from-redirect",
        "mailto:hello@example.com",
        "data:text/plain,redirect",
    ];
    let page_count = u32::try_from(targets.len()).unwrap() + 3;
    for check_external in [false, true] {
        let mut links = String::new();
        for i in 0..targets.len() {
            write!(&mut links, "<a href='/r/{i}'>Redirect</a>").unwrap();
        }
        let mut site = FakeSite::default()
            .html(
                "https://example.com/",
                &format!("{links}<a href='/http'>HTTP</a>"),
            )
            .respond(
                "https://example.com/http",
                307,
                &[("Location", "/final?version=2#section")],
                "",
            )
            .html(
                "https://example.com/final?version=2",
                "<title>Final</title>",
            );
        for (i, target) in targets.iter().enumerate() {
            site = site.respond(
                &format!("https://example.com/r/{i}"),
                302,
                &[("Location", target)],
                "",
            );
        }
        let (crawl, summary) = crawl(&site, |config| {
            config.robots = RobotsPolicy::Ignore;
            config.follow.sitemaps = false;
            config.follow.check_external = check_external;
            config.limits.max_urls = page_count;
            config.storage.headers = true;
        });
        assert_eq!(summary.progress.crawled, page_count);
        assert_eq!(summary.progress.errors, 0);
        assert_eq!(
            crawl.record("https://example.com/final?version=2").depth,
            Some(1)
        );
        let http = crawl.record("https://example.com/http");
        assert_eq!(
            http.redirect_url.as_deref(),
            Some("https://example.com/final?version=2")
        );
        assert!(
            http.headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("location")
                    && value == "/final?version=2#section")
        );
        for (i, target) in targets.iter().enumerate() {
            let record = crawl.record(&format!("https://example.com/r/{i}"));
            assert_eq!(record.redirect_url.as_deref(), Some(*target));
            assert_eq!(site.requested(target), 0);
            let link = crawl
                .links
                .iter()
                .find(|link| link.target_url == *target)
                .unwrap();
            assert!(!link.internal);
        }
        assert!(
            site.requests
                .lock()
                .unwrap()
                .iter()
                .all(|raw| raw.starts_with("https://"))
        );
    }
}

#[test]
fn cancelling_an_unsent_page_preserves_its_completed_robots_response() {
    let site = FakeSite::default().respond(
        "https://example.com/robots.txt",
        200,
        &[("Content-Type", "text/plain")],
        "User-agent: *\nAllow: /\n",
    );
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let shared = Shared {
        robots: RobotsCache::new(plan.robots_tokens()),
        limiter: RateLimiter::new(1.0),
    };
    let control = CrawlControl::new();
    assert!(
        shared
            .limiter
            .wait_until(Instant::now() + Duration::from_secs(1), &control)
    );
    let outcome = thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(20));
            control.stop();
        });
        Crawler::new(&plan, &site).work(
            &shared,
            Job::Fetch(Task {
                id: 0,
                url: Url::parse("https://example.com/").unwrap(),
                depth: Some(0),
                kind: TaskKind::Page,
                internal: true,
            }),
            &control,
        )
    });
    assert!(matches!(&outcome,Outcome::Skipped(Some(record)) if record.http_status==Some(200)));
    let mut sink = Collected::default();
    let mut coordinator = Coordinator::new(&plan, 1);
    coordinator.handle(outcome, &mut sink).unwrap();
    assert_eq!(sink.robots.len(), 1);
    assert_eq!(site.requested("https://example.com/robots.txt"), 1);
    assert_eq!(site.requested("https://example.com/"), 0);
    assert_eq!(sink.urls, Vec::<UrlRecord>::new());
    assert_eq!(coordinator.progress.crawled, 0);
    assert_eq!(coordinator.progress.errors, 0);
}
