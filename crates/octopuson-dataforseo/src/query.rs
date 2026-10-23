//! Filters, order and paging of list reports, asked of DataForSEO so they reach every row it
//! holds, not only the rows already fetched.
//!
//! A [`ListQuery`] names fields by the platform's own names (`volume`, `kd`, `position` …); each
//! report has a field list ([`FieldSpec`]) mapping them to DataForSEO's paths and saying which
//! operators they take. [`apply`] checks the query and writes it into the task. Filters are
//! written in a fixed order, so the same query always makes the same task (and cache key), and an
//! empty query leaves the task as it was.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::DfsError;

/// Most filters DataForSEO takes in one task.
pub const MAX_FILTERS: usize = 8;

/// Most sorting rules DataForSEO takes in one task.
pub const MAX_ORDER: usize = 3;

/// Farthest row a list can start at.
pub const MAX_OFFSET: u32 = 100_000;

/// How a filter compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// At most.
    Le,
    /// More than.
    Gt,
    /// At least.
    Ge,
    /// One of a list.
    In,
    /// None of a list.
    NotIn,
    /// Holds the text (any case).
    Contains,
    /// Does not hold the text (any case).
    NotContains,
    /// Starts with the text (any case).
    StartsWith,
    /// Matches a regular expression (RE2).
    Regex,
    /// Does not match a regular expression.
    NotRegex,
    /// A list field holds the value.
    Has,
    /// A list field does not hold the value.
    HasNot,
}

impl Op {
    /// DataForSEO's operator.
    fn word(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ne => "<>",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::In => "in",
            Self::NotIn => "not_in",
            Self::Contains | Self::StartsWith => "ilike",
            Self::NotContains => "not_ilike",
            Self::Regex => "regex",
            Self::NotRegex => "not_regex",
            Self::Has => "has",
            Self::HasNot => "has_not",
        }
    }
}

/// What a field holds, which decides the operators it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// A number.
    Number,
    /// Text.
    Text,
    /// True or false.
    Flag,
    /// A list of words (SERP features).
    List,
}

impl FieldKind {
    fn takes(self, op: Op) -> bool {
        match self {
            Self::Number => matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge),
            Self::Text => matches!(
                op,
                Op::Eq
                    | Op::Ne
                    | Op::In
                    | Op::NotIn
                    | Op::Contains
                    | Op::NotContains
                    | Op::StartsWith
                    | Op::Regex
                    | Op::NotRegex
            ),
            Self::Flag => matches!(op, Op::Eq),
            Self::List => matches!(op, Op::Has | Op::HasNot),
        }
    }
}

/// A field of a report: the platform's name and DataForSEO's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    /// The platform's name (`volume`).
    pub name: &'static str,
    /// DataForSEO's path (`keyword_info.search_volume`).
    pub path: &'static str,
    /// What it holds.
    pub kind: FieldKind,
}

const fn field(name: &'static str, path: &'static str, kind: FieldKind) -> FieldSpec {
    FieldSpec { name, path, kind }
}

/// One filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// The field's platform name.
    pub field: String,
    /// How it compares.
    pub op: Op,
    /// What it compares with: a number, text, a flag, or a list for `in`/`not_in`.
    pub value: Value,
}

/// Which way a sort goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Smallest first.
    Asc,
    /// Largest first.
    Desc,
}

/// One sorting rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// The field's platform name.
    pub field: String,
    /// Which way.
    pub direction: Direction,
}

/// Filters, order and the first row of a list report.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListQuery {
    /// All of them must hold.
    #[serde(default)]
    pub filters: Vec<Filter>,
    /// Sorting rules, first first; the report's own order when empty.
    #[serde(default)]
    pub order: Vec<Order>,
    /// Rows to skip.
    #[serde(default)]
    pub offset: u32,
}

impl ListQuery {
    /// Whether it changes nothing.
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty() && self.order.is_empty() && self.offset == 0
    }
}

fn invalid(message: String) -> DfsError {
    DfsError::Config(message)
}

