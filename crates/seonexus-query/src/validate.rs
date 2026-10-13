//! Semantic checks for queries built from the text language or deserialized from JSON.

use std::collections::HashSet;

use crate::ast::{Expr, Literal, Predicate, Query};
use crate::field::{Field, FieldKind};
use crate::shape::Shape;

/// Largest accepted `limit`.
pub const MAX_LIMIT: u64 = 1_000_000_000;
/// Largest accepted `lastNd`.
pub const MAX_LAST_DAYS: u32 = 3_650;

/// A query that is well-formed syntactically but not meaningful.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ValidationError {
    /// The predicate is not applicable to the field (e.g. a regex on `status`).
    #[error("`{field}` does not support this kind of test")]
    UnsupportedPredicate {
        /// Field name.
        field: &'static str,
    },
    /// A predicate value is out of range or malformed.
    #[error("invalid value for `{field}`: {reason}")]
    InvalidValue {
        /// Field name.
        field: &'static str,
        /// Explanation.
        reason: &'static str,
    },
    /// `And` / `Or` without operands.
    #[error("empty AND/OR group")]
    EmptyGroup,
    /// A dimension appears twice in `group`.
    #[error("`{0}` is listed twice in group")]
    DuplicateDimension(&'static str),
    /// A metric appears twice in `metric`.
    #[error("`{0}` is listed twice in metric")]
    DuplicateMetric(&'static str),
    /// Sorting by a column that the query does not return.
    #[error("cannot sort by `{column}`; available columns: {available}")]
    UnknownSortColumn {
        /// Requested column.
        column: String,
        /// Comma-separated result columns.
        available: String,
    },
    /// The same column is sorted twice.
    #[error("`{0}` is sorted twice")]
    DuplicateSortColumn(String),
    /// `limit` outside `1..=MAX_LIMIT`.
    #[error("limit must be between 1 and {MAX_LIMIT}")]
    LimitOutOfRange,
}

/// Checks `query`.
///
/// # Errors
/// The first problem found.
pub fn validate(query: &Query) -> Result<(), ValidationError> {
    if let Some(filter) = &query.filter {
        validate_expr(filter)?;
    }
    if let Some(dimension) = first_duplicate(query.group.iter().map(|d| d.name())) {
        return Err(ValidationError::DuplicateDimension(dimension));
    }
    if let Some(metric) = first_duplicate(query.metrics.iter().map(|m| m.name())) {
        return Err(ValidationError::DuplicateMetric(metric));
    }
    validate_sort(query)?;
    match query.limit {
        Some(limit) if !(1..=MAX_LIMIT).contains(&limit) => Err(ValidationError::LimitOutOfRange),
        _ => Ok(()),
    }
}

fn first_duplicate<'a>(names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen = HashSet::new();
    names.into_iter().find(|name| !seen.insert(*name))
}

fn validate_sort(query: &Query) -> Result<(), ValidationError> {
    let columns = Shape::of(query).columns();
    for key in &query.sort {
        if !columns.iter().any(|c| c.name == key.column) {
            let available: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
            return Err(ValidationError::UnknownSortColumn {
                column: key.column.clone(),
                available: available.join(", "),
            });
        }
    }
    match first_duplicate(query.sort.iter().map(|k| k.column.as_str())) {
        Some(column) => Err(ValidationError::DuplicateSortColumn(column.to_owned())),
        None => Ok(()),
    }
}

fn validate_expr(expr: &Expr) -> Result<(), ValidationError> {
    match expr {
        Expr::And(operands) | Expr::Or(operands) => {
            if operands.is_empty() {
                return Err(ValidationError::EmptyGroup);
            }
            operands.iter().try_for_each(validate_expr)
        }
        Expr::Not(inner) => validate_expr(inner),
        Expr::Condition(condition) => validate_predicate(condition.field, &condition.predicate),
    }
}

