//! The report's research parts: the domain's organic search and how its keywords moved, the
//! keyword lists, the link profile over time with the links won, lost and judged, the keyword
//! gap with the competitors, and what is worth doing next. Each reads what the modules already
//! keep; nothing is bought.

use serde_json::Value;

use super::texts::{self, number, text};
use super::{
    Cell, Chart, Table, Widget, change, count_cell, list, num, percent_change, short, tile, txt,
};

/// Rows of a report's keyword tables.
const ROWS: usize = 12;

/// Keyword lists shown at most.
pub(crate) const LISTS_SHOWN: usize = 3;

/// Months a link chart covers at most.
const MONTHS: usize = 24;

/// Periods of won and lost links listed.
const PERIODS: usize = 6;

/// Opportunities listed.
const OPPORTUNITIES: usize = 10;

fn dash() -> Cell {
    Cell::Text("—".to_owned())
}

fn optional(value: Option<f64>, decimals: usize, lang: &str) -> String {
    value.map_or_else(|| "—".to_owned(), |value| number(value, decimals, lang))
}

/// A keyword table from ranked keyword rows: keyword, position, previous, volume, traffic, page.
fn keyword_table(title: &str, rows: &[&Value], lang: &str) -> Table {
    Table {
        title: text(lang, title),
        columns: [
            "keyword", "position", "previous", "volume", "traffic", "page",
        ]
        .iter()
        .map(|key| text(lang, key))
        .collect(),
        rows: rows
            .iter()
            .take(ROWS)
            .map(|row| {
                vec![
                    Cell::Text(txt(row, "keyword").unwrap_or_default()),
                    count_cell(num(row, "position"), lang),
                    count_cell(num(row, "previous_position"), lang),
                    count_cell(num(row, "volume"), lang),
                    count_cell(num(row, "traffic"), lang),
                    Cell::Text(txt(row, "url").map(|url| short(&url)).unwrap_or_default()),
                ]
            })
            .collect(),
    }
}

/// The months of a kept domain history (`yyyy-mm`, organic traffic), oldest first; else the
/// days of the program's own metric points.
fn traffic_points(organic: &Value, lang: &str) -> Vec<(String, Option<f64>)> {
    let months = list(organic, "history");
    if months.len() > 1 {
        return months
            .iter()
            .map(|month| {
                let part = |key: &str| month.get(key).and_then(Value::as_u64).unwrap_or_default();
                let label = format!("{:02}.{}", part("month"), part("year"));
                (label, num(month, "organic.traffic"))
            })
            .collect();
    }
    list(organic, "points")
        .iter()
        .map(|point| {
            (
                texts::date(&txt(point, "day").unwrap_or_default(), lang),
                num(point, "metrics.organic.traffic"),
            )
        })
        .collect()
}

