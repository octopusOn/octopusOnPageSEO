//! Turns a lexed `field<op>values` term into a typed [`Condition`].

use std::ops::Range;

use octopuson_core::Date;

use super::error::{ParseError, ParseErrorKind};
use super::lexer::{Op, RawValue, Term};
use crate::ast::{CompareOp, Condition, Literal, Predicate};
use crate::field::{Field, FieldKind};
use crate::validate::MAX_LAST_DAYS;

pub(super) fn condition(
    field: Field,
    term: Term,
    span: Range<usize>,
) -> Result<Condition, ParseError> {
    let predicate = match term.op {
        Op::Eq => equality(field, term.values, span)?,
        Op::Regex => regex(field, term.values, span)?,
        op => compare(field, op, term.values, span)?,
    };
    Ok(Condition { field, predicate })
}

fn unsupported(field: Field, op: Op, span: Range<usize>) -> ParseError {
    ParseError::new(
        ParseErrorKind::UnsupportedOperator {
            field: field.name().to_owned(),
            operator: op.symbol(),
        },
        span,
    )
}

fn invalid(field: Field, expected: &'static str, span: Range<usize>) -> ParseError {
    ParseError::new(
        ParseErrorKind::InvalidValue {
            field: field.name().to_owned(),
            expected,
        },
        span,
    )
}

/// Exactly one value, optionally required to be unquoted.
fn single(
    field: Field,
    mut values: Vec<RawValue>,
    allow_quoted: bool,
    span: Range<usize>,
) -> Result<RawValue, ParseError> {
    if values.len() != 1 {
        return Err(ParseError::new(
            ParseErrorKind::ListNotAllowed(field.name().to_owned()),
            span,
        ));
    }
    let value = values.remove(0);
    if value.quoted && !allow_quoted {
        return Err(invalid(field, expected_form(field.kind()), value.span));
    }
    Ok(value)
}

fn regex(field: Field, values: Vec<RawValue>, span: Range<usize>) -> Result<Predicate, ParseError> {
    if field.kind() != FieldKind::Text {
        return Err(unsupported(field, Op::Regex, span));
    }
    let value = single(field, values, true, span)?;
    if value.text.is_empty() {
        return Err(invalid(field, "a regular expression", value.span));
    }
    Ok(Predicate::Regex(value.text))
}

fn compare(
    field: Field,
    op: Op,
    values: Vec<RawValue>,
    span: Range<usize>,
) -> Result<Predicate, ParseError> {
    let comparison = match op {
        Op::Gt => CompareOp::Gt,
        Op::Ge => CompareOp::Ge,
        Op::Lt => CompareOp::Lt,
        Op::Le => CompareOp::Le,
        Op::Eq | Op::Regex => return Err(unsupported(field, op, span)),
    };
    if !matches!(field.kind(), FieldKind::Integer | FieldKind::Date) {
        return Err(unsupported(field, op, span));
    }
    let value = single(field, values, false, span)?;
    Ok(Predicate::Compare(
        comparison,
        ordered_literal(field, &value)?,
    ))
}

fn equality(
    field: Field,
    values: Vec<RawValue>,
    span: Range<usize>,
) -> Result<Predicate, ParseError> {
    match field.kind() {
        FieldKind::Text => text_equality(field, values),
        FieldKind::Boolean => {
            let value = single(field, values, false, span)?;
            match value.text.as_str() {
                "true" => Ok(Predicate::OneOf(vec![Literal::Boolean(true)])),
                "false" => Ok(Predicate::OneOf(vec![Literal::Boolean(false)])),
                _ => Err(invalid(field, "true or false", value.span)),
            }
        }
        FieldKind::Integer | FieldKind::Date => ordered_equality(field, &values, span),
    }
}

