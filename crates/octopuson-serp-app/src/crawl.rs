//! Crawling the pages that rank, with the Spider engine.
//!
//! The result URLs are fetched as one list crawl (robots.txt respected, HTML kept, audits
//! run), saved under `<data>/crawls/` like any Spider crawl. A result that redirects to
//! another host is not parsed by a list crawl, so its final URL is fetched in a second list
//! crawl. Internal and external links are told apart per page later, from each page's own
//! host, so results of different sites do not count as one site.

use std::collections::HashMap;

use octopuson_core::table::{Cell, TableBuffer};
use octopuson_crawl::Storage;
use octopuson_spider_app::{
    AgentPreset, CrawlConfig, CrawlControl, CrawlEvent, CrawlMode, CrawlProgress, RenderMode,
    RobotsPolicy, SpiderWorkspace,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{SerpAppError, SerpWorkspace};

/// Redirects followed from a result URL to its page.
const MAX_HOPS: usize = 8;

/// How result pages identify themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrawlIdentity {
    /// The OctopusOn SEO crawler user agent.
    #[default]
    Octopus,
    /// A desktop Chrome user agent (sites that serve crawlers differently).
    Chrome,
}

/// How result pages are crawled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCrawlOptions {
    /// Run JavaScript in a local browser before reading pages.
    #[serde(default)]
    pub javascript: bool,
    /// User agent.
    #[serde(default)]
    pub identity: CrawlIdentity,
}

/// One crawled result page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawledPage {
    /// The result URL.
    pub requested: String,
    /// URL of the page after redirects.
    pub url: String,
    /// Redirects followed.
    pub redirects: u32,
    /// HTTP status of the page, if a response arrived.
    pub status: Option<u16>,
    /// Network error, if none arrived.
    pub error: Option<String>,
    /// Time to the response headers, milliseconds.
    pub response_ms: u32,
    /// Body size in bytes.
    pub bytes: u64,
    /// The Spider's indexability verdict (`indexable`, `noindex`, ...).
    pub indexability: String,
    /// Whether the response sent HSTS.
    pub hsts: bool,
    /// robots.txt disallowed fetching the page.
    pub blocked_by_robots: bool,
    /// Media type.
    pub content_type: Option<String>,
    /// The HTML (rendered when JavaScript ran).
    pub html: Option<String>,
    /// Audit issue ids found on the page.
    pub findings: Vec<String>,
}

/// Pages of a set of result URLs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCrawl {
    /// Saved crawls the pages came from.
    pub crawl_ids: Vec<String>,
    /// One entry per requested URL, in order.
    pub pages: Vec<CrawledPage>,
}

#[derive(Debug, Clone)]
struct Row {
    url: String,
    status: Option<u16>,
    error: Option<String>,
    response_ms: u32,
    bytes: u64,
    redirect: Option<String>,
    indexability: String,
    hsts: bool,
    blocked: bool,
    content_type: Option<String>,
    html: Option<String>,
}

impl SerpWorkspace {
    /// The Spider workspace of this data directory (`<data>/crawls`).
    ///
    /// # Errors
    /// Never in practice; the directory is given.
    pub fn spider(&self) -> Result<SpiderWorkspace, SerpAppError> {
        SpiderWorkspace::resolve(Some(self.data_dir().to_owned()))
            .map_err(|error| SerpAppError::Crawl(error.to_string()))
    }

