//! Canonical text form of a [`Query`]; `parse(&to_dsl(q)) == q` for every valid,
//! normalized query (see the round-trip property test).

use std::fmt::Write as _;

use super::lexer::is_bare_char;
use crate::ast::{Condition, Expr, Literal, Predicate, Query};

/// Renders `query` in the query language.
pub fn to_dsl(query: &Query) -> String {
    let mut parts = Vec::new();
    if let Some(filter) = &query.filter {
        parts.push(expr(filter));
    }
    if !query.group.is_empty() {
        let names: Vec<&str> = query.group.iter().map(|d| d.name()).collect();
        parts.push(format!("group:{}", names.join(",")));
    }
    if !query.metrics.is_empty() {
        let names: Vec<&str> = query.metrics.iter().map(|m| m.name()).collect();
        parts.push(format!("metric:{}", names.join(",")));
    }
    if !query.sort.is_empty() {
        let keys: Vec<String> = query
            .sort
            .iter()
            .map(|key| format!("{}:{}", key.column, key.direction.name()))
            .collect();
        parts.push(format!("sort:{}", keys.join(",")));
    }
    if let Some(limit) = query.limit {
        parts.push(format!("limit:{limit}"));
    }
    parts.join(" ")
}

fn expr(node: &Expr) -> String {
    match node {
        Expr::Condition(c) => condition(c),
        Expr::And(operands) => operands
            .iter()
            .map(|operand| match operand {
                Expr::Or(_) => parenthesized(operand),
                _ => expr(operand),
            })
            .collect::<Vec<_>>()
            .join(" "),
        Expr::Or(operands) => operands
            .iter()
            .map(|operand| match operand {
                Expr::Or(_) => parenthesized(operand),
                _ => expr(operand),
            })
            .collect::<Vec<_>>()
            .join(" OR "),
        Expr::Not(inner) => match inner.as_ref() {
            Expr::Condition(_) | Expr::Not(_) => format!("-{}", expr(inner)),
            _ => format!("-{}", parenthesized(inner)),
        },
    }
}

fn parenthesized(node: &Expr) -> String {
    format!("({})", expr(node))
}

fn condition(condition: &Condition) -> String {
    let mut out = condition.field.name().to_owned();
    match &condition.predicate {
        Predicate::OneOf(values) => {
            out.push(':');
            let rendered: Vec<String> = values.iter().map(literal).collect();
            out.push_str(&rendered.join(","));
        }
        Predicate::Glob(pattern) => {
            out.push(':');
            out.push_str(&text(pattern));
        }
        Predicate::Regex(pattern) => {
            out.push_str(":~");
            out.push_str(&text(pattern));
        }
        Predicate::Compare(op, value) => {
            out.push(':');
            out.push_str(op.symbol());
            out.push_str(&literal(value));
        }
        Predicate::Between(low, high) => {
            let _ = write!(out, ":{}..{}", literal(low), literal(high));
        }
        Predicate::StatusClass(class) => {
            let _ = write!(out, ":{class}xx");
        }
        Predicate::LastDays(days) => {
            let _ = write!(out, ":last{days}d");
        }
    }
    out
}

fn literal(value: &Literal) -> String {
    match value {
        Literal::Text(value) => text(value),
        Literal::Integer(value) => value.to_string(),
        Literal::Date(value) => value.to_string(),
        Literal::Boolean(value) => value.to_string(),
    }
}

/// Bare when the lexer would read it back unchanged, quoted otherwise.
fn text(value: &str) -> String {
    let bare =
        !value.is_empty() && value.chars().all(is_bare_char) && !value.starts_with(['~', '>', '<']);
    if bare {
        return value.to_owned();
    }
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        if matches!(c, '"' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}
