//! A report as it is drawn: the parts the user chose, each made from the modules' own reports
//! (rank tracking, Search Console, the site audit, AI visibility, kept domain and backlink
//! answers) into tiles, a chart, bars and tables, in the report's language. [`html`] writes it
//! as one self-contained page (shown, shared and printed to PDF), [`xlsx`] its tables as a
//! workbook. Nothing is asked of a paid source: a report shows what the modules already have.

pub(crate) mod html;
pub(crate) mod texts;
pub(crate) mod xlsx;

use octopuson_platform::{Branding, ProjectRow, ReportRow};
use serde::Serialize;
use serde_json::Value;

use texts::{number, percent, text};

/// The modules' reports a report is drawn from; `None` where a module has nothing yet.
#[derive(Debug, Clone, Default)]
pub(crate) struct Sources {
    pub(crate) rank: Option<Value>,
    pub(crate) gsc: Option<Value>,
    pub(crate) audit: Option<Value>,
    pub(crate) ai: Option<Value>,
    /// A kept domain overview (`Fetched`).
    pub(crate) overview: Option<Value>,
    /// A kept backlink summary (`Fetched`).
    pub(crate) backlinks: Option<Value>,
}

/// A change with whether it is good news.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Change {
    pub(crate) text: String,
    pub(crate) good: bool,
}

/// A headline number.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Tile {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) change: Option<Change>,
}

/// Lines over days.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Chart {
    pub(crate) title: String,
    /// Day labels, oldest first.
    pub(crate) labels: Vec<String>,
    /// Name, values (one per label), drawn dashed.
    pub(crate) series: Vec<(String, Vec<Option<f64>>, bool)>,
}

/// A horizontal bar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Bar {
    pub(crate) label: String,
    /// Its length against the longest.
    pub(crate) value: f64,
    pub(crate) text: String,
    /// The project's own.
    pub(crate) own: bool,
}

/// A cell: text, or a number with how it is written.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum Cell {
    Text(String),
    Number { value: f64, text: String },
}

/// A table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Table {
    pub(crate) title: String,
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<Cell>>,
}

/// One part of a report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Widget {
    pub(crate) id: String,
    pub(crate) title: String,
    /// A line under the title (the period a module covers).
    pub(crate) note: Option<String>,
    pub(crate) tiles: Vec<Tile>,
    pub(crate) chart: Option<Chart>,
    pub(crate) bars_title: Option<String>,
    pub(crate) bars: Vec<Bar>,
    /// The value of a full bar; the longest bar's when `None`.
    pub(crate) bars_scale: Option<f64>,
    pub(crate) tables: Vec<Table>,
    /// The module has nothing to show yet.
    pub(crate) empty: bool,
}

impl Widget {
    fn new(id: &str, lang: &str) -> Self {
        Self {
            id: id.to_owned(),
            title: text(lang, id),
            note: None,
            tiles: Vec::new(),
            chart: None,
            bars_title: None,
            bars: Vec::new(),
            bars_scale: None,
            tables: Vec::new(),
            empty: false,
        }
    }

    fn empty(id: &str, lang: &str) -> Self {
        Self {
            empty: true,
            ..Self::new(id, lang)
        }
    }
}

/// A report ready to be written.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ReportView {
    pub(crate) name: String,
    pub(crate) language: String,
    pub(crate) project: String,
    pub(crate) domain: String,
    /// First and last day, `YYYY-MM-DD`.
    pub(crate) period: (String, String),
    /// `YYYY-MM-DD`.
    pub(crate) generated: String,
    #[serde(skip)]
    pub(crate) branding: Branding,
    pub(crate) widgets: Vec<Widget>,
}

/// A number at `path` (`a.b.c`).
fn num(value: &Value, path: &str) -> Option<f64> {
    path.split('.')
        .try_fold(value, |value, key| value.get(key))
        .and_then(Value::as_f64)
}

