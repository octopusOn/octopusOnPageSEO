//! URL list ↔ log comparison.

use std::collections::{BTreeSet, HashMap};

use octopuson_core::Timestamp;
use octopuson_core::table::{Cell, Column, ColumnKind};
use serde::Serialize;

/// Where a URL was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Presence {
    /// Listed and requested.
    Both,
    /// Listed but never requested (e.g. not crawled).
    NotInLogs,
    /// Requested but not listed (orphan pages, removed pages, parameter variants).
    NotInList,
}

impl Presence {
    /// Stable machine name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::NotInLogs => "not-in-logs",
            Self::NotInList => "not-in-list",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Entry {
    listed: bool,
    hits: u64,
    last_seen: Option<Timestamp>,
    statuses: BTreeSet<u16>,
}

/// Accumulates listed and logged URLs keyed by their normalized form.
#[derive(Debug, Clone, Default)]
pub struct UrlComparison {
    entries: HashMap<String, Entry>,
}

/// Counts per [`Presence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct ComparisonSummary {
    /// Distinct listed URLs.
    pub listed: u64,
    /// Distinct logged URLs.
    pub logged: u64,
    /// Listed and logged.
    pub both: u64,
    /// Listed only.
    pub not_in_logs: u64,
    /// Logged only.
    pub not_in_list: u64,
}

/// One output row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonRow {
    /// Normalized URL.
    pub url: String,
    /// Where it was found.
    pub presence: Presence,
    /// Logged hits.
    pub hits: u64,
    /// Last logged request.
    pub last_seen: Option<Timestamp>,
    /// Distinct logged status codes.
    pub statuses: Vec<u16>,
}

impl UrlComparison {
    /// Adds a URL from the list.
    pub fn add_listed(&mut self, key: String) {
        self.entries.entry(key).or_default().listed = true;
    }

    /// Adds logged traffic of a URL; several raw URLs may share a key.
    pub fn add_logged(
        &mut self,
        key: String,
        hits: u64,
        last_seen: Option<Timestamp>,
        statuses: impl IntoIterator<Item = u16>,
    ) {
        let entry = self.entries.entry(key).or_default();
        entry.hits += hits;
        entry.last_seen = entry.last_seen.max(last_seen);
        entry.statuses.extend(statuses);
    }

    /// Counts per presence.
    pub fn summary(&self) -> ComparisonSummary {
        let mut summary = ComparisonSummary::default();
        for entry in self.entries.values() {
            let logged = entry.hits > 0;
            summary.listed += u64::from(entry.listed);
            summary.logged += u64::from(logged);
            match (entry.listed, logged) {
                (true, true) => summary.both += 1,
                (true, false) => summary.not_in_logs += 1,
                (false, true) => summary.not_in_list += 1,
                (false, false) => {}
            }
        }
        summary
    }

    /// Rows ordered by presence, then most hits, then URL.
    pub fn into_rows(self) -> Vec<ComparisonRow> {
        let mut rows: Vec<ComparisonRow> = self
            .entries
            .into_iter()
            .filter_map(|(url, entry)| {
                let presence = match (entry.listed, entry.hits > 0) {
                    (true, true) => Presence::Both,
                    (true, false) => Presence::NotInLogs,
                    (false, true) => Presence::NotInList,
                    (false, false) => return None,
                };
                Some(ComparisonRow {
                    url,
                    presence,
                    hits: entry.hits,
                    last_seen: entry.last_seen,
                    statuses: entry.statuses.into_iter().collect(),
                })
            })
            .collect();
        rows.sort_by(|a, b| (a.presence, b.hits, &a.url).cmp(&(b.presence, a.hits, &b.url)));
        rows
    }
}

impl ComparisonRow {
    /// Result columns, matching [`Self::cells`].
    pub fn columns() -> Vec<Column> {
        vec![
            Column::new("url", ColumnKind::Text),
            Column::new("presence", ColumnKind::Text),
            Column::new("hits", ColumnKind::Integer),
            Column::new("last_seen", ColumnKind::Timestamp),
            Column::new("status_codes", ColumnKind::Text),
        ]
    }

    /// Cells in column order.
    pub fn cells(&self) -> Vec<Cell> {
        let statuses: Vec<String> = self.statuses.iter().map(u16::to_string).collect();
        vec![
            Cell::Text(self.url.clone()),
            Cell::Text(self.presence.name().to_owned()),
            Cell::Integer(i64::try_from(self.hits).unwrap_or(i64::MAX)),
            self.last_seen.map_or(Cell::Null, Cell::Timestamp),
            if statuses.is_empty() {
                Cell::Null
            } else {
                Cell::Text(statuses.join(","))
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_and_orders_urls() {
        let mut comparison = UrlComparison::default();
        for url in ["/a", "/b", "/c"] {
            comparison.add_listed(url.into());
        }
        let t = |micros| Some(Timestamp::from_unix_micros(micros));
        comparison.add_logged("/a".into(), 3, t(5), [200]);
        comparison.add_logged("/a".into(), 2, t(9), [301]);
        comparison.add_logged("/orphan".into(), 7, t(1), [404]);
        comparison.add_logged("/zero".into(), 0, None, []);

        assert_eq!(
            comparison.summary(),
            ComparisonSummary {
                listed: 3,
                logged: 2,
                both: 1,
                not_in_logs: 2,
                not_in_list: 1,
            }
        );
        let rows = comparison.into_rows();
        let order: Vec<(&str, Presence)> =
            rows.iter().map(|r| (r.url.as_str(), r.presence)).collect();
        assert_eq!(
            order,
            [
                ("/a", Presence::Both),
                ("/b", Presence::NotInLogs),
                ("/c", Presence::NotInLogs),
                ("/orphan", Presence::NotInList),
            ]
        );
        assert_eq!((rows[0].hits, rows[0].last_seen), (5, t(9)));
        assert_eq!(rows[0].cells()[4], Cell::Text("200,301".into()));
        assert_eq!(rows[1].cells()[3], Cell::Null);
    }
}
