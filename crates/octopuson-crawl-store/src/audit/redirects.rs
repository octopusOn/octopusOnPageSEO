//! Resolve observed redirect graphs once per node, including tails entering cycles.

use std::collections::{HashMap, HashSet};

use duckdb::Connection;

use crate::CrawlStoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Other,
    Error,
    Loop,
}

pub(super) fn check(connection: &Connection) -> Result<(), CrawlStoreError> {
    let mut statement = connection.prepare(
        "SELECT url,redirect_url FROM urls WHERE status IN (301,302,303,307,308) AND redirect_url IS NOT NULL",
    )?;
    let redirects: HashMap<String, String> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut statement =
        connection.prepare("SELECT url FROM urls WHERE status>=400 OR error IS NOT NULL")?;
    let errors: HashSet<String> = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for (url, end) in resolve(&redirects, &errors) {
        match end {
            End::Error => super::insert(connection, "redirect-to-error", url)?,
            End::Loop => super::insert(connection, "redirect-loop", url)?,
            End::Other => {}
        }
    }
    Ok(())
}

fn resolve<'a>(
    redirects: &'a HashMap<String, String>,
    errors: &HashSet<String>,
) -> HashMap<&'a str, End> {
    let mut resolved = HashMap::new();
    for start in redirects.keys() {
        if resolved.contains_key(start.as_str()) {
            continue;
        }
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let mut current = start.as_str();
        let end = loop {
            if let Some(&end) = resolved.get(current) {
                break end;
            }
            if !seen.insert(current) {
                break End::Loop;
            }
            let Some(next) = redirects.get(current) else {
                break if errors.contains(current) {
                    End::Error
                } else {
                    End::Other
                };
            };
            path.push(current);
            if errors.contains(current) {
                break End::Error;
            }
            current = next;
        };
        for url in path {
            resolved.insert(url, end);
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_navigation_statuses_do_not_propagate_legacy_redirect_errors_or_cycles() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE urls(url VARCHAR,status USMALLINT,redirect_url VARCHAR,error VARCHAR);
            CREATE TABLE audit_findings(issue_id VARCHAR,url VARCHAR,PRIMARY KEY(issue_id,url));
            INSERT INTO urls VALUES
              ('tail',301,'not-modified',NULL),('not-modified',304,'missing',NULL),
              ('choice',300,'choice',NULL),('unused',306,'missing',NULL),
              ('real',308,'missing',NULL),('missing',404,NULL,NULL);").unwrap();
        check(&connection).unwrap();
        let rows = connection
            .prepare("SELECT issue_id,url FROM audit_findings ORDER BY issue_id,url")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows, [("redirect-to-error".into(), "real".into())]);
    }

    #[test]
    fn resolves_tails_cycles_failures_and_unknown_endpoints() {
        let edges = HashMap::from_iter(
            [
                ("a", "b"),
                ("b", "404"),
                ("c", "a"),
                ("loop-a", "loop-b"),
                ("loop-b", "loop-a"),
                ("tail", "loop-a"),
                ("self", "self"),
                ("unknown", "unfetched"),
                ("ok", "200"),
            ]
            .map(|(a, b)| (a.to_owned(), b.to_owned())),
        );
        let resolved = resolve(&edges, &HashSet::from(["404".to_owned()]));
        for url in ["a", "b", "c"] {
            assert_eq!(resolved[url], End::Error);
        }
        for url in ["loop-a", "loop-b", "tail", "self"] {
            assert_eq!(resolved[url], End::Loop);
        }
        for url in ["unknown", "ok"] {
            assert_eq!(resolved[url], End::Other);
        }
    }

    #[test]
    fn resolves_long_shared_chains_without_recursion() {
        let edges: HashMap<String, String> = (0..5000)
            .map(|i| (i.to_string(), (i + 1).to_string()))
            .collect();
        let resolved = resolve(&edges, &HashSet::from(["5000".to_owned()]));
        assert_eq!(resolved.len(), 5000);
        assert!(resolved.values().all(|&end| end == End::Error));
    }
}