/// Organic search: the domain's numbers, their trend, its best keywords and how they moved.
#[allow(clippy::too_many_lines)] // tiles, the trend and five tables
pub(crate) fn organic(organic: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("organic", lang);
    let summary = organic.pointer("/summary/organic");
    let keywords: Vec<&Value> = organic
        .pointer("/keywords/keywords")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().collect())
        .unwrap_or_default();
    if summary.is_none() && keywords.is_empty() {
        return Widget::empty("organic", lang);
    }
    widget.note = txt(organic, "fetched_at").map(|at| texts::date(&at, lang));
    let points = traffic_points(organic, lang);
    let traffic_change = || {
        let values: Vec<f64> = points.iter().filter_map(|(_, value)| *value).collect();
        let (last, before) = (values.last()?, values.get(values.len().checked_sub(2)?)?);
        percent_change(*last, *before, lang)
    };
    if let Some(summary) = summary {
        let count = |key: &str| num(summary, key).unwrap_or_default();
        widget.tiles = vec![
            tile(
                "organic_traffic",
                number(count("traffic"), 0, lang),
                traffic_change(),
                lang,
            ),
            tile(
                "organic_keywords",
                number(count("keywords"), 0, lang),
                None,
                lang,
            ),
            tile(
                "traffic_value",
                format!("${}", number(count("traffic_value_usd"), 0, lang)),
                None,
                lang,
            ),
            tile(
                "kpi_top3",
                number(count("pos_1") + count("pos_2_3"), 0, lang),
                None,
                lang,
            ),
            tile(
                "kpi_top10",
                number(
                    count("pos_1") + count("pos_2_3") + count("pos_4_10"),
                    0,
                    lang,
                ),
                None,
                lang,
            ),
            tile("keywords_new", number(count("new"), 0, lang), None, lang),
            tile("keywords_up", number(count("up"), 0, lang), None, lang),
            tile("keywords_down", number(count("down"), 0, lang), None, lang),
            tile("keywords_lost", number(count("lost"), 0, lang), None, lang),
        ];
    }
    if points.len() > 1 {
        widget.chart = Some(Chart {
            title: text(lang, "organic_trend"),
            labels: points.iter().map(|(label, _)| label.clone()).collect(),
            series: vec![(
                text(lang, "organic_traffic"),
                points.iter().map(|(_, value)| *value).collect(),
                false,
            )],
        });
    }
    if !keywords.is_empty() {
        widget
            .tables
            .push(keyword_table("top_organic_keywords", &keywords, lang));
    }
    let moved = |kind: &str| -> Vec<&Value> {
        keywords
            .iter()
            .copied()
            .filter(|row| txt(row, "change").as_deref() == Some(kind))
            .collect()
    };
    let mut up = moved("up");
    up.sort_by(|left, right| {
        let gain = |row: &Value| {
            num(row, "previous_position").unwrap_or_default()
                - num(row, "position").unwrap_or_default()
        };
        gain(right).total_cmp(&gain(left))
    });
    let mut down = moved("down");
    down.sort_by(|left, right| {
        let loss = |row: &Value| {
            num(row, "position").unwrap_or_default()
                - num(row, "previous_position").unwrap_or_default()
        };
        loss(right).total_cmp(&loss(left))
    });
    for (title, rows) in [
        ("improved_keywords", up),
        ("declined_keywords", down),
        ("new_keywords", moved("new")),
    ] {
        if !rows.is_empty() {
            widget.tables.push(keyword_table(title, &rows, lang));
        }
    }
    let lost: Vec<&Value> = organic
        .pointer("/lost/keywords")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().collect())
        .unwrap_or_default();
    if !lost.is_empty() {
        widget.tables.push(Table {
            title: text(lang, "lost_keywords"),
            columns: ["keyword", "previous", "volume", "page"]
                .iter()
                .map(|key| text(lang, key))
                .collect(),
            rows: lost
                .iter()
                .take(ROWS)
                .map(|row| {
                    vec![
                        Cell::Text(txt(row, "keyword").unwrap_or_default()),
                        count_cell(
                            num(row, "previous_position").or_else(|| num(row, "position")),
                            lang,
                        ),
                        count_cell(num(row, "volume"), lang),
                        Cell::Text(txt(row, "url").map(|url| short(&url)).unwrap_or_default()),
                    ]
                })
                .collect(),
        });
    }
    widget
}

/// The months of a link profile (referring domains), oldest first: the kept history, else the
/// program's own metric points.
fn domain_points(links: &Value, lang: &str) -> Vec<(String, Option<f64>)> {
    let months = list(links, "history");
    if months.len() > 1 {
        let start = months.len().saturating_sub(MONTHS);
        return months[start..]
            .iter()
            .map(|month| {
                let label = txt(month, "month").unwrap_or_default();
                let label = match label.split_once('-') {
                    Some((year, month)) => format!("{month}.{year}"),
                    None => label,
                };
                (label, num(month, "referring_domains"))
            })
            .collect();
    }
    list(links, "points")
        .iter()
        .map(|point| {
            (
                texts::date(&txt(point, "day").unwrap_or_default(), lang),
                num(point, "metrics.referring_domains"),
            )
        })
        .collect()
}

