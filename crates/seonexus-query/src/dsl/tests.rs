use seonexus_core::Date;

use super::*;
use crate::ast::{CompareOp, Condition, Expr, Literal, Predicate, SortDirection, SortKey};
use crate::field::Field;
use crate::measure::{Dimension, Metric};

fn cond(field: Field, predicate: Predicate) -> Expr {
    Expr::Condition(Condition { field, predicate })
}

fn date(text: &str) -> Literal {
    Literal::Date(Date::parse(text).unwrap())
}

fn error_kind(text: &str) -> ParseErrorKind {
    parse(text).unwrap_err().kind().clone()
}

#[test]
fn parses_the_documented_example() {
    let query =
        parse("status:5xx group:url metric:hits,last_seen sort:hits desc limit:500").unwrap();
    assert_eq!(
        query.filter,
        Some(cond(Field::Status, Predicate::StatusClass(5)))
    );
    assert_eq!(query.group, [Dimension::Url]);
    assert_eq!(query.metrics, [Metric::Hits, Metric::LastSeen]);
    assert_eq!(
        query.sort,
        [SortKey {
            column: "hits".into(),
            direction: SortDirection::Desc
        }]
    );
    assert_eq!(query.limit, Some(500));
}

#[test]
fn and_binds_tighter_than_or() {
    let query = parse("status:404 OR status:410 url:/a").unwrap();
    let status = |code| {
        cond(
            Field::Status,
            Predicate::OneOf(vec![Literal::Integer(code)]),
        )
    };
    let url = cond(
        Field::Url,
        Predicate::OneOf(vec![Literal::Text("/a".into())]),
    );
    assert_eq!(
        query.filter,
        Some(Expr::Or(vec![
            status(404),
            Expr::And(vec![status(410), url.clone()])
        ]))
    );
    let explicit = parse("(status:404 OR status:410) AND url:/a").unwrap();
    assert_eq!(
        explicit.filter,
        Some(Expr::And(vec![
            Expr::Or(vec![status(404), status(410)]),
            url
        ]))
    );
}

#[test]
fn negation_forms_are_equivalent() {
    let minus = parse("-ua:*bot*").unwrap();
    let not = parse("NOT ua:*bot*").unwrap();
    assert_eq!(minus, not);
    assert_eq!(
        minus.filter,
        Some(cond(Field::Ua, Predicate::Glob("*bot*".into())).negate())
    );
}