fn text_equality(field: Field, values: Vec<RawValue>) -> Result<Predicate, ParseError> {
    if let [value] = values.as_slice()
        && value.text.contains('*')
    {
        return Ok(Predicate::Glob(value.text.clone()));
    }
    if let Some(pattern) = values.iter().find(|v| v.text.contains('*')) {
        return Err(invalid(
            field,
            "a single `*` pattern (patterns cannot be combined with a list)",
            pattern.span.clone(),
        ));
    }
    Ok(Predicate::OneOf(
        values.into_iter().map(|v| Literal::Text(v.text)).collect(),
    ))
}

fn ordered_equality(
    field: Field,
    values: &[RawValue],
    span: Range<usize>,
) -> Result<Predicate, ParseError> {
    if let [value] = values
        && !value.quoted
        && let Some(predicate) = shorthand(field, value)?
    {
        return Ok(predicate);
    }
    let literals = values
        .iter()
        .map(|value| {
            if value.quoted {
                Err(invalid(
                    field,
                    expected_form(field.kind()),
                    value.span.clone(),
                ))
            } else {
                ordered_literal(field, value)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if literals.is_empty() {
        return Err(ParseError::new(ParseErrorKind::MissingValue, span));
    }
    Ok(Predicate::OneOf(literals))
}

/// Ranges (`a..b`), status classes (`4xx`) and relative dates (`last7d`).
fn shorthand(field: Field, value: &RawValue) -> Result<Option<Predicate>, ParseError> {
    let text = value.text.as_str();
    if let Some((low, high)) = text.split_once("..") {
        let low = literal_at(field, low, value)?;
        let high = literal_at(field, high, value)?;
        let ordered = match (&low, &high) {
            (Literal::Integer(a), Literal::Integer(b)) => a <= b,
            (Literal::Date(a), Literal::Date(b)) => a <= b,
            _ => false,
        };
        if !ordered {
            return Err(invalid(
                field,
                "a range whose start is not after its end",
                value.span.clone(),
            ));
        }
        return Ok(Some(Predicate::Between(low, high)));
    }
    if field == Field::Status
        && let Some(class) = status_class(text)
    {
        return Ok(Some(Predicate::StatusClass(class)));
    }
    if field.kind() == FieldKind::Date
        && let Some(days) = text.strip_prefix("last").and_then(|t| t.strip_suffix('d'))
    {
        return match days.parse::<u32>() {
            Ok(days) if (1..=MAX_LAST_DAYS).contains(&days) => Ok(Some(Predicate::LastDays(days))),
            _ => Err(invalid(
                field,
                "lastNd with N between 1 and 3650",
                value.span.clone(),
            )),
        };
    }
    Ok(None)
}

fn status_class(text: &str) -> Option<u8> {
    let bytes = text.as_bytes();
    let valid = bytes.len() == 3
        && (b'1'..=b'5').contains(&bytes[0])
        && bytes[1].eq_ignore_ascii_case(&b'x')
        && bytes[2].eq_ignore_ascii_case(&b'x');
    valid.then(|| bytes[0] - b'0')
}

fn ordered_literal(field: Field, value: &RawValue) -> Result<Literal, ParseError> {
    literal_at(field, &value.text, value)
}

fn literal_at(field: Field, text: &str, value: &RawValue) -> Result<Literal, ParseError> {
    let parsed = match field.kind() {
        FieldKind::Integer => parse_integer(text).map(Literal::Integer),
        FieldKind::Date => Date::parse(text).map(Literal::Date),
        FieldKind::Text | FieldKind::Boolean => None,
    };
    parsed.ok_or_else(|| invalid(field, expected_form(field.kind()), value.span.clone()))
}

fn parse_integer(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn expected_form(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Integer => "a number, a list (1,2), a range (1..9) or a comparison (:>9)",
        FieldKind::Date => "YYYY-MM-DD, a range (YYYY-MM-DD..YYYY-MM-DD) or lastNd",
        FieldKind::Boolean => "true or false",
        FieldKind::Text => "text",
    }
}