/// Backlinks over time: authority and referring domains with their change, links won and lost
/// period by period, the riskiest linking sites with the decisions taken, and link work.
#[allow(clippy::too_many_lines)] // tiles, the trend and three tables
pub(crate) fn link_history(links: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("link_history", lang);
    let summary = links.get("summary").filter(|summary| summary.is_object());
    let periods = list(links, "new_lost");
    let risks = list(links, "audit");
    let decisions = list(links, "decisions");
    let prospects = list(links, "prospects");
    let points = domain_points(links, lang);
    if summary.is_none()
        && periods.is_empty()
        && risks.is_empty()
        && points.len() < 2
        && decisions.is_empty()
        && prospects.is_empty()
    {
        return Widget::empty("link_history", lang);
    }
    widget.note = txt(links, "fetched_at").map(|at| texts::date(&at, lang));
    let history = list(links, "history");
    let year_ago = |key: &str| -> Option<f64> {
        let index = history.len().checked_sub(13)?;
        num(history.get(index)?, key)
    };
    if let Some(summary) = summary {
        let domains = num(summary, "referring_domains").unwrap_or_default();
        widget.tiles.push(tile(
            "authority",
            optional(num(summary, "rank"), 0, lang),
            change(
                num(summary, "rank")
                    .zip(year_ago("rank"))
                    .map(|(now, before)| now - before),
                0,
                true,
                lang,
            ),
            lang,
        ));
        widget.tiles.push(tile(
            "referring_domains",
            number(domains, 0, lang),
            year_ago("referring_domains").and_then(|before| percent_change(domains, before, lang)),
            lang,
        ));
        widget.tiles.push(tile(
            "backlink_count",
            number(num(summary, "backlinks").unwrap_or_default(), 0, lang),
            None,
            lang,
        ));
    }
    if let Some(latest) = periods.last() {
        widget.tiles.push(tile(
            "domains_won",
            number(
                num(latest, "new_referring_domains").unwrap_or_default(),
                0,
                lang,
            ),
            None,
            lang,
        ));
        widget.tiles.push(tile(
            "domains_lost_short",
            number(
                num(latest, "lost_referring_domains").unwrap_or_default(),
                0,
                lang,
            ),
            None,
            lang,
        ));
    }
    let high = risks
        .iter()
        .filter(|risk| txt(risk, "risk").as_deref() == Some("high"))
        .count();
    if !risks.is_empty() {
        widget.tiles.push(tile(
            "risky_sites",
            format!(
                "{} / {}",
                number(f64::from(u32::try_from(high).unwrap_or(u32::MAX)), 0, lang),
                number(
                    f64::from(u32::try_from(risks.len()).unwrap_or(u32::MAX)),
                    0,
                    lang
                )
            ),
            None,
            lang,
        ));
    }
    let decided = |kind: &str| {
        decisions
            .iter()
            .filter(|decision| txt(decision, "decision").as_deref() == Some(kind))
            .count()
    };
    if decided("disavow") > 0 {
        widget.tiles.push(tile(
            "disavowed",
            decided("disavow").to_string(),
            None,
            lang,
        ));
    }
    if !prospects.is_empty() {
        let status = |kind: &str| {
            prospects
                .iter()
                .filter(|prospect| txt(prospect, "status").as_deref() == Some(kind))
                .count()
        };
        widget.tiles.push(tile(
            "prospects_open",
            format!(
                "{} ({} {})",
                status("prospect") + status("contacted"),
                status("contacted"),
                text(lang, "contacted")
            ),
            None,
            lang,
        ));
        widget
            .tiles
            .push(tile("prospects_won", status("won").to_string(), None, lang));
    }
    if points.len() > 1 {
        widget.chart = Some(Chart {
            title: text(lang, "referring_domains_trend"),
            labels: points.iter().map(|(label, _)| label.clone()).collect(),
            series: vec![(
                text(lang, "referring_domains"),
                points.iter().map(|(_, value)| *value).collect(),
                false,
            )],
        });
    }
    if !periods.is_empty() {
        let start = periods.len().saturating_sub(PERIODS);
        widget.tables.push(Table {
            title: text(lang, "links_by_period"),
            columns: [
                "period_end",
                "domains_won",
                "domains_lost_short",
                "links_won",
                "links_lost",
            ]
            .iter()
            .map(|key| text(lang, key))
            .collect(),
            rows: periods[start..]
                .iter()
                .rev()
                .map(|period| {
                    vec![
                        Cell::Text(texts::date(&txt(period, "date").unwrap_or_default(), lang)),
                        count_cell(num(period, "new_referring_domains"), lang),
                        count_cell(num(period, "lost_referring_domains"), lang),
                        count_cell(num(period, "new_backlinks"), lang),
                        count_cell(num(period, "lost_backlinks"), lang),
                    ]
                })
                .collect(),
        });
    }
    let mut risky: Vec<&Value> = risks
        .iter()
        .filter(|risk| txt(risk, "risk").as_deref() != Some("low"))
        .collect();
    risky.sort_by(|left, right| {
        num(right, "score")
            .unwrap_or_default()
            .total_cmp(&num(left, "score").unwrap_or_default())
    });
    if !risky.is_empty() {
        let decision_of = |domain: &str| {
            decisions
                .iter()
                .find(|decision| txt(decision, "domain").as_deref() == Some(domain))
                .and_then(|decision| txt(decision, "decision"))
        };
        widget.tables.push(Table {
            title: text(lang, "risky_links"),
            columns: ["domain", "spam_score", "risk", "decision"]
                .iter()
                .map(|key| text(lang, key))
                .collect(),
            rows: risky
                .iter()
                .take(ROWS)
                .map(|risk| {
                    let domain = txt(risk, "domain").unwrap_or_default();
                    vec![
                        Cell::Text(domain.clone()),
                        count_cell(num(risk, "spam_score"), lang),
                        Cell::Text(text(
                            lang,
                            &format!("risk_{}", txt(risk, "risk").unwrap_or_default()),
                        )),
                        Cell::Text(text(
                            lang,
                            &format!(
                                "decision_{}",
                                decision_of(&domain).unwrap_or_else(|| "none".to_owned())
                            ),
                        )),
                    ]
                })
                .collect(),
        });
    }
    widget
}

