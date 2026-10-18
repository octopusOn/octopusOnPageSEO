//! Property: printing then parsing any valid, normalized query yields the same query.

use octopuson_core::Date;
use proptest::prelude::*;
use proptest::sample::subsequence;

use super::{parse, to_dsl};
use crate::ast::{CompareOp, Condition, Expr, Literal, Predicate, Query, SortDirection, SortKey};
use crate::field::{Field, FieldKind};
use crate::measure::{Dimension, Metric};
use crate::shape::Shape;

/// Arbitrary printable text without `*` (which would turn a value into a pattern).
fn plain_text() -> impl Strategy<Value = String> {
    "[^*\\p{Cc}]{0,10}"
}

fn date() -> impl Strategy<Value = Date> {
    (1990_i32..2100, 1_u8..=12, 1_u8..=28).prop_map(|(y, m, d)| Date::from_ymd(y, m, d).unwrap())
}

fn compare_op() -> impl Strategy<Value = CompareOp> {
    prop_oneof![
        Just(CompareOp::Gt),
        Just(CompareOp::Ge),
        Just(CompareOp::Lt),
        Just(CompareOp::Le)
    ]
}

fn predicate(field: Field) -> BoxedStrategy<Predicate> {
    match field.kind() {
        FieldKind::Text => prop_oneof![
            prop::collection::vec(plain_text().prop_map(Literal::Text), 1..4)
                .prop_map(Predicate::OneOf),
            (plain_text(), plain_text()).prop_map(|(a, b)| Predicate::Glob(format!("{a}*{b}"))),
            plain_text()
                .prop_filter("non-empty", |t| !t.is_empty())
                .prop_map(Predicate::Regex),
        ]
        .boxed(),
        FieldKind::Integer => {
            let number = 0_i64..1_000_000;
            let base = prop_oneof![
                prop::collection::vec(number.clone().prop_map(Literal::Integer), 1..4)
                    .prop_map(Predicate::OneOf),
                (compare_op(), number.clone())
                    .prop_map(|(op, n)| Predicate::Compare(op, Literal::Integer(n))),
                (number, 0_i64..1_000).prop_map(|(low, span)| Predicate::Between(
                    Literal::Integer(low),
                    Literal::Integer(low + span)
                )),
            ];
            if field == Field::Status {
                prop_oneof![base, (1_u8..=5).prop_map(Predicate::StatusClass)].boxed()
            } else {
                base.boxed()
            }
        }
        FieldKind::Date => prop_oneof![
            prop::collection::vec(date().prop_map(Literal::Date), 1..3).prop_map(Predicate::OneOf),
            (compare_op(), date()).prop_map(|(op, d)| Predicate::Compare(op, Literal::Date(d))),
            (date(), 0_i32..400).prop_map(|(low, span)| Predicate::Between(
                Literal::Date(low),
                Literal::Date(low.add_days(span))
            )),
            (1_u32..=3_650).prop_map(Predicate::LastDays),
        ]
        .boxed(),
        FieldKind::Boolean => any::<bool>()
            .prop_map(|b| Predicate::OneOf(vec![Literal::Boolean(b)]))
            .boxed(),
    }
}

fn condition() -> impl Strategy<Value = Expr> {
    prop::sample::select(Field::ALL.to_vec()).prop_flat_map(|field| {
        predicate(field).prop_map(move |predicate| Expr::Condition(Condition { field, predicate }))
    })
}

fn expr() -> impl Strategy<Value = Expr> {
    condition().prop_recursive(4, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 2..4).prop_filter_map("and", Expr::and),
            prop::collection::vec(inner.clone(), 2..4).prop_filter_map("or", Expr::or),
            inner.prop_map(Expr::negate),
        ]
    })
}

fn query() -> impl Strategy<Value = Query> {
    (
        prop::option::of(expr()),
        subsequence(Dimension::ALL.to_vec(), 0..3),
        subsequence(Metric::ALL.to_vec(), 0..3),
        prop::option::of(1_u64..1_000_000),
    )
        .prop_flat_map(|(filter, group, metrics, limit)| {
            let base = Query {
                filter,
                group,
                metrics,
                sort: Vec::new(),
                limit,
            };
            let columns: Vec<String> = Shape::of(&base)
                .columns()
                .into_iter()
                .map(|c| c.name)
                .collect();
            let max = columns.len().min(2);
            (
                Just(base),
                subsequence(columns, 0..=max),
                any::<[bool; 2]>(),
            )
        })
        .prop_map(|(mut query, columns, descending)| {
            query.sort = columns
                .into_iter()
                .zip(descending)
                .map(|(column, desc)| SortKey {
                    column,
                    direction: if desc {
                        SortDirection::Desc
                    } else {
                        SortDirection::Asc
                    },
                })
                .collect();
            query
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn print_then_parse_is_identity(query in query()) {
        let text = to_dsl(&query);
        let parsed = parse(&text);
        prop_assert_eq!(parsed.as_ref().ok(), Some(&query), "text: {} error: {:?}", text, parsed.as_ref().err());
    }

    #[test]
    fn parser_never_panics(text in "\\PC{0,64}") {
        let _ = parse(&text);
    }
}
