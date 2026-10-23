//! The before/after part of a report: two snapshots of the project compared — where the work
//! began, or the snapshot before, against the latest — told in words, with headline numbers,
//! every number before and after, and what moved in between.

use super::texts::{self, fill, number, percent, text};
use super::{Cell, Change, Table, Tile, Widget, count_cell, engine_name, short};
use crate::modules::snapshots::Compared;
use crate::snapshot::DomainPoint;
use crate::snapshot::compare::{KeywordMove, Kpi, PageMove};

/// Rows of each table.
const ROWS: usize = 15;

/// Headline numbers, in the order they are shown.
const HEADLINE: &[&str] = &[
    "traffic",
    "keywords",
    "top10",
    "referring_domains",
    "oa",
    "visibility",
    "health",
    "mention_rate",
    "clicks",
];

/// Headline tiles at most.
const TILES: usize = 8;

/// How a number is written.
#[derive(Clone, Copy)]
enum Kind {
    Count,
    /// 0–100, written as a percentage.
    Rate,
    /// Scores and positions, one decimal.
    Score,
    Money,
}

fn kind(key: &str) -> Kind {
    match key {
        "visibility" | "mention_rate" | "citation_rate" | "share_of_voice" | "ctr"
        | "local_solv" => Kind::Rate,
        "average_position" | "gsc_position" | "health" | "geo" | "local_arp" | "local_rating" => {
            Kind::Score
        }
        "traffic_value" => Kind::Money,
        _ => Kind::Count,
    }
}

/// The report's word for a number.
fn label(key: &str) -> &str {
    match key {
        "traffic" => "organic_traffic",
        "keywords" => "organic_keywords",
        "top3" => "kpi_top3",
        "top10" => "kpi_top10",
        "oa" => "authority",
        "backlinks" => "backlink_count",
        "geo" => "geo_readiness",
        "issues" => "findings",
        "sessions" => "organic_sessions",
        "risky_links" => "risky_links_kpi",
        other => other,
    }
}

fn written(key: &str, value: f64, lang: &str) -> String {
    match kind(key) {
        Kind::Rate => percent(value, lang),
        Kind::Score => number(value, 1, lang),
        Kind::Money => format!("${}", number(value, 0, lang)),
        Kind::Count => number(value, 0, lang),
    }
}

fn signed(value: f64, decimals: usize, lang: &str) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    format!("{sign}{}", number(value, decimals, lang))
}

fn signed_percent(value: f64, lang: &str) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    format!("{sign}{}", percent(value, lang))
}

/// How much a number moved: in percent for counts, in points for rates and scores.
fn delta(kpi: &Kpi, lang: &str) -> Option<String> {
    let change = kpi.change.filter(|change| change.abs() > f64::EPSILON)?;
    Some(match (kind(kpi.key), kpi.percent) {
        (Kind::Count | Kind::Money, Some(share)) => signed_percent(share, lang),
        (Kind::Count | Kind::Money, None) => signed(change, 0, lang),
        (Kind::Rate | Kind::Score, _) => signed(change, 1, lang),
    })
}

fn headline(kpi: &Kpi, lang: &str) -> Tile {
    let change = kpi
        .good()
        .zip(delta(kpi, lang))
        .map(|(good, moved)| Change {
            text: match kpi.before {
                Some(before) => format!(
                    "{moved} ({} {})",
                    text(lang, "was"),
                    written(kpi.key, before, lang)
                ),
                None => moved,
            },
            good,
        });
    Tile {
        label: text(lang, label(kpi.key)),
        value: kpi
            .after
            .map_or_else(|| "—".to_owned(), |after| written(kpi.key, after, lang)),
        change,
    }
}

fn value_cell(key: &str, value: Option<f64>, lang: &str) -> Cell {
    value.map_or_else(
        || Cell::Text("—".to_owned()),
        |value| Cell::Number {
            value,
            text: written(key, value, lang),
        },
    )
}

fn signed_cell(value: Option<f64>, decimals: usize, lang: &str) -> Cell {
    value.map_or_else(
        || Cell::Text(String::new()),
        |value| Cell::Number {
            value,
            text: signed(value, decimals, lang),
        },
    )
}

fn position_cell(position: Option<u32>, lang: &str) -> Cell {
    count_cell(position.map(f64::from), lang)
}

fn places_cell(change: Option<i64>, lang: &str) -> Cell {
    #[allow(clippy::cast_precision_loss)] // places on a result page
    signed_cell(change.map(|change| change as f64), 0, lang)
}

fn columns(keys: &[&str], lang: &str) -> Vec<String> {
    keys.iter().map(|key| text(lang, key)).collect()
}