/// The keyword gap: how many keywords fall in each group, and the missing and weak ones worth
/// most.
pub(crate) fn gap(matrix: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("gap", lang);
    let keywords = list(matrix, "keywords");
    if keywords.is_empty() {
        return Widget::empty("gap", lang);
    }
    let competitors: Vec<String> = list(matrix, "competitors")
        .iter()
        .filter_map(|domain| domain.as_str().map(str::to_owned))
        .collect();
    widget.note = Some(texts::fill(
        lang,
        "gap_note",
        &[
            ("own", &txt(matrix, "own").unwrap_or_default()),
            ("competitors", &competitors.join(", ")),
        ],
    ));
    let counts = matrix.get("counts").unwrap_or(&Value::Null);
    widget.tiles = ["shared", "missing", "weak", "strong", "untapped", "unique"]
        .iter()
        .map(|group| {
            tile(
                &format!("gap_{group}"),
                number(num(counts, group).unwrap_or_default(), 0, lang),
                None,
                lang,
            )
        })
        .collect();
    let in_group = |group: &str| -> Vec<&Value> {
        keywords
            .iter()
            .filter(|row| {
                list(row, "groups")
                    .iter()
                    .any(|known| known.as_str() == Some(group))
            })
            .collect()
    };
    let best_rival = |row: &Value| -> Option<(String, f64)> {
        list(row, "rankings")
            .iter()
            .skip(1)
            .filter_map(|ranking| Some((txt(ranking, "domain")?, num(ranking, "position")?)))
            .min_by(|left, right| left.1.total_cmp(&right.1))
    };
    for (title, group) in [("gap_missing_table", "missing"), ("gap_weak_table", "weak")] {
        let rows = in_group(group);
        if rows.is_empty() {
            continue;
        }
        widget.tables.push(Table {
            title: text(lang, title),
            columns: ["keyword", "volume", "difficulty", "you", "best_rival"]
                .iter()
                .map(|key| text(lang, key))
                .collect(),
            rows: rows
                .iter()
                .take(ROWS)
                .map(|row| {
                    let own = list(row, "rankings")
                        .first()
                        .and_then(|ranking| num(ranking, "position"));
                    vec![
                        Cell::Text(txt(row, "keyword").unwrap_or_default()),
                        count_cell(num(row, "volume"), lang),
                        count_cell(num(row, "difficulty"), lang),
                        own.map_or_else(dash, |position| count_cell(Some(position), lang)),
                        best_rival(row).map_or_else(dash, |(domain, position)| {
                            Cell::Text(format!("{domain} · {}", number(position, 0, lang)))
                        }),
                    ]
                })
                .collect(),
        });
    }
    widget
}