/// Checks that `predicate` suits `field`.
///
/// # Errors
/// [`ValidationError::UnsupportedPredicate`] or [`ValidationError::InvalidValue`].
pub fn validate_predicate(field: Field, predicate: &Predicate) -> Result<(), ValidationError> {
    let name = field.name();
    let invalid = |reason| {
        Err(ValidationError::InvalidValue {
            field: name,
            reason,
        })
    };
    let kind = field.kind();
    match predicate {
        Predicate::OneOf(values) if values.is_empty() => invalid("at least one value is required"),
        Predicate::OneOf(values) if kind == FieldKind::Boolean && values.len() != 1 => {
            invalid("expected a single true or false")
        }
        Predicate::OneOf(values) => values
            .iter()
            .try_for_each(|value| validate_literal(field, value)),
        Predicate::Glob(pattern) if kind == FieldKind::Text => {
            if pattern.contains('*') {
                Ok(())
            } else {
                invalid("a pattern needs at least one `*`")
            }
        }
        Predicate::Regex(pattern) if kind == FieldKind::Text => {
            if pattern.is_empty() {
                invalid("the regular expression is empty")
            } else {
                Ok(())
            }
        }
        Predicate::Compare(_, value) if is_ordered(kind) => validate_literal(field, value),
        Predicate::Between(low, high) if is_ordered(kind) => {
            validate_literal(field, low)?;
            validate_literal(field, high)?;
            if literal_le(low, high) {
                Ok(())
            } else {
                invalid("the range start is after its end")
            }
        }
        Predicate::StatusClass(class) if field == Field::Status => {
            if (1..=5).contains(class) {
                Ok(())
            } else {
                invalid("status classes are 1xx to 5xx")
            }
        }
        Predicate::LastDays(days) if kind == FieldKind::Date => {
            if (1..=MAX_LAST_DAYS).contains(days) {
                Ok(())
            } else {
                invalid("lastNd accepts 1 to 3650 days")
            }
        }
        _ => Err(ValidationError::UnsupportedPredicate { field: name }),
    }
}

fn is_ordered(kind: FieldKind) -> bool {
    matches!(kind, FieldKind::Integer | FieldKind::Date)
}

fn validate_literal(field: Field, value: &Literal) -> Result<(), ValidationError> {
    let name = field.name();
    let matches_kind = match (field.kind(), value) {
        (FieldKind::Text, Literal::Text(text)) => {
            return if text.contains('*') {
                Err(ValidationError::InvalidValue {
                    field: name,
                    reason: "`*` patterns cannot be combined with a list of values",
                })
            } else {
                Ok(())
            };
        }
        (FieldKind::Integer, Literal::Integer(number)) => {
            return if *number < 0 {
                Err(ValidationError::InvalidValue {
                    field: name,
                    reason: "negative numbers are not allowed",
                })
            } else {
                Ok(())
            };
        }
        (FieldKind::Date, Literal::Date(_)) | (FieldKind::Boolean, Literal::Boolean(_)) => true,
        _ => false,
    };
    if matches_kind {
        Ok(())
    } else {
        Err(ValidationError::InvalidValue {
            field: name,
            reason: "the value has the wrong type",
        })
    }
}

fn literal_le(low: &Literal, high: &Literal) -> bool {
    match (low, high) {
        (Literal::Integer(a), Literal::Integer(b)) => a <= b,
        (Literal::Date(a), Literal::Date(b)) => a <= b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Condition, SortDirection, SortKey};
    use crate::measure::Dimension;

    fn with_filter(field: Field, predicate: Predicate) -> Query {
        Query {
            filter: Some(Expr::Condition(Condition { field, predicate })),
            ..Query::default()
        }
    }

    #[test]
    fn rejects_mismatched_predicates() {
        let regex_on_status = with_filter(Field::Status, Predicate::Regex("4..".into()));
        assert!(matches!(
            validate(&regex_on_status),
            Err(ValidationError::UnsupportedPredicate { field: "status" })
        ));
        let class_on_bytes = with_filter(Field::Bytes, Predicate::StatusClass(4));
        assert!(validate(&class_on_bytes).is_err());
        let text_on_status = with_filter(
            Field::Status,
            Predicate::OneOf(vec![Literal::Text("404".into())]),
        );
        assert!(validate(&text_on_status).is_err());
        let reversed = with_filter(
            Field::Bytes,
            Predicate::Between(Literal::Integer(5), Literal::Integer(1)),
        );
        assert!(validate(&reversed).is_err());
    }

    #[test]
    fn checks_sort_columns_against_shape() {
        let query = Query {
            group: vec![Dimension::Url],
            sort: vec![SortKey {
                column: "ip".into(),
                direction: SortDirection::Asc,
            }],
            ..Query::default()
        };
        assert!(matches!(
            validate(&query),
            Err(ValidationError::UnknownSortColumn { .. })
        ));
    }

    #[test]
    fn accepts_valid_queries() {
        let query = with_filter(Field::Url, Predicate::Glob("/blog/*".into()));
        assert_eq!(validate(&query), Ok(()));
        let zero_limit = Query {
            limit: Some(0),
            ..Query::default()
        };
        assert_eq!(validate(&zero_limit), Err(ValidationError::LimitOutOfRange));
    }
}