/// A table of up to [`ROWS`] rows; none when it has no rows.
fn table<T>(
    tables: &mut Vec<Table>,
    title: &str,
    keys: &[&str],
    items: &[T],
    lang: &str,
    row: impl Fn(&T) -> Vec<Cell>,
) {
    if items.is_empty() {
        return;
    }
    tables.push(Table {
        title: text(lang, title),
        columns: columns(keys, lang),
        rows: items.iter().take(ROWS).map(row).collect(),
    });
}

/// A site's number after, with its change.
fn site_cell(kpi: Option<&Kpi>, lang: &str) -> Cell {
    let Some(kpi) = kpi else {
        return Cell::Text("—".to_owned());
    };
    let Some(after) = kpi.after else {
        return Cell::Text("—".to_owned());
    };
    let written = written(kpi.key, after, lang);
    Cell::Number {
        value: after,
        text: match delta(kpi, lang) {
            Some(moved) => format!("{written} ({moved})"),
            None => written,
        },
    }
}

/// The date a side was taken, with its name.
fn side(taken_at: &str, label: Option<&str>, lang: &str) -> String {
    let day = texts::date(taken_at, lang);
    match label.filter(|label| !label.trim().is_empty()) {
        Some(label) => format!("{day} ({label})"),
        None => day,
    }
}

