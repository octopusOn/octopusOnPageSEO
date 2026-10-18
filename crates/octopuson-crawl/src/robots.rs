//! robots.txt of every origin the crawl touches, fetched once and shared by the workers.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use octopuson_url::robots::Robots;
use url::{Position, Url};

use crate::fetch::{FetchRequest, Fetcher, follows_location};
use crate::record::{RobotsRecord, RobotsStatus};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod redirect_tests;

/// Redirects followed for robots.txt (RFC 9309 asks for at least five).
const MAX_REDIRECTS: usize = 5;

/// robots.txt files above this size are cut (Google reads 500 KiB).
const MAX_BYTES: u64 = 512 * 1024;

/// The robots.txt of one origin.
#[derive(Debug)]
pub(crate) struct HostRobots {
    status: RobotsStatus,
    http_status: Option<u16>,
    robots: Robots,
}

impl HostRobots {
    pub(crate) fn sitemaps(&self) -> &[String] {
        &self.robots.sitemaps
    }
}

/// The decision for one URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Verdict {
    pub(crate) allowed: bool,
    /// The deciding rule, e.g. `Disallow: /admin/`.
    pub(crate) rule: Option<String>,
}

/// robots.txt per origin, fetched on first use.
#[derive(Debug)]
pub(crate) struct RobotsCache {
    tokens: &'static [&'static str],
    origins: Mutex<HashMap<String, Arc<OnceLock<HostRobots>>>>,
    /// Responses first observed by browser policy have no page outcome to carry them.
    deferred: Mutex<Vec<RobotsRecord>>,
}

impl RobotsCache {
    pub(crate) fn new(tokens: &'static [&'static str]) -> Self {
        Self {
            tokens,
            origins: Mutex::new(HashMap::new()),
            deferred: Mutex::new(Vec::new()),
        }
    }

    /// The robots.txt of `url`'s origin, plus its record when this call fetched it.
    pub(crate) fn host(
        &self,
        fetcher: &dyn Fetcher,
        url: &Url,
    ) -> (Arc<OnceLock<HostRobots>>, Option<RobotsRecord>) {
        let origin = url.origin().ascii_serialization();
        let cell = {
            let mut origins = self.origins.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(origins.entry(origin.clone()).or_default())
        };
        let mut record = None;
        cell.get_or_init(|| {
            let (host, fetched) = fetch_robots(fetcher, &format!("{origin}/robots.txt"));
            record = Some(fetched);
            host
        });
        (cell, record)
    }

    /// Whether the crawling agent may fetch `url`, plus the origin's robots record when this
    /// call fetched it.
    pub(crate) fn check(
        &self,
        fetcher: &dyn Fetcher,
        url: &Url,
    ) -> (Verdict, Option<RobotsRecord>) {
        let (cell, record) = self.host(fetcher, url);
        let Some(host) = cell.get() else {
            return (allowed(), record);
        };
        let verdict = match host.status {
            RobotsStatus::Missing => allowed(),
            RobotsStatus::Unreachable => Verdict {
                allowed: false,
                rule: Some(
                    if host.http_status == Some(429) {
                        "robots.txt rate limited (HTTP 429)"
                    } else {
                        "robots.txt unreachable"
                    }
                    .to_owned(),
                ),
            },
            RobotsStatus::Found => {
                let decision = host
                    .robots
                    .group_for(self.tokens, true)
                    .decide(&url[Position::BeforePath..]);
                Verdict {
                    allowed: decision.allowed,
                    rule: decision.rule.map(|rule| {
                        format!(
                            "{}: {}",
                            if rule.allow { "Allow" } else { "Disallow" },
                            rule.pattern
                        )
                    }),
                }
            }
        };
        (verdict, record)
    }

    /// Browser policy shares decisions while retaining newly observed responses for storage.
    pub(crate) fn check_resource(&self, fetcher: &dyn Fetcher, url: &Url) -> Verdict {
        let (verdict, record) = self.check(fetcher, url);
        if let Some(record) = record {
            self.deferred
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(record);
        }
        verdict
    }

    /// Called after workers have joined, including a normally stopped crawl.
    pub(crate) fn take_deferred(&self) -> Vec<RobotsRecord> {
        std::mem::take(&mut *self.deferred.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

fn allowed() -> Verdict {
    Verdict {
        allowed: true,
        rule: None,
    }
}

/// Fetches robots.txt following redirects; ordinary 4xx means no rules. HTTP 429, 5xx or
/// no answer conservatively block this origin for the crawl's cache lifetime.
fn fetch_robots(fetcher: &dyn Fetcher, url: &str) -> (HostRobots, RobotsRecord) {
    let mut record = RobotsRecord {
        url: url.to_owned(),
        status: RobotsStatus::Unreachable,
        http_status: None,
        text: String::new(),
        sitemaps: Vec::new(),
    };
    let mut target = url.to_owned();
    let mut visited = HashSet::new();
    for _ in 0..=MAX_REDIRECTS {
        if !visited.insert(target.clone()) {
            break;
        }
        let request = FetchRequest {
            url: &target,
            body_limit: MAX_BYTES,
        };
        let Ok(response) = fetcher.fetch(&request) else {
            break;
        };
        record.http_status = Some(response.status);
        match response.status {
            status if follows_location(status) => {
                let next = response.header("location").and_then(|location| {
                    let mut next = Url::parse(&target).ok()?.join(location.trim()).ok()?;
                    if !matches!(next.scheme(), "http" | "https") {
                        return None;
                    }
                    next.set_fragment(None);
                    Some(next)
                });
                match next {
                    Some(next) => target = next.into(),
                    None => break,
                }
            }
            200..=299 => {
                record.status = RobotsStatus::Found;
                record.text = String::from_utf8_lossy(&response.body).into_owned();
                break;
            }
            429 => break,
            400..=499 => {
                record.status = RobotsStatus::Missing;
                break;
            }
            _ => break,
        }
    }
    let robots = Robots::parse(&record.text);
    record.sitemaps.clone_from(&robots.sitemaps);
    let host = HostRobots {
        status: record.status,
        http_status: record.http_status,
        robots,
    };
    (host, record)
}