/// A filter as DataForSEO takes it, checked against its field.
fn filter_of(filter: &Filter, fields: &[FieldSpec]) -> Result<Value, DfsError> {
    let spec = fields
        .iter()
        .find(|spec| spec.name == filter.field)
        .ok_or_else(|| invalid(format!("filters: unknown field `{}`", filter.field)))?;
    if !spec.kind.takes(filter.op) {
        return Err(invalid(format!(
            "filters: `{}` does not take `{:?}`",
            filter.field, filter.op
        )));
    }
    let value = match (spec.kind, filter.op, &filter.value) {
        (FieldKind::Number, _, Value::Number(_))
        | (FieldKind::Flag, _, Value::Bool(_))
        | (FieldKind::List, _, Value::String(_)) => filter.value.clone(),
        (FieldKind::Text, Op::In | Op::NotIn, Value::Array(items))
            if !items.is_empty() && items.iter().all(Value::is_string) =>
        {
            filter.value.clone()
        }
        (FieldKind::Text, Op::Contains | Op::NotContains, Value::String(text))
            if !text.trim().is_empty() =>
        {
            json!(format!("%{}%", text.trim()))
        }
        (FieldKind::Text, Op::StartsWith, Value::String(text)) if !text.trim().is_empty() => {
            json!(format!("{}%", text.trim()))
        }
        (FieldKind::Text, Op::Eq | Op::Ne | Op::Regex | Op::NotRegex, Value::String(text))
            if !text.is_empty() =>
        {
            filter.value.clone()
        }
        _ => {
            return Err(invalid(format!(
                "filters: `{}` needs another kind of value",
                filter.field
            )));
        }
    };
    Ok(json!([spec.path, filter.op.word(), value]))
}

/// Writes `query` into `task`: its filters (beside the task's own, all of which must hold), its
/// order in place of the report's, and its offset.
///
/// # Errors
/// [`DfsError::Config`] for an unknown field, an operator the field does not take, a value of
/// the wrong kind, or too many filters or sorting rules.
pub fn apply(task: &mut Value, query: &ListQuery, fields: &[FieldSpec]) -> Result<(), DfsError> {
    if query.is_empty() {
        return Ok(());
    }
    let mut filters: Vec<Value> = Vec::new();
    for filter in &query.filters {
        filters.push(filter_of(filter, fields)?);
    }
    // The same filters in any order make the same task.
    filters.sort_by_key(Value::to_string);
    filters.dedup();
    if !filters.is_empty() {
        let mut joined: Vec<Value> = Vec::new();
        if let Some(own) = task.get("filters").filter(|own| !own.is_null()).cloned() {
            joined.push(own);
        }
        joined.extend(filters);
        let conditions = joined.len();
        if conditions > MAX_FILTERS {
            return Err(invalid(format!("filters: at most {MAX_FILTERS}")));
        }
        let mut combined: Vec<Value> = Vec::new();
        for (index, condition) in joined.into_iter().enumerate() {
            if index > 0 {
                combined.push(json!("and"));
            }
            combined.push(condition);
        }
        task["filters"] = if conditions == 1 {
            combined.remove(0)
        } else {
            Value::Array(combined)
        };
    }
    if !query.order.is_empty() {
        if query.order.len() > MAX_ORDER {
            return Err(invalid(format!("order: at most {MAX_ORDER} rules")));
        }
        let mut rules = Vec::new();
        for rule in &query.order {
            let spec = fields
                .iter()
                .find(|spec| spec.name == rule.field && spec.kind != FieldKind::List)
                .ok_or_else(|| invalid(format!("order: cannot sort by `{}`", rule.field)))?;
            let direction = match rule.direction {
                Direction::Asc => "asc",
                Direction::Desc => "desc",
            };
            rules.push(json!(format!("{},{direction}", spec.path)));
        }
        task["order_by"] = Value::Array(rules);
    }
    if query.offset > 0 {
        if query.offset > MAX_OFFSET {
            return Err(invalid(format!("offset: at most {MAX_OFFSET}")));
        }
        task["offset"] = json!(query.offset);
    }
    Ok(())
}