/// The before/after part `id`; empty while the project has fewer than two snapshots.
#[allow(clippy::too_many_lines)] // the headline, the numbers and one table per kind of move
pub(crate) fn widget(id: &str, compared: Option<&Compared>, lang: &str) -> Widget {
    let Some(compared) = compared else {
        let mut widget = Widget::empty(id, lang);
        widget.note = Some(text(lang, "no_comparison"));
        return widget;
    };
    let comparison = &compared.comparison;
    let mut widget = Widget::new(id, lang);
    widget.note = Some(fill(
        lang,
        "comparison_period",
        &[
            (
                "from",
                &side(
                    &comparison.from.taken_at,
                    comparison.from.label.as_deref(),
                    lang,
                ),
            ),
            (
                "to",
                &side(
                    &comparison.to.taken_at,
                    comparison.to.label.as_deref(),
                    lang,
                ),
            ),
            ("days", &comparison.days.to_string()),
        ],
    ));
    widget.lines.clone_from(&compared.summary);
    widget.tiles = HEADLINE
        .iter()
        .filter_map(|key| comparison.kpis.iter().find(|kpi| kpi.key == *key))
        .filter(|kpi| kpi.after.is_some())
        .take(TILES)
        .map(|kpi| headline(kpi, lang))
        .collect();
    let tables = &mut widget.tables;
    table(
        tables,
        "all_metrics",
        &["metric", "before", "after", "change", "change_percent"],
        &comparison.kpis,
        lang,
        |kpi| {
            let decimals = match kind(kpi.key) {
                Kind::Rate | Kind::Score => 1,
                Kind::Count | Kind::Money => 0,
            };
            vec![
                Cell::Text(text(lang, label(kpi.key))),
                value_cell(kpi.key, kpi.before, lang),
                value_cell(kpi.key, kpi.after, lang),
                signed_cell(kpi.change, decimals, lang),
                kpi.percent.map_or_else(
                    || Cell::Text(String::new()),
                    |share| Cell::Number {
                        value: share,
                        text: signed_percent(share, lang),
                    },
                ),
            ]
        },
    );
    if comparison.sites.len() > 1 {
        table(
            tables,
            "you_and_competitors",
            &[
                "domain",
                "organic_traffic",
                "organic_keywords",
                "kpi_top10",
                "authority",
                "referring_domains",
            ],
            &comparison.sites,
            lang,
            |site| {
                let mut cells = vec![Cell::Text(site.domain.clone())];
                for key in ["traffic", "keywords", "top10", "oa", "referring_domains"] {
                    cells.push(site_cell(site.kpis.iter().find(|kpi| kpi.key == key), lang));
                }
                cells
            },
        );
    }
    if let Some(keywords) = &comparison.keywords {
        let moved = |row: &KeywordMove| {
            vec![
                Cell::Text(row.keyword.clone()),
                position_cell(row.before, lang),
                position_cell(row.after, lang),
                places_cell(row.change, lang),
                count_cell(Some(row.traffic_after), lang),
                Cell::Text(row.url.as_deref().map(short).unwrap_or_default()),
            ]
        };
        let keys = ["keyword", "before", "after", "change", "traffic", "page"];
        table(
            tables,
            "improved_keywords",
            &keys,
            &keywords.improved_keywords,
            lang,
            moved,
        );
        table(
            tables,
            "declined_keywords",
            &keys,
            &keywords.declined_keywords,
            lang,
            moved,
        );
        table(
            tables,
            "new_keywords",
            &["keyword", "position", "traffic", "page"],
            &keywords.new_keywords,
            lang,
            |row| {
                vec![
                    Cell::Text(row.keyword.clone()),
                    position_cell(row.after, lang),
                    count_cell(Some(row.traffic_after), lang),
                    Cell::Text(row.url.as_deref().map(short).unwrap_or_default()),
                ]
            },
        );
        table(
            tables,
            "lost_keywords",
            &["keyword", "position", "traffic", "page"],
            &keywords.lost_keywords,
            lang,
            |row| {
                vec![
                    Cell::Text(row.keyword.clone()),
                    position_cell(row.before, lang),
                    count_cell(Some(row.traffic_before), lang),
                    Cell::Text(row.url.as_deref().map(short).unwrap_or_default()),
                ]
            },
        );
    }
    if let Some(pages) = &comparison.pages {
        let keys = ["page", "before", "after", "change"];
        let row = |page: &PageMove| {
            vec![
                Cell::Text(short(&page.url)),
                count_cell(Some(page.before), lang),
                count_cell(Some(page.after), lang),
                signed_cell(Some(page.change), 0, lang),
            ]
        };
        table(tables, "pages_up", &keys, &pages.up, lang, row);
        table(tables, "pages_down", &keys, &pages.down, lang, row);
    }
    if let Some(domains) = &comparison.domains {
        let keys = ["domain", "authority"];
        let row = |domain: &DomainPoint| {
            vec![
                Cell::Text(domain.domain.clone()),
                count_cell(domain.rank, lang),
            ]
        };
        table(
            tables,
            "domains_gained",
            &keys,
            &domains.gained_domains,
            lang,
            row,
        );
        table(
            tables,
            "domains_lost",
            &keys,
            &domains.lost_domains,
            lang,
            row,
        );
    }
    if let Some(tracked) = &comparison.tracked {
        table(
            tables,
            "tracked_changes",
            &["keyword", "before", "after", "change"],
            &tracked.moves,
            lang,
            |row| {
                vec![
                    Cell::Text(row.keyword.clone()),
                    position_cell(row.before, lang),
                    position_cell(row.after, lang),
                    places_cell(row.change, lang),
                ]
            },
        );
    }
    if let Some(audit) = &comparison.audit {
        table(
            tables,
            "issues_fixed",
            &["issue", "before"],
            &audit.fixed,
            lang,
            |issue| {
                vec![
                    Cell::Text(issue.title.clone()),
                    count_cell(Some(issue.before), lang),
                ]
            },
        );
        table(
            tables,
            "issues_new",
            &["issue", "after"],
            &audit.new_issues,
            lang,
            |issue| {
                vec![
                    Cell::Text(issue.title.clone()),
                    count_cell(Some(issue.after), lang),
                ]
            },
        );
    }
    if let Some(local) = &comparison.local {
        table(
            tables,
            "local_changes",
            &["keyword", "local_solv_short", "local_arp_short"],
            &local.keywords,
            lang,
            |row| {
                let solv = |value: f64| percent(value, lang);
                let arp = |value: Option<f64>| {
                    value.map_or_else(|| "—".to_owned(), |value| number(value, 1, lang))
                };
                vec![
                    Cell::Text(row.keyword.clone()),
                    Cell::Number {
                        value: row.solv_after,
                        text: format!("{} → {}", solv(row.solv_before), solv(row.solv_after)),
                    },
                    Cell::Text(format!("{} → {}", arp(row.arp_before), arp(row.arp_after))),
                ]
            },
        );
    }
    if let Some(links) = &comparison.link_audit {
        table(
            tables,
            "risky_new",
            &["domain"],
            &links.new_risky,
            lang,
            |domain| vec![Cell::Text(domain.clone())],
        );
    }
    if let Some(ai) = &comparison.ai {
        let keys = ["prompt", "assistant", "position"];
        table(tables, "ai_gained", &keys, &ai.gained, lang, |cell| {
            vec![
                Cell::Text(cell.prompt.clone()),
                Cell::Text(engine_name(&cell.engine)),
                count_cell(cell.after, lang),
            ]
        });
        table(tables, "ai_lost", &keys, &ai.lost, lang, |cell| {
            vec![
                Cell::Text(cell.prompt.clone()),
                Cell::Text(engine_name(&cell.engine)),
                count_cell(cell.before, lang),
            ]
        });
    }
    widget
}