#[test]
fn interprets_values_by_field_kind() {
    let cases = [
        (
            "status:301,302",
            cond(
                Field::Status,
                Predicate::OneOf(vec![Literal::Integer(301), Literal::Integer(302)]),
            ),
        ),
        (
            "bytes:>=1024",
            cond(
                Field::Bytes,
                Predicate::Compare(CompareOp::Ge, Literal::Integer(1024)),
            ),
        ),
        (
            "response_ms:100..900",
            cond(
                Field::ResponseMs,
                Predicate::Between(Literal::Integer(100), Literal::Integer(900)),
            ),
        ),
        (
            "date:2026-10-01..2026-10-07",
            cond(
                Field::Date,
                Predicate::Between(date("2026-10-01"), date("2026-10-07")),
            ),
        ),
        (
            "date:<2026-10-01",
            cond(
                Field::Date,
                Predicate::Compare(CompareOp::Lt, date("2026-10-01")),
            ),
        ),
        ("date:last7d", cond(Field::Date, Predicate::LastDays(7))),
        (
            "params:true",
            cond(
                Field::Params,
                Predicate::OneOf(vec![Literal::Boolean(true)]),
            ),
        ),
        (
            "url:~\"\\.php$\"",
            cond(Field::Url, Predicate::Regex("\\.php$".into())),
        ),
        (
            "ua:\"Mozilla/5.0 (compatible)\"",
            cond(
                Field::Ua,
                Predicate::OneOf(vec![Literal::Text("Mozilla/5.0 (compatible)".into())]),
            ),
        ),
        (
            "ip:1.2.3.4,5.6.7.8",
            cond(
                Field::Ip,
                Predicate::OneOf(vec![
                    Literal::Text("1.2.3.4".into()),
                    Literal::Text("5.6.7.8".into()),
                ]),
            ),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(parse(text).unwrap().filter, Some(expected), "{text}");
    }
}

#[test]
fn empty_query_is_valid() {
    assert_eq!(parse("  ").unwrap(), crate::Query::default());
}

#[test]
fn reports_helpful_errors() {
    assert!(matches!(error_kind("foo:1"), ParseErrorKind::UnknownField(name) if name == "foo"));
    assert!(matches!(
        error_kind("blog"),
        ParseErrorKind::ExpectedCondition(_)
    ));
    assert!(matches!(
        error_kind("status:abc"),
        ParseErrorKind::InvalidValue { .. }
    ));
    assert!(matches!(
        error_kind("status:~4.."),
        ParseErrorKind::UnsupportedOperator { .. }
    ));
    assert!(matches!(
        error_kind("url:>a"),
        ParseErrorKind::UnsupportedOperator { .. }
    ));
    assert!(matches!(
        error_kind("url:/a*,/b"),
        ParseErrorKind::InvalidValue { .. }
    ));
    assert!(matches!(
        error_kind("bytes:9..1"),
        ParseErrorKind::InvalidValue { .. }
    ));
    assert!(matches!(
        error_kind("date:last0d"),
        ParseErrorKind::InvalidValue { .. }
    ));
    assert!(matches!(
        error_kind("(status:200"),
        ParseErrorKind::UnbalancedParenthesis
    ));
    assert!(matches!(
        error_kind("status:200)"),
        ParseErrorKind::UnbalancedParenthesis
    ));
    assert!(matches!(error_kind("()"), ParseErrorKind::EmptyParentheses));
    assert!(matches!(
        error_kind("OR status:200"),
        ParseErrorKind::DanglingOperator("OR")
    ));
    assert!(matches!(
        error_kind("status:200 OR"),
        ParseErrorKind::DanglingOperator("OR")
    ));
    assert!(matches!(
        error_kind("status:200 AND"),
        ParseErrorKind::DanglingOperator("AND")
    ));
    assert!(matches!(
        error_kind("NOT"),
        ParseErrorKind::DanglingOperator("NOT")
    ));
    assert!(matches!(
        error_kind("(group:url)"),
        ParseErrorKind::MisplacedDirective(_)
    ));
    assert!(matches!(
        error_kind("limit:1 limit:2"),
        ParseErrorKind::DuplicateDirective(_)
    ));
    assert!(matches!(
        error_kind("group:nope"),
        ParseErrorKind::UnknownDimension(_)
    ));
    assert!(matches!(
        error_kind("metric:nope"),
        ParseErrorKind::UnknownMetric(_)
    ));
    assert!(matches!(
        error_kind("limit:0"),
        ParseErrorKind::InvalidValue { .. }
    ));
    assert!(matches!(
        error_kind("sort:hits:up"),
        ParseErrorKind::InvalidValue { .. }
    ));
}

#[test]
fn error_spans_point_at_the_problem() {
    let error = parse("status:404 foo:1").unwrap_err();
    assert_eq!(error.span(), 11..14);
    let error = parse("status:404 bytes:x").unwrap_err();
    assert_eq!(error.span(), 17..18);
}

#[test]
fn rejects_hostile_nesting_and_size() {
    let deep = format!("{}status:200{}", "(".repeat(500), ")".repeat(500));
    assert_eq!(error_kind(&deep), ParseErrorKind::TooDeep);
    let negations = format!("{}status:200", "-".repeat(500));
    assert_eq!(error_kind(&negations), ParseErrorKind::TooDeep);
    let long = "status:200 ".repeat(2_000);
    assert_eq!(error_kind(&long), ParseErrorKind::TooLong);
}

#[test]
fn prints_canonical_text() {
    let query =
        parse("(status:404 OR status:410) url:\"a b\" -ua:~\"(?i)bot\" sort:hits group:url")
            .unwrap();
    assert_eq!(
        to_dsl(&query),
        "(status:404 OR status:410) url:\"a b\" -ua:~\"(?i)bot\" group:url sort:hits:desc"
    );
}