    /// Crawls result URLs and returns their pages in the order given.
    ///
    /// # Errors
    /// [`SerpAppError::Crawl`] for invalid URLs, a missing browser (with JavaScript) or
    /// storage failures; network failures are reported per page.
    pub fn crawl_pages(
        &self,
        urls: &[String],
        options: PageCrawlOptions,
        control: &CrawlControl,
        on_progress: &mut dyn FnMut(&CrawlProgress),
    ) -> Result<PageCrawl, SerpAppError> {
        let spider = self.spider()?;
        let mut crawl_ids = Vec::new();
        let mut rows = HashMap::new();
        if urls.is_empty() {
            return Ok(PageCrawl {
                crawl_ids,
                pages: Vec::new(),
            });
        }
        crawl_ids.push(crawl_list(
            &spider,
            urls,
            options,
            control,
            on_progress,
            &mut rows,
        )?);
        // Redirects that leave the crawl's hosts end at URLs the first crawl did not parse.
        let missing: Vec<String> = urls
            .iter()
            .filter_map(|url| {
                let (end, _) = follow(&rows, url);
                (!rows.contains_key(&key(&end))).then_some(end)
            })
            .collect();
        if !missing.is_empty() && !control.is_stopped() {
            crawl_ids.push(crawl_list(
                &spider,
                &missing,
                options,
                control,
                on_progress,
                &mut rows,
            )?);
        }
        let findings = crawl_ids
            .iter()
            .map(|id| findings_of(&spider, id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .fold(
                HashMap::<String, Vec<String>>::new(),
                |mut map, (issue, url)| {
                    map.entry(key(&url)).or_default().push(issue);
                    map
                },
            );
        let pages = urls
            .iter()
            .map(|requested| {
                let (end, redirects) = follow(&rows, requested);
                let row = rows.get(&key(&end));
                CrawledPage {
                    requested: requested.clone(),
                    url: row.map_or_else(|| end.clone(), |row| row.url.clone()),
                    redirects,
                    status: row.and_then(|row| row.status),
                    error: row
                        .and_then(|row| row.error.clone())
                        .or_else(|| row.is_none().then(|| "not crawled".to_owned())),
                    response_ms: row.map_or(0, |row| row.response_ms),
                    bytes: row.map_or(0, |row| row.bytes),
                    indexability: row
                        .map_or_else(|| "unknown".to_owned(), |row| row.indexability.clone()),
                    hsts: row.is_some_and(|row| row.hsts),
                    blocked_by_robots: row.is_some_and(|row| row.blocked),
                    content_type: row.and_then(|row| row.content_type.clone()),
                    html: row.and_then(|row| row.html.clone()),
                    findings: findings.get(&key(&end)).cloned().unwrap_or_default(),
                }
            })
            .collect();
        Ok(PageCrawl { crawl_ids, pages })
    }
}

/// Comparable form of a URL: parsed, without the fragment.
fn key(url: &str) -> String {
    Url::parse(url.trim()).map_or_else(
        |_| url.trim().to_owned(),
        |mut url| {
            url.set_fragment(None);
            url.to_string()
        },
    )
}

/// Where a URL ends after the redirects the crawl recorded, and how many hops it took.
fn follow(rows: &HashMap<String, Row>, url: &str) -> (String, u32) {
    let mut current = key(url);
    let mut hops = 0;
    while hops < MAX_HOPS {
        let Some(next) = rows.get(&current).and_then(|row| row.redirect.clone()) else {
            break;
        };
        current = key(&next);
        hops += 1;
    }
    (current, u32::try_from(hops).unwrap_or(u32::MAX))
}

fn crawl_list(
    spider: &SpiderWorkspace,
    urls: &[String],
    options: PageCrawlOptions,
    control: &CrawlControl,
    on_progress: &mut dyn FnMut(&CrawlProgress),
    rows: &mut HashMap<String, Row>,
) -> Result<String, SerpAppError> {
    let mut config = CrawlConfig {
        start_urls: urls.to_vec(),
        mode: CrawlMode::List,
        agent: match options.identity {
            CrawlIdentity::Octopus => AgentPreset::Octopus,
            CrawlIdentity::Chrome => AgentPreset::Chrome,
        },
        robots: RobotsPolicy::Respect,
        storage: Storage {
            headers: false,
            html: true,
        },
        ..CrawlConfig::default()
    };
    config.follow.check_external = false;
    config.limits.max_urls = config
        .limits
        .max_urls
        .max(u32::try_from(urls.len() * (MAX_HOPS + 1)).unwrap_or(u32::MAX));
    if options.javascript {
        config.rendering.mode = RenderMode::Javascript;
    }
    let meta = spider
        .crawl_observed(config, control, |event| {
            if let CrawlEvent::Progress(progress) = event {
                on_progress(progress);
            }
        })
        .map_err(|error| SerpAppError::Crawl(error.to_string()))?;
    let reader = spider
        .open(&meta.id)
        .map_err(|error| SerpAppError::Crawl(error.to_string()))?;
    let mut table = TableBuffer::default();
    reader
        .query(
            "SELECT u.url, u.status, u.error, u.response_ms, u.size_bytes, u.redirect_url,
                    u.indexability, u.hsts, u.blocked_by_robots, u.content_type, h.html
             FROM urls u LEFT JOIN html h ON h.page = u.id",
            &mut table,
        )
        .map_err(|error| SerpAppError::Crawl(error.to_string()))?;
    for cells in table.rows {
        let row = Row {
            url: text(&cells, 0).unwrap_or_default(),
            status: integer(&cells, 1).and_then(|status| u16::try_from(status).ok()),
            error: text(&cells, 2),
            response_ms: integer(&cells, 3)
                .and_then(|ms| u32::try_from(ms).ok())
                .unwrap_or(0),
            bytes: integer(&cells, 4)
                .and_then(|bytes| u64::try_from(bytes).ok())
                .unwrap_or(0),
            redirect: text(&cells, 5),
            indexability: text(&cells, 6).unwrap_or_default(),
            hsts: boolean(&cells, 7),
            blocked: boolean(&cells, 8),
            content_type: text(&cells, 9),
            html: text(&cells, 10),
        };
        rows.insert(key(&row.url), row);
    }
    Ok(meta.id)
}

fn findings_of(spider: &SpiderWorkspace, id: &str) -> Result<Vec<(String, String)>, SerpAppError> {
    let reader = spider
        .open(id)
        .map_err(|error| SerpAppError::Crawl(error.to_string()))?;
    let mut table = TableBuffer::default();
    reader
        .query("SELECT issue_id, url FROM audit_findings", &mut table)
        .map_err(|error| SerpAppError::Crawl(error.to_string()))?;
    Ok(table
        .rows
        .iter()
        .filter_map(|cells| Some((text(cells, 0)?, text(cells, 1)?)))
        .collect())
}

fn text(cells: &[Cell], index: usize) -> Option<String> {
    match cells.get(index)? {
        Cell::Text(text) => Some(text.clone()),
        Cell::Null => None,
        other => Some(other.to_string()),
    }
}

fn integer(cells: &[Cell], index: usize) -> Option<i64> {
    match cells.get(index)? {
        Cell::Integer(value) => Some(*value),
        Cell::Text(text) => text.parse().ok(),
        _ => None,
    }
}

fn boolean(cells: &[Cell], index: usize) -> bool {
    matches!(cells.get(index), Some(Cell::Boolean(true)))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;
    use crate::secrets::{FileBackend, Secrets};

    /// Serves a page, a cross-host redirect (127.0.0.1 → localhost) and its target.
    fn serve() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let _ = reader.read_line(&mut request);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                    line.clear();
                }
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let response = match path.as_str() {
                    "/page" => html(
                        "<html><head><title>Kahve</title></head><body><h1>Türk kahvesi</h1><p>Cezve ile.</p></body></html>",
                    ),
                    "/moved" => format!(
                        "HTTP/1.1 301 Moved Permanently\r\nLocation: http://localhost:{port}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    ),
                    "/target" => html(
                        "<html><head><title>Hedef</title></head><body><h1>Yeni adres</h1></body></html>",
                    ),
                    _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_owned(),
                };
                let _ = stream.write_all(response.as_bytes());
            }
        });
        port
    }

    fn html(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn crawls_results_and_follows_cross_host_redirects() {
        let port = serve();
        let dir = tempfile::tempdir().unwrap();
        let workspace = SerpWorkspace::with_secrets(
            dir.path().to_owned(),
            Secrets::new(
                Box::new(FileBackend::new(dir.path().join("s.json"))),
                dir.path().join("b.json"),
            ),
        );
        let urls = vec![
            format!("http://127.0.0.1:{port}/page"),
            format!("http://127.0.0.1:{port}/moved"),
            format!("http://127.0.0.1:{port}/gone"),
        ];
        let control = CrawlControl::new();
        let crawl = workspace
            .crawl_pages(&urls, PageCrawlOptions::default(), &control, &mut |_| {})
            .unwrap();
        assert_eq!(crawl.crawl_ids.len(), 2);
        let [page, moved, gone] = &crawl.pages[..] else {
            panic!("expected three pages");
        };
        assert_eq!(page.status, Some(200));
        assert!(page.html.as_deref().unwrap().contains("Cezve"));
        assert_eq!(moved.redirects, 1);
        assert_eq!(moved.url, format!("http://localhost:{port}/target"));
        assert_eq!(moved.status, Some(200));
        assert!(moved.html.as_deref().unwrap().contains("Yeni adres"));
        assert_eq!(gone.status, Some(404));
        assert!(gone.html.is_none());
        assert!(dir.path().join("crawls").is_dir());
    }
}
