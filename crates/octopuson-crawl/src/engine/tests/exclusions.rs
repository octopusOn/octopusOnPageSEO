use super::*;

#[test]
fn editable_system_exclusions_preserve_links_and_can_be_removed() {
    let site = FakeSite::default().html("https://example.com/", "<title>Home</title><a href='/cdn-cgi/l/email-protection#a'>Email A</a><a href='/cdn-cgi/l/email-protection#b'>Email B</a><a href='/cdn-cgi/challenge-platform/h/g/'>Bot</a><a href='/cdn-cgi/trace'>Trace</a><a href='/missing'>Broken</a><img src='/cdn-cgi/image/width=800/photo.jpg'>");
    let (collected, summary) = crawl(&site, |config| {
        config.robots = RobotsPolicy::Ignore;
        config.follow.sitemaps = false;
    });
    assert_eq!(collected.links.len(), 6);
    assert_eq!(summary.progress.crawled, 3);
    assert_eq!(summary.progress.errors, 2);
    for path in [
        "/cdn-cgi/l/email-protection",
        "/cdn-cgi/challenge-platform/h/g/",
        "/cdn-cgi/trace",
    ] {
        assert_eq!(site.requested(&format!("https://example.com{path}")), 0);
    }
    assert_eq!(
        site.requested("https://example.com/cdn-cgi/image/width=800/photo.jpg"),
        1
    );
    let (unfiltered, summary) = crawl(&site, |config| {
        config.robots = RobotsPolicy::Ignore;
        config.follow.sitemaps = false;
        config.urls.exclude.clear();
    });
    assert!(unfiltered.crawled("https://example.com/cdn-cgi/l/email-protection"));
    assert_eq!(summary.progress.errors, 5);
}