/// The project's keyword lists: their sizes and each list's most searched keywords.
pub(crate) fn keyword_lists(lists: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("keyword_lists", lang);
    let lists: Vec<&Value> = lists
        .as_array()
        .map(|lists| {
            lists
                .iter()
                .filter(|entry| !list(entry, "keywords").is_empty())
                .collect()
        })
        .unwrap_or_default();
    if lists.is_empty() {
        return Widget::empty("keyword_lists", lang);
    }
    let volume = |row: &Value| num(row, "data.volume").unwrap_or_default();
    let keywords: usize = lists
        .iter()
        .map(|entry| list(entry, "keywords").len())
        .sum();
    let total: f64 = lists
        .iter()
        .flat_map(|entry| list(entry, "keywords"))
        .map(volume)
        .sum();
    widget.tiles = vec![
        tile("lists", lists.len().to_string(), None, lang),
        tile("listed_keywords", keywords.to_string(), None, lang),
        tile("total_volume", number(total, 0, lang), None, lang),
    ];
    for entry in lists.iter().take(LISTS_SHOWN) {
        let mut rows: Vec<&Value> = list(entry, "keywords").iter().collect();
        rows.sort_by(|left, right| volume(right).total_cmp(&volume(left)));
        widget.tables.push(Table {
            title: format!(
                "{} · {}",
                txt(entry, "name").unwrap_or_default(),
                texts::fill(
                    lang,
                    "list_size",
                    &[("n", &list(entry, "keywords").len().to_string())]
                )
            ),
            columns: ["keyword", "volume", "difficulty", "cpc", "intent"]
                .iter()
                .map(|key| text(lang, key))
                .collect(),
            rows: rows
                .iter()
                .take(ROWS)
                .map(|row| {
                    let data = row.get("data").unwrap_or(&Value::Null);
                    vec![
                        Cell::Text(txt(row, "keyword").unwrap_or_default()),
                        count_cell(num(data, "volume"), lang),
                        count_cell(num(data, "difficulty"), lang),
                        num(data, "cpc").map_or_else(dash, |cpc| Cell::Number {
                            value: cpc,
                            text: format!("${}", number(cpc, 2, lang)),
                        }),
                        Cell::Text(
                            txt(data, "intent")
                                .map(|intent| text(lang, &format!("intent_{intent}")))
                                .unwrap_or_default(),
                        ),
                    ]
                })
                .collect(),
        });
    }
    widget
}

/// What an opportunity is about, short enough for a table.
fn subject(item: &Value, lang: &str) -> String {
    let detail = item.get("detail").unwrap_or(&Value::Null);
    let count = || {
        num(detail, "count")
            .map(|count| texts::fill(lang, "pages_count", &[("n", &number(count, 0, lang))]))
    };
    let subject = txt(item, "subject").unwrap_or_default();
    match txt(item, "rule").as_deref() {
        Some("audit_issue") => txt(detail, "title").unwrap_or(subject),
        Some("orphans" | "weak_links" | "deep_pages") => count().unwrap_or(subject),
        Some("lost_links") => num(detail, "count")
            .map(|count| texts::fill(lang, "sites_count", &[("n", &number(count, 0, lang))]))
            .unwrap_or(subject),
        Some("vitals") => txt(detail, "metrics").unwrap_or(subject),
        _ if subject.contains("://") => short(&subject),
        _ => subject,
    }
}

/// What to do next: the open opportunities worth most for their work.
pub(crate) fn opportunities(found: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("opportunities", lang);
    let items: Vec<&Value> = list(found, "items")
        .iter()
        .filter(|item| txt(item, "state").as_deref() == Some("open"))
        .collect();
    if items.is_empty() {
        return Widget::empty("opportunities", lang);
    }
    widget.tiles = vec![
        tile(
            "opportunities_open",
            number(num(found, "open").unwrap_or_default(), 0, lang),
            None,
            lang,
        ),
        tile(
            "opportunities_gain",
            number(num(found, "gain").unwrap_or_default(), 0, lang),
            None,
            lang,
        ),
    ];
    widget.tables.push(Table {
        title: text(lang, "opportunities_top"),
        columns: ["what", "subject", "gain", "effort"]
            .iter()
            .map(|key| text(lang, key))
            .collect(),
        rows: items
            .iter()
            .take(OPPORTUNITIES)
            .map(|item| {
                vec![
                    Cell::Text(text(
                        lang,
                        &format!("opp_{}", txt(item, "rule").unwrap_or_default()),
                    )),
                    Cell::Text(subject(item, lang)),
                    num(item, "gain")
                        .filter(|gain| *gain > 0.0)
                        .map_or_else(dash, |gain| count_cell(Some(gain.round()), lang)),
                    Cell::Text(text(
                        lang,
                        &format!("effort_{}", txt(item, "effort").unwrap_or_default()),
                    )),
                ]
            })
            .collect(),
    });
    widget
}
