//! Value suggestions for query editors: the most frequent values of a field, optionally
//! narrowed by typed text (case-insensitive) and by the editor's other filters.

use seonexus_core::table::{Cell, Column, TableSink};
use seonexus_query::{
    Condition, Dimension, Expr, Field, Metric, Predicate, Query, SortDirection, SortKey,
};
use serde::Serialize;

use super::{QueryError, QueryRequest, QuerySpec, parse_query, run_query};
use crate::workspace::Workspace;

/// Most suggestions returned at once.
pub const MAX_SUGGESTIONS: u64 = 200;

/// Input of [`suggest_values`].
#[derive(Debug, Clone)]
pub struct SuggestRequest {
    /// Site name.
    pub site: String,
    /// Field whose values are suggested, e.g. `bot` or `ua`.
    pub field: String,
    /// Text the values must contain (case-insensitive, `*` matches any text); empty for the
    /// most frequent values.
    pub search: String,
    /// Other conditions to respect, e.g. `date:last30d` (directives are ignored).
    pub filter: Option<String>,
    /// Number of suggestions (at most [`MAX_SUGGESTIONS`]).
    pub limit: u64,
}

/// A suggested value and how often it occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValueSuggestion {
    /// Value as the query language expects it.
    pub value: String,
    /// Hits with this value.
    pub hits: u64,
}

/// The most frequent values of a field. Fields without a matching grouping key (dates,
/// sizes, response times, booleans) have no suggestions.
///
/// # Errors
/// See [`QueryError`]; an invalid `filter` is a parse error.
pub fn suggest_values(
    workspace: &Workspace,
    request: &SuggestRequest,
) -> Result<Vec<ValueSuggestion>, QueryError> {
    let (Some(field), Some(dimension)) = (
        Field::from_name(&request.field),
        Dimension::from_name(&request.field),
    ) else {
        return Ok(Vec::new());
    };
    let mut conditions = Vec::new();
    if let Some(filter) = request.filter.as_deref().filter(|f| !f.trim().is_empty()) {
        conditions.extend(parse_query(filter)?.filter);
    }
    let search = request.search.trim();
    if !search.is_empty() {
        let pattern = format!("(?i){}", search_pattern(search));
        conditions.push(Expr::Condition(Condition {
            field,
            predicate: Predicate::Regex(pattern),
        }));
    }
    let query = Query {
        filter: Expr::and(conditions),
        group: vec![dimension],
        metrics: vec![Metric::Hits],
        sort: vec![SortKey {
            column: Metric::Hits.name().to_owned(),
            direction: SortDirection::Desc,
        }],
        limit: Some(request.limit.clamp(1, MAX_SUGGESTIONS)),
    };
    let mut sink = Suggestions::default();
    run_query(
        workspace,
        &QueryRequest {
            site: request.site.clone(),
            spec: QuerySpec::Ast { query },
            page: None,
        },
        &mut sink,
    )?;
    Ok(sink.values)
}

/// RE2 pattern finding typed text: literal, except that `*` matches any text, as in the
/// `googlebot*` values users type into the filter form.
fn search_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if c == '*' {
            pattern.push_str(".*");
            continue;
        }
        if r"\.+?()|[]{}^$".contains(c) {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern
}

#[derive(Default)]
struct Suggestions {
    values: Vec<ValueSuggestion>,
}

impl TableSink for Suggestions {
    fn begin(&mut self, _columns: &[Column]) -> std::io::Result<()> {
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> std::io::Result<()> {
        let value = match cells.first() {
            Some(Cell::Null) | None => return Ok(()),
            Some(cell) => cell.to_string(),
        };
        let hits = match cells.get(1) {
            Some(Cell::Integer(n)) => u64::try_from(*n).unwrap_or(0),
            _ => 0,
        };
        self.values.push(ValueSuggestion { value, hits });
        Ok(())
    }

    fn finish(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    fn suggest(
        fixture: &Fixture,
        field: &str,
        search: &str,
        filter: Option<&str>,
    ) -> Vec<(String, u64)> {
        let request = SuggestRequest {
            site: "acme.com".into(),
            field: field.into(),
            search: search.into(),
            filter: filter.map(Into::into),
            limit: 10,
        };
        suggest_values(&fixture.workspace, &request)
            .unwrap()
            .into_iter()
            .map(|s| (s.value, s.hits))
            .collect()
    }

    #[test]
    fn suggests_frequent_values_case_insensitively() {
        let fixture = Fixture::imported();
        assert_eq!(
            suggest(&fixture, "bot", "", None),
            [
                ("googlebot-desktop".to_owned(), 3),
                ("gptbot".to_owned(), 1)
            ]
        );
        assert_eq!(
            suggest(&fixture, "ua", "GOOGLE", None),
            [("Googlebot/2.1".to_owned(), 3)]
        );
        assert_eq!(
            suggest(&fixture, "status", "", Some("bot:*")),
            [("200".to_owned(), 3), ("404".to_owned(), 1)]
        );
        assert_eq!(
            suggest(&fixture, "url", "a?x", None),
            [("/a?x=1".to_owned(), 1)]
        );
        assert_eq!(
            suggest(&fixture, "bot", "G*BOT", None),
            [
                ("googlebot-desktop".to_owned(), 3),
                ("gptbot".to_owned(), 1)
            ]
        );
        assert_eq!(suggest(&fixture, "bytes", "", None), Vec::new());
        assert_eq!(search_pattern("a.b(c)*d"), r"a\.b\(c\).*d");
    }
}