fn txt(value: &Value, path: &str) -> Option<String> {
    path.split('.')
        .try_fold(value, |value, key| value.get(key))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn count_cell(value: Option<f64>, lang: &str) -> Cell {
    value.map_or_else(
        || Cell::Text("—".to_owned()),
        |value| Cell::Number {
            value,
            text: number(value, 0, lang),
        },
    )
}

fn decimal_cell(value: Option<f64>, lang: &str) -> Cell {
    value.map_or_else(
        || Cell::Text("—".to_owned()),
        |value| Cell::Number {
            value,
            text: number(value, 1, lang),
        },
    )
}

/// A signed change; `higher_is_better` decides its colour.
fn change(
    value: Option<f64>,
    decimals: usize,
    higher_is_better: bool,
    lang: &str,
) -> Option<Change> {
    let value = value.filter(|value| value.is_finite() && value.abs() >= 0.05)?;
    let sign = if value > 0.0 { "+" } else { "" };
    Some(Change {
        text: format!("{sign}{}", number(value, decimals, lang)),
        good: (value > 0.0) == higher_is_better,
    })
}

/// A change in percent between two totals.
fn percent_change(now: f64, before: f64, lang: &str) -> Option<Change> {
    if before <= 0.0 {
        return None;
    }
    let value = (now - before) / before * 100.0;
    let sign = if value > 0.0 { "+" } else { "" };
    (value.abs() >= 0.05).then(|| Change {
        text: format!("{sign}{}", percent(value, lang)),
        good: value > 0.0,
    })
}

fn tile(label: &str, value: String, change: Option<Change>, lang: &str) -> Tile {
    Tile {
        label: text(lang, label),
        value,
        change,
    }
}

/// The path of an address, short enough for a table.
fn short(address: &str) -> String {
    let path = address.split_once("://").map_or(address, |(_, rest)| {
        rest.find('/').map_or("/", |at| &rest[at..])
    });
    if path.chars().count() > 60 {
        format!("{}…", path.chars().take(59).collect::<String>())
    } else {
        path.to_owned()
    }
}

#[allow(clippy::too_many_lines)] // headline numbers, the trend and the keyword table
fn rankings(rank: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("rankings", lang);
    let summary = rank.get("summary").unwrap_or(&Value::Null);
    if num(summary, "checked").unwrap_or_default() == 0.0 {
        return Widget::empty("rankings", lang);
    }
    widget.tiles = vec![
        tile(
            "visibility",
            num(summary, "visibility").map_or_else(|| "—".to_owned(), |value| percent(value, lang)),
            change(num(summary, "visibility_change"), 1, true, lang),
            lang,
        ),
        tile(
            "average_position",
            num(summary, "average_position")
                .map_or_else(|| "—".to_owned(), |value| number(value, 1, lang)),
            None,
            lang,
        ),
        tile(
            "ranked",
            format!(
                "{} / {}",
                number(num(summary, "ranked").unwrap_or_default(), 0, lang),
                number(num(summary, "checked").unwrap_or_default(), 0, lang)
            ),
            None,
            lang,
        ),
        tile(
            "top3_top10",
            format!(
                "{} / {}",
                number(num(summary, "top3").unwrap_or_default(), 0, lang),
                number(num(summary, "top10").unwrap_or_default(), 0, lang)
            ),
            None,
            lang,
        ),
        tile(
            "improved",
            number(num(summary, "improved").unwrap_or_default(), 0, lang),
            None,
            lang,
        ),
        tile(
            "declined",
            number(num(summary, "declined").unwrap_or_default(), 0, lang),
            None,
            lang,
        ),
    ];
    let points = list(rank, "visibility");
    if points.len() > 1 {
        widget.chart = Some(Chart {
            title: text(lang, "visibility_trend"),
            labels: points
                .iter()
                .map(|point| texts::date(&txt(point, "date").unwrap_or_default(), lang))
                .collect(),
            series: vec![(
                text(lang, "visibility"),
                points
                    .iter()
                    .map(|point| num(point, "visibility"))
                    .collect(),
                false,
            )],
        });
    }
    let mut rows: Vec<&Value> = list(rank, "rows").iter().collect();
    rows.sort_by(|left, right| {
        num(right, "volume")
            .unwrap_or_default()
            .total_cmp(&num(left, "volume").unwrap_or_default())
            .then_with(|| {
                num(left, "position")
                    .unwrap_or(1_000.0)
                    .total_cmp(&num(right, "position").unwrap_or(1_000.0))
            })
    });
    widget.tables.push(Table {
        title: text(lang, "top_keywords"),
        columns: ["keyword", "position", "change", "best", "volume", "page"]
            .iter()
            .map(|key| text(lang, key))
            .collect(),
        rows: rows
            .iter()
            .take(25)
            .map(|row| {
                vec![
                    Cell::Text(txt(row, "keyword").unwrap_or_default()),
                    count_cell(num(row, "position"), lang),
                    num(row, "change").map_or_else(
                        || Cell::Text(String::new()),
                        |value| Cell::Number {
                            value,
                            text: if value > 0.0 {
                                format!("+{}", number(value, 0, lang))
                            } else {
                                number(value, 0, lang)
                            },
                        },
                    ),
                    count_cell(num(row, "best"), lang),
                    count_cell(num(row, "volume"), lang),
                    Cell::Text(txt(row, "url").map(|url| short(&url)).unwrap_or_default()),
                ]
            })
            .collect(),
    });
    widget
}

fn competitors(rank: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("competitors", lang);
    let shares = list(rank, "competitors");
    if shares.is_empty() {
        return Widget::empty("competitors", lang);
    }
    widget.bars_title = Some(text(lang, "visibility"));
    widget.bars = shares
        .iter()
        .map(|share| {
            let visibility = num(share, "visibility").unwrap_or_default();
            Bar {
                label: txt(share, "domain").unwrap_or_default(),
                value: visibility,
                text: percent(visibility, lang),
                own: share.get("own").and_then(Value::as_bool).unwrap_or(false),
            }
        })
        .collect();
    widget.tables.push(Table {
        title: String::new(),
        columns: ["domain", "visibility", "keywords", "average_position"]
            .iter()
            .map(|key| text(lang, key))
            .collect(),
        rows: shares
            .iter()
            .map(|share| {
                vec![
                    Cell::Text(txt(share, "domain").unwrap_or_default()),
                    decimal_cell(num(share, "visibility"), lang),
                    count_cell(num(share, "keywords"), lang),
                    decimal_cell(num(share, "average_position"), lang),
                ]
            })
            .collect(),
    });
    widget
}

#[allow(clippy::too_many_lines)] // totals, the daily chart and two tables
fn search_console(gsc: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("search_console", lang);
    let (now, before) = (
        gsc.get("now").unwrap_or(&Value::Null),
        gsc.get("before").unwrap_or(&Value::Null),
    );
    let period = gsc.get("period").and_then(Value::as_array);
    widget.note = period.map(|period| {
        format!(
            "{} – {} · {}",
            texts::date(
                period.first().and_then(Value::as_str).unwrap_or_default(),
                lang
            ),
            texts::date(
                period.get(1).and_then(Value::as_str).unwrap_or_default(),
                lang
            ),
            text(lang, "vs_before")
        )
    });
    let clicks = num(now, "clicks").unwrap_or_default();
    let impressions = num(now, "impressions").unwrap_or_default();
    widget.tiles = vec![
        tile(
            "clicks",
            number(clicks, 0, lang),
            percent_change(clicks, num(before, "clicks").unwrap_or_default(), lang),
            lang,
        ),
        tile(
            "impressions",
            number(impressions, 0, lang),
            percent_change(
                impressions,
                num(before, "impressions").unwrap_or_default(),
                lang,
            ),
            lang,
        ),
        tile(
            "ctr",
            percent(num(now, "ctr").unwrap_or_default(), lang),
            change(
                num(now, "ctr")
                    .zip(num(before, "ctr"))
                    .map(|(now, before)| now - before),
                1,
                true,
                lang,
            ),
            lang,
        ),
        tile(
            "average_position",
            num(now, "position").map_or_else(|| "—".to_owned(), |value| number(value, 1, lang)),
            change(
                num(now, "position")
                    .zip(num(before, "position"))
                    .map(|(now, before)| now - before),
                1,
                false,
                lang,
            ),
            lang,
        ),
    ];
    if let Some(traffic) = gsc.get("traffic_now").filter(|traffic| !traffic.is_null()) {
        let sessions = num(traffic, "sessions").unwrap_or_default();
        let before_sessions = gsc
            .get("traffic_before")
            .and_then(|before| num(before, "sessions"))
            .unwrap_or_default();
        widget.tiles.push(tile(
            "organic_sessions",
            number(sessions, 0, lang),
            percent_change(sessions, before_sessions, lang),
            lang,
        ));
        widget.tiles.push(tile(
            "key_events",
            number(num(traffic, "key_events").unwrap_or_default(), 0, lang),
            None,
            lang,
        ));
    }
    let days = list(gsc, "daily");
    let current: Vec<&Value> = days
        .iter()
        .filter(|day| day.get("period").and_then(Value::as_str) == Some("now"))
        .collect();
    let earlier: Vec<&Value> = days
        .iter()
        .filter(|day| day.get("period").and_then(Value::as_str) == Some("before"))
        .collect();
    if current.len() > 1 {
        let offset = earlier.len().saturating_sub(current.len());
        widget.chart = Some(Chart {
            title: text(lang, "clicks_daily"),
            labels: current
                .iter()
                .map(|day| texts::date(&txt(day, "date").unwrap_or_default(), lang))
                .collect(),
            series: vec![
                (
                    text(lang, "clicks"),
                    current.iter().map(|day| num(day, "clicks")).collect(),
                    false,
                ),
                (
                    text(lang, "vs_before"),
                    (0..current.len())
                        .map(|index| {
                            earlier
                                .get(index + offset)
                                .and_then(|day| num(day, "clicks"))
                        })
                        .collect(),
                    true,
                ),
            ],
        });
    }
    widget.tables.push(Table {
        title: text(lang, "top_queries"),
        columns: ["query", "clicks", "impressions", "ctr", "position"]
            .iter()
            .map(|key| text(lang, key))
            .collect(),
        rows: list(gsc, "queries")
            .iter()
            .take(15)
            .map(|row| {
                vec![
                    Cell::Text(txt(row, "query").unwrap_or_default()),
                    count_cell(num(row, "clicks"), lang),
                    count_cell(num(row, "impressions"), lang),
                    num(row, "ctr").map_or(Cell::Text("—".to_owned()), |value| Cell::Number {
                        value,
                        text: percent(value, lang),
                    }),
                    decimal_cell(num(row, "position"), lang),
                ]
            })
            .collect(),
    });
    widget.tables.push(Table {
        title: text(lang, "top_pages"),
        columns: [
            "page",
            "clicks",
            "impressions",
            "position",
            "organic_sessions",
        ]
        .iter()
        .map(|key| text(lang, key))
        .collect(),
        rows: list(gsc, "pages")
            .iter()
            .take(15)
            .map(|row| {
                vec![
                    Cell::Text(
                        txt(row, "page")
                            .map(|page| short(&page))
                            .unwrap_or_default(),
                    ),
                    count_cell(num(row, "clicks"), lang),
                    count_cell(num(row, "impressions"), lang),
                    decimal_cell(num(row, "position"), lang),
                    count_cell(num(row, "sessions"), lang),
                ]
            })
            .collect(),
    });
    widget
}

fn site_audit(audit: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("site_audit", lang);
    let Some(row) = audit.get("audit").filter(|row| !row.is_null()) else {
        return Widget::empty("site_audit", lang);
    };
    let previous = audit.get("previous").filter(|row| !row.is_null());
    let score = |row: &Value, key: &str| num(row, key);
    widget.note = txt(row, "finished_at")
        .or_else(|| txt(row, "started_at"))
        .map(|at| texts::date(&at, lang));
    widget.tiles = vec![
        tile(
            "health",
            score(row, "health").map_or_else(|| "—".to_owned(), |value| number(value, 1, lang)),
            change(
                score(row, "health")
                    .zip(previous.and_then(|before| score(before, "health")))
                    .map(|(now, before)| now - before),
                1,
                true,
                lang,
            ),
            lang,
        ),
        tile(
            "geo_readiness",
            score(row, "geo").map_or_else(|| "—".to_owned(), |value| number(value, 1, lang)),
            change(
                score(row, "geo")
                    .zip(previous.and_then(|before| score(before, "geo")))
                    .map(|(now, before)| now - before),
                1,
                true,
                lang,
            ),
            lang,
        ),
        tile(
            "pages_crawled",
            number(num(row, "pages").unwrap_or_default(), 0, lang),
            None,
            lang,
        ),
        tile(
            "findings",
            number(num(row, "issues").unwrap_or_default(), 0, lang),
            None,
            lang,
        ),
    ];
    let mut issues: Vec<(String, f64, String)> = list(audit, "categories")
        .iter()
        .flat_map(|category| list(category, "issues"))
        .filter(|issue| issue.get("kind").and_then(Value::as_str) == Some("issue"))
        .filter_map(|issue| {
            let count = num(issue, "count").filter(|count| *count > 0.0)?;
            Some((
                txt(issue, "title").unwrap_or_default(),
                count,
                txt(issue, "priority").unwrap_or_default(),
            ))
        })
        .collect();
    let weight = |priority: &str| match priority {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };
    issues.sort_by(|left, right| {
        weight(&left.2)
            .cmp(&weight(&right.2))
            .then(right.1.total_cmp(&left.1))
    });
    if !issues.is_empty() {
        widget.tables.push(Table {
            title: text(lang, "top_issues"),
            columns: vec![text(lang, "issue"), text(lang, "count")],
            rows: issues
                .into_iter()
                .take(12)
                .map(|(title, count, _)| vec![Cell::Text(title), count_cell(Some(count), lang)])
                .collect(),
        });
    }
    widget
}

fn ai_visibility(ai: &Value, lang: &str) -> Widget {
    let mut widget = Widget::new("ai_visibility", lang);
    let summary = ai.get("summary").unwrap_or(&Value::Null);
    if num(summary, "answers").unwrap_or_default() == 0.0 {
        return Widget::empty("ai_visibility", lang);
    }
    let history = list(ai, "history");
    let first = history.first().and_then(|point| num(point, "mention_rate"));
    let last = history.last().and_then(|point| num(point, "mention_rate"));
    widget.tiles = vec![
        tile(
            "mention_rate",
            percent(num(summary, "mention_rate").unwrap_or_default(), lang),
            if history.len() > 1 {
                change(
                    last.zip(first).map(|(last, first)| last - first),
                    1,
                    true,
                    lang,
                )
            } else {
                None
            },
            lang,
        ),
        tile(
            "citation_rate",
            percent(num(summary, "citation_rate").unwrap_or_default(), lang),
            None,
            lang,
        ),
        tile(
            "share_of_voice",
            percent(num(summary, "share_of_voice").unwrap_or_default(), lang),
            None,
            lang,
        ),
        tile(
            "average_position",
            num(summary, "average_position")
                .map_or_else(|| "—".to_owned(), |value| number(value, 1, lang)),
            None,
            lang,
        ),
    ];
    widget.bars_title = Some(text(lang, "by_assistant"));
    widget.bars_scale = Some(100.0);
    widget.bars = list(ai, "engines")
        .iter()
        .filter(|engine| num(engine, "answers").unwrap_or_default() > 0.0)
        .map(|engine| {
            let rate = num(engine, "mention_rate").unwrap_or_default();
            Bar {
                label: engine_name(&txt(engine, "engine").unwrap_or_default()),
                value: rate,
                text: percent(rate, lang),
                own: false,
            }
        })
        .collect();
    widget.tables.push(Table {
        title: text(lang, "brands"),
        columns: vec![
            text(lang, "domain"),
            text(lang, "share"),
            text(lang, "count"),
        ],
        rows: list(ai, "brands")
            .iter()
            .map(|brand| {
                vec![
                    Cell::Text(txt(brand, "key").unwrap_or_default()),
                    num(brand, "share").map_or(Cell::Text("—".to_owned()), |value| {
                        Cell::Number {
                            value,
                            text: percent(value, lang),
                        }
                    }),
                    count_cell(num(brand, "mentions"), lang),
                ]
            })
            .collect(),
    });
    widget
}

fn engine_name(engine: &str) -> String {
    match engine {
        "chatgpt" => "ChatGPT",
        "claude" => "Claude",
        "gemini" => "Gemini",
        "perplexity" => "Perplexity",
        "google_ai_mode" => "Google AI Mode",
        other => other,
    }
    .to_owned()
}

fn backlinks(sources: &Sources, lang: &str) -> Widget {
    let mut widget = Widget::new("backlinks", lang);
    let summary = sources.backlinks.as_ref().and_then(|kept| kept.get("data"));
    let overview = sources.overview.as_ref().and_then(|kept| kept.get("data"));
    if summary.is_none() && overview.is_none() {
        return Widget::empty("backlinks", lang);
    }
    if let Some(summary) = summary {
        widget.note = sources
            .backlinks
            .as_ref()
            .and_then(|kept| txt(kept, "fetched_at"))
            .map(|at| texts::date(&at, lang));
        widget.tiles.extend([
            tile(
                "authority",
                num(summary, "rank").map_or_else(|| "—".to_owned(), |value| number(value, 0, lang)),
                None,
                lang,
            ),
            tile(
                "referring_domains",
                number(
                    num(summary, "referring_domains").unwrap_or_default(),
                    0,
                    lang,
                ),
                None,
                lang,
            ),
            tile(
                "backlink_count",
                number(num(summary, "backlinks").unwrap_or_default(), 0, lang),
                None,
                lang,
            ),
            tile(
                "broken_backlinks",
                number(
                    num(summary, "broken_backlinks").unwrap_or_default(),
                    0,
                    lang,
                ),
                None,
                lang,
            ),
        ]);
    }
    if let Some(overview) = overview {
        widget.tiles.extend([
            tile(
                "organic_traffic",
                number(
                    num(overview, "organic.traffic").unwrap_or_default(),
                    0,
                    lang,
                ),
                None,
                lang,
            ),
            tile(
                "organic_keywords",
                number(
                    num(overview, "organic.keywords").unwrap_or_default(),
                    0,
                    lang,
                ),
                None,
                lang,
            ),
        ]);
    }
    widget
}

/// The headline numbers of every module that has them.
fn summary(sources: &Sources, lang: &str) -> Widget {
    let mut widget = Widget::new("summary", lang);
    let mut tiles = Vec::new();
    for part in [
        sources.rank.as_ref().map(|rank| rankings(rank, lang)),
        sources.gsc.as_ref().map(|gsc| search_console(gsc, lang)),
        sources.audit.as_ref().map(|audit| site_audit(audit, lang)),
        sources.ai.as_ref().map(|ai| ai_visibility(ai, lang)),
    ]
    .into_iter()
    .flatten()
    {
        tiles.extend(part.tiles.into_iter().take(2));
    }
    if tiles.is_empty() {
        return Widget::empty("summary", lang);
    }
    widget.tiles = tiles;
    widget
}

/// The report's parts from the modules' data.
pub(crate) fn build(
    report: &ReportRow,
    project: &ProjectRow,
    branding: Branding,
    sources: &Sources,
    (period, generated): ((String, String), String),
) -> ReportView {
    let lang = report.spec.language.as_str();
    let widgets = report
        .spec
        .widgets
        .iter()
        .map(|id| match (id.as_str(), sources) {
            ("summary", _) => summary(sources, lang),
            (
                "rankings",
                Sources {
                    rank: Some(rank), ..
                },
            ) => rankings(rank, lang),
            (
                "competitors",
                Sources {
                    rank: Some(rank), ..
                },
            ) => competitors(rank, lang),
            ("search_console", Sources { gsc: Some(gsc), .. }) => search_console(gsc, lang),
            (
                "site_audit",
                Sources {
                    audit: Some(audit), ..
                },
            ) => site_audit(audit, lang),
            ("ai_visibility", Sources { ai: Some(ai), .. }) => ai_visibility(ai, lang),
            ("backlinks", _) => backlinks(sources, lang),
            (other, _) => Widget::empty(other, lang),
        })
        .collect();
    ReportView {
        name: report.spec.name.clone(),
        language: report.spec.language.clone(),
        project: project.name.clone(),
        domain: project.domain.clone(),
        period,
        generated,
        branding,
        widgets,
    }
}

#[cfg(test)]
mod tests;
