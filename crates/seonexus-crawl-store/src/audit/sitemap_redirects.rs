//! Resolve saved sitemap redirect graphs once per node, without recursive SQL or HTTP.

use crate::CrawlStoreError;
use duckdb::{Connection, params};
use std::collections::{HashMap, HashSet};

struct Node {
    status: Option<u16>,
    next: Option<String>,
    error: Option<String>,
}
impl Node {
    fn is_redirect(&self) -> bool {
        self.next.is_some() || matches!(self.status, Some(300..=399))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Complete,
    Error,
    Loop,
    Unchecked,
}
impl End {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Loop => "loop",
            Self::Unchecked => "unchecked",
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Resolution<'a> {
    end: End,
    url: Option<&'a str>,
    status: Option<u16>,
    hops: Option<u32>,
    cause: Option<&'a str>,
}

pub(super) fn check(connection: &Connection) -> Result<(), CrawlStoreError> {
    connection.execute("DELETE FROM sitemap_redirects", [])?;
    let mut statement = connection.prepare("SELECT url,status,redirect_url,error FROM sitemaps")?;
    let nodes: HashMap<String, Node> = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                Node {
                    status: row.get(1)?,
                    next: row.get(2)?,
                    error: row.get(3)?,
                },
            ))
        })?
        .collect::<Result<_, _>>()?;
    let resolutions = resolve(&nodes);
    {
        let mut rows = connection.appender("sitemap_redirects")?;
        for (url, node) in &nodes {
            if !node.is_redirect() {
                continue;
            }
            let result = resolutions[url.as_str()];
            let error = match result.end {
                End::Loop => Some("sitemap redirect loop".to_owned()),
                End::Error => Some(result.cause.map_or_else(
                    || "sitemap redirect destination failed".to_owned(),
                    |cause| format!("sitemap redirect destination failed: {cause}"),
                )),
                End::Complete | End::Unchecked => None,
            };
            rows.append_row(params![
                url,
                result.end.as_str(),
                result.url,
                result.status,
                result.hops,
                error
            ])?;
        }
        rows.flush()?;
    }
    for (url, node) in &nodes {
        if node.is_redirect() && matches!(resolutions[url.as_str()].end, End::Error | End::Loop) {
            super::insert(connection, "sitemap-error", url)?;
        }
    }
    Ok(())
}

fn resolve(nodes: &HashMap<String, Node>) -> HashMap<&str, Resolution<'_>> {
    let mut resolved = HashMap::new();
    for start in nodes.keys() {
        if resolved.contains_key(start.as_str()) {
            continue;
        }
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let mut current = start.as_str();
        let mut result = loop {
            if let Some(&result) = resolved.get(current) {
                break result;
            }
            if !seen.insert(current) {
                break Resolution {
                    end: End::Loop,
                    url: None,
                    status: None,
                    hops: None,
                    cause: None,
                };
            }
            let Some(node) = nodes.get(current) else {
                break Resolution {
                    end: End::Unchecked,
                    url: Some(current),
                    status: None,
                    hops: Some(0),
                    cause: None,
                };
            };
            if node.error.is_none()
                && let Some(next) = node.next.as_deref()
            {
                path.push(current);
                current = next;
                continue;
            }
            let end = if node.error.is_some() {
                End::Error
            } else {
                match node.status {
                    Some(200..=299) => End::Complete,
                    Some(_) => End::Error,
                    None => End::Unchecked,
                }
            };
            let result = Resolution {
                end,
                url: Some(current),
                status: node.status,
                hops: Some(0),
                cause: node.error.as_deref(),
            };
            resolved.insert(current, result);
            break result;
        };
        for url in path.into_iter().rev() {
            result.hops = result.hops.map(|hops| hops.saturating_add(1));
            resolved.insert(url, result);
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    fn redirect(next: &str) -> Node {
        Node {
            status: Some(301),
            next: Some(next.into()),
            error: None,
        }
    }
    #[test]
    fn distinguishes_shared_targets_cycles_failures_and_unfetched_ends() {
        let nodes = HashMap::from([
            ("a".into(), redirect("b")),
            ("b".into(), redirect("good")),
            ("shared".into(), redirect("good")),
            (
                "good".into(),
                Node {
                    status: Some(200),
                    next: None,
                    error: None,
                },
            ),
            ("loop".into(), redirect("self")),
            ("self".into(), redirect("self")),
            ("bad".into(), redirect("broken")),
            (
                "broken".into(),
                Node {
                    status: Some(200),
                    next: None,
                    error: Some("invalid XML".into()),
                },
            ),
            ("unknown".into(), redirect("unfetched")),
        ]);
        let result = resolve(&nodes);
        assert_eq!(result["a"].end, End::Complete);
        assert_eq!(result["a"].hops, Some(2));
        assert_eq!(result["a"].url, Some("good"));
        assert_eq!(result["shared"].hops, Some(1));
        for name in ["loop", "self"] {
            assert_eq!(result[name].end, End::Loop);
            assert!(result[name].url.is_none() && result[name].hops.is_none());
        }
        assert_eq!(result["bad"].end, End::Error);
        assert_eq!(result["bad"].status, Some(200));
        assert_eq!(result["bad"].cause, Some("invalid XML"));
        assert_eq!(result["unknown"].end, End::Unchecked);
        assert_eq!(result["unknown"].url, Some("unfetched"));
        assert_eq!(result["unknown"].hops, Some(1));
    }
    #[test]
    fn resolves_long_shared_chains_without_recursion_or_a_fixed_trace_cutoff() {
        let mut nodes: HashMap<_, _> = (0..5000)
            .map(|i| (i.to_string(), redirect(&(i + 1).to_string())))
            .collect();
        nodes.insert(
            "5000".into(),
            Node {
                status: Some(200),
                next: None,
                error: None,
            },
        );
        let result = resolve(&nodes);
        assert_eq!(result.len(), 5001);
        assert_eq!(result["0"].hops, Some(5000));
        assert!(
            result
                .values()
                .all(|row| row.end == End::Complete && row.url == Some("5000"))
        );
    }
}