/// Fields of a domain's ranked keywords (`ranked_keywords`).
pub const RANKED_KEYWORD_FIELDS: &[FieldSpec] = &[
    field("keyword", "keyword_data.keyword", FieldKind::Text),
    field(
        "volume",
        "keyword_data.keyword_info.search_volume",
        FieldKind::Number,
    ),
    field("cpc", "keyword_data.keyword_info.cpc", FieldKind::Number),
    field(
        "competition",
        "keyword_data.keyword_info.competition",
        FieldKind::Number,
    ),
    field(
        "kd",
        "keyword_data.keyword_properties.keyword_difficulty",
        FieldKind::Number,
    ),
    field(
        "intent",
        "keyword_data.search_intent_info.main_intent",
        FieldKind::Text,
    ),
    field(
        "features",
        "keyword_data.serp_info.serp_item_types",
        FieldKind::List,
    ),
    field(
        "position",
        "ranked_serp_element.serp_item.rank_group",
        FieldKind::Number,
    ),
    field(
        "previous",
        "ranked_serp_element.serp_item.rank_changes.previous_rank_absolute",
        FieldKind::Number,
    ),
    field(
        "new",
        "ranked_serp_element.serp_item.rank_changes.is_new",
        FieldKind::Flag,
    ),
    field(
        "up",
        "ranked_serp_element.serp_item.rank_changes.is_up",
        FieldKind::Flag,
    ),
    field(
        "down",
        "ranked_serp_element.serp_item.rank_changes.is_down",
        FieldKind::Flag,
    ),
    field(
        "traffic",
        "ranked_serp_element.serp_item.etv",
        FieldKind::Number,
    ),
    field(
        "value",
        "ranked_serp_element.serp_item.estimated_paid_traffic_cost",
        FieldKind::Number,
    ),
    field("url", "ranked_serp_element.serp_item.url", FieldKind::Text),
];

/// Fields of keyword ideas and suggestions (`keyword_ideas`, `keyword_suggestions`).
pub const KEYWORD_FIELDS: &[FieldSpec] = &[
    field("keyword", "keyword", FieldKind::Text),
    field("volume", "keyword_info.search_volume", FieldKind::Number),
    field("cpc", "keyword_info.cpc", FieldKind::Number),
    field("competition", "keyword_info.competition", FieldKind::Number),
    field(
        "kd",
        "keyword_properties.keyword_difficulty",
        FieldKind::Number,
    ),
    field("intent", "search_intent_info.main_intent", FieldKind::Text),
    field("features", "serp_info.serp_item_types", FieldKind::List),
    field("words", "keyword_properties.words_count", FieldKind::Number),
];

/// Fields of related keywords (`related_keywords`), whose rows nest the keyword's numbers.
pub const RELATED_KEYWORD_FIELDS: &[FieldSpec] = &[
    field("keyword", "keyword_data.keyword", FieldKind::Text),
    field(
        "volume",
        "keyword_data.keyword_info.search_volume",
        FieldKind::Number,
    ),
    field("cpc", "keyword_data.keyword_info.cpc", FieldKind::Number),
    field(
        "competition",
        "keyword_data.keyword_info.competition",
        FieldKind::Number,
    ),
    field(
        "kd",
        "keyword_data.keyword_properties.keyword_difficulty",
        FieldKind::Number,
    ),
    field(
        "intent",
        "keyword_data.search_intent_info.main_intent",
        FieldKind::Text,
    ),
    field(
        "features",
        "keyword_data.serp_info.serp_item_types",
        FieldKind::List,
    ),
    field(
        "words",
        "keyword_data.keyword_properties.words_count",
        FieldKind::Number,
    ),
];

/// Fields of a target's backlinks (`backlinks/backlinks`).
pub const BACKLINK_FIELDS: &[FieldSpec] = &[
    field("domain", "domain_from", FieldKind::Text),
    field("url", "url_from", FieldKind::Text),
    field("target", "url_to", FieldKind::Text),
    field("anchor", "anchor", FieldKind::Text),
    field("dofollow", "dofollow", FieldKind::Flag),
    field("rank", "page_from_rank", FieldKind::Number),
    field("domain_rank", "domain_from_rank", FieldKind::Number),
    field("spam", "backlink_spam_score", FieldKind::Number),
    field("first_seen", "first_seen", FieldKind::Text),
    field("last_seen", "last_seen", FieldKind::Text),
    field("status", "url_to_status_code", FieldKind::Number),
    field("country", "domain_from_country", FieldKind::Text),
];

/// Fields of a target's referring domains (`backlinks/referring_domains`).
pub const REFERRING_DOMAIN_FIELDS: &[FieldSpec] = &[
    field("domain", "domain", FieldKind::Text),
    field("rank", "rank", FieldKind::Number),
    field("backlinks", "backlinks", FieldKind::Number),
    field("spam", "backlinks_spam_score", FieldKind::Number),
    field("first_seen", "first_seen", FieldKind::Text),
    field("pages", "referring_pages", FieldKind::Number),
];
