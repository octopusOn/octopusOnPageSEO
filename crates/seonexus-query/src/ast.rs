//! Query syntax tree — the single source of truth shared by the text language, the visual
//! filter builder (as JSON) and the SQL compiler.

use seonexus_core::Date;
use serde::{Deserialize, Serialize};

use crate::field::Field;
use crate::measure::{Dimension, Metric};

/// A complete query: filter, grouping, aggregates, ordering and limit.
///
/// Without `group` and `metrics` the query lists individual hits.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Query {
    /// Rows to include; `None` keeps all.
    pub filter: Option<Expr>,
    /// Grouping keys.
    pub group: Vec<Dimension>,
    /// Aggregates; `hits` is implied when grouping without metrics.
    pub metrics: Vec<Metric>,
    /// Ordering by result column name.
    pub sort: Vec<SortKey>,
    /// Maximum number of result rows.
    pub limit: Option<u64>,
}

/// Boolean filter expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expr {
    /// All operands hold.
    And(Vec<Expr>),
    /// At least one operand holds.
    Or(Vec<Expr>),
    /// The operand does not hold.
    Not(Box<Expr>),
    /// A single field test.
    Condition(Condition),
}

impl Expr {
    /// Conjunction that flattens nested `And`s and unwraps a single operand.
    pub fn and(operands: impl IntoIterator<Item = Self>) -> Option<Self> {
        Self::flatten(operands, |e| matches!(e, Self::And(_)), Self::And)
    }

    /// Disjunction that flattens nested `Or`s and unwraps a single operand.
    pub fn or(operands: impl IntoIterator<Item = Self>) -> Option<Self> {
        Self::flatten(operands, |e| matches!(e, Self::Or(_)), Self::Or)
    }

    /// Negation.
    #[must_use]
    pub fn negate(self) -> Self {
        Self::Not(Box::new(self))
    }

    fn flatten(
        operands: impl IntoIterator<Item = Self>,
        same_kind: impl Fn(&Self) -> bool,
        build: impl Fn(Vec<Self>) -> Self,
    ) -> Option<Self> {
        let mut flat = Vec::new();
        for operand in operands {
            if same_kind(&operand) {
                if let Self::And(inner) | Self::Or(inner) = operand {
                    flat.extend(inner);
                }
            } else {
                flat.push(operand);
            }
        }
        match flat.len() {
            0 => None,
            1 => flat.pop(),
            _ => Some(build(flat)),
        }
    }
}

/// `field` satisfies `predicate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// Tested attribute.
    pub field: Field,
    /// Test.
    pub predicate: Predicate,
}

/// A test on one field. Which variants a field accepts depends on its kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    /// Equals any of the values (`status:301,302`).
    OneOf(Vec<Literal>),
    /// Text pattern where `*` matches any run of characters (case-sensitive).
    Glob(String),
    /// Regular expression (RE2 syntax).
    Regex(String),
    /// Comparison (`bytes:>1000`).
    Compare(CompareOp, Literal),
    /// Inclusive range (`date:2026-10-01..2026-10-07`).
    Between(Literal, Literal),
    /// Status class, e.g. `4` for `4xx`.
    StatusClass(u8),
    /// The last `n` days including today (`date:last7d`).
    LastDays(u32),
}

/// A typed constant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Literal {
    /// Text.
    Text(String),
    /// Non-negative integer.
    Integer(i64),
    /// Calendar date.
    Date(Date),
    /// Boolean.
    Boolean(bool),
}

/// Comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `<`
    Lt,
    /// `<=`
    Le,
}

impl CompareOp {
    /// SQL and query-language spelling.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Lt => "<",
            Self::Le => "<=",
        }
    }
}

/// Ordering by one result column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    /// Result column name.
    pub column: String,
    /// Direction.
    pub direction: SortDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    /// Smallest first.
    Asc,
    /// Largest first.
    Desc,
}

impl SortDirection {
    /// Query-language and SQL spelling.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn condition(code: i64) -> Expr {
        Expr::Condition(Condition {
            field: Field::Status,
            predicate: Predicate::OneOf(vec![Literal::Integer(code)]),
        })
    }

    #[test]
    fn constructors_flatten_and_unwrap() {
        assert_eq!(Expr::and([]), None);
        assert_eq!(Expr::and([condition(1)]), Some(condition(1)));
        let nested = Expr::and([Expr::And(vec![condition(1), condition(2)]), condition(3)]);
        assert_eq!(
            nested,
            Some(Expr::And(vec![condition(1), condition(2), condition(3)]))
        );
        let mixed = Expr::or([Expr::And(vec![condition(1), condition(2)]), condition(3)]);
        assert_eq!(
            mixed,
            Some(Expr::Or(vec![
                Expr::And(vec![condition(1), condition(2)]),
                condition(3)
            ]))
        );
    }

    #[test]
    fn serializes_to_stable_json() {
        let query = Query {
            filter: Some(condition(404)),
            group: vec![Dimension::Url],
            ..Query::default()
        };
        let json = serde_json::to_string(&query).unwrap();
        assert_eq!(
            json,
            r#"{"filter":{"condition":{"field":"status","predicate":{"one_of":[{"integer":404}]}}},"group":["url"],"metrics":[],"sort":[],"limit":null}"#
        );
        assert_eq!(serde_json::from_str::<Query>(&json).unwrap(), query);
    }
}
