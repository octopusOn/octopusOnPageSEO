//! Recursive-descent parser: tokens → [`Query`].
//!
//! ```text
//! query   = or                       (directives may appear between top-level terms)
//! or      = and { "OR" and }
//! and     = unary { ["AND"] unary }
//! unary   = ("-" | "NOT") unary | primary
//! primary = "(" or ")" | field op value { "," value }
//! ```

use std::ops::Range;

use super::error::{ParseError, ParseErrorKind};
use super::lexer::{Op, RawValue, Term, Token, TokenKind};
use super::value;
use crate::ast::{Expr, Query, SortDirection, SortKey};
use crate::field::Field;
use crate::measure::{Dimension, Metric};
use crate::validate::MAX_LIMIT;

pub(super) fn parse_tokens(tokens: Vec<Token>, text_len: usize) -> Result<Query, ParseError> {
    let mut parser = Parser {
        tokens,
        index: 0,
        text_len,
        query: Query::default(),
        seen: Vec::new(),
    };
    let filter = parser.or(0)?;
    if let Some(token) = parser.peek() {
        return Err(ParseError::new(
            ParseErrorKind::UnbalancedParenthesis,
            token.span.clone(),
        ));
    }
    parser.query.filter = filter;
    Ok(parser.query)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    text_len: usize,
    query: Query,
    seen: Vec<&'static str>,
}

const DIRECTIVES: [&str; 4] = ["group", "metric", "sort", "limit"];

/// Bounds recursion so hostile input cannot exhaust the stack.
const MAX_DEPTH: usize = 64;

fn is_keyword(token: &Token, keyword: &str) -> bool {
    matches!(&token.kind, TokenKind::Word(word) if word.eq_ignore_ascii_case(keyword))
}

fn directive_name(token: &Token) -> Option<&'static str> {
    match &token.kind {
        TokenKind::Term(term) => DIRECTIVES.into_iter().find(|d| *d == term.key),
        _ => None,
    }
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        self.index += usize::from(token.is_some());
        token
    }

    fn end_span(&self) -> Range<usize> {
        self.text_len..self.text_len
    }

    fn or(&mut self, depth: usize) -> Result<Option<Expr>, ParseError> {
        let mut operands = Vec::new();
        let mut pending_or: Option<Range<usize>> = None;
        loop {
            let operand = self.and(depth)?;
            match (operand, &pending_or) {
                (Some(expr), _) => operands.push(expr),
                (None, Some(span)) => {
                    return Err(ParseError::new(
                        ParseErrorKind::DanglingOperator("OR"),
                        span.clone(),
                    ));
                }
                (None, None) => {}
            }
            match self.peek() {
                Some(token) if is_keyword(token, "OR") => {
                    if operands.is_empty() {
                        return Err(ParseError::new(
                            ParseErrorKind::DanglingOperator("OR"),
                            token.span.clone(),
                        ));
                    }
                    pending_or = Some(token.span.clone());
                    self.index += 1;
                }
                _ => return Ok(Expr::or(operands)),
            }
        }
    }

    fn and(&mut self, depth: usize) -> Result<Option<Expr>, ParseError> {
        let mut operands = Vec::new();
        while let Some(token) = self.peek() {
            if matches!(token.kind, TokenKind::Close) || is_keyword(token, "OR") {
                break;
            }
            if is_keyword(token, "AND") {
                let span = token.span.clone();
                self.index += 1;
                if operands.is_empty() || self.at_operand_end() {
                    return Err(ParseError::new(
                        ParseErrorKind::DanglingOperator("AND"),
                        span,
                    ));
                }
                continue;
            }
            if let Some(name) = directive_name(token) {
                if depth > 0 {
                    return Err(misplaced(token));
                }
                let span = token.span.clone();
                if let Some(Token {
                    kind: TokenKind::Term(term),
                    ..
                }) = self.next()
                {
                    self.directive(name, term, span)?;
                }
                continue;
            }
            operands.push(self.unary(depth)?);
        }
        Ok(Expr::and(operands))
    }

    fn at_operand_end(&self) -> bool {
        self.peek()
            .is_none_or(|t| matches!(t.kind, TokenKind::Close) || is_keyword(t, "OR"))
    }

    fn unary(&mut self, depth: usize) -> Result<Expr, ParseError> {
        let Some(token) = self.next() else {
            return Err(ParseError::new(
                ParseErrorKind::DanglingOperator("-"),
                self.end_span(),
            ));
        };
        if depth > MAX_DEPTH {
            return Err(ParseError::new(ParseErrorKind::TooDeep, token.span));
        }
        if matches!(token.kind, TokenKind::Minus) || is_keyword(&token, "NOT") {
            let operator = if matches!(token.kind, TokenKind::Minus) {
                "-"
            } else {
                "NOT"
            };
            if self.at_operand_end() {
                return Err(ParseError::new(
                    ParseErrorKind::DanglingOperator(operator),
                    token.span,
                ));
            }
            if let Some(next) = self.peek()
                && directive_name(next).is_some()
            {
                return Err(misplaced(next));
            }
            return Ok(self.unary(depth + 1)?.negate());
        }
        self.primary(token, depth)
    }

    fn primary(&mut self, token: Token, depth: usize) -> Result<Expr, ParseError> {
        match token.kind {
            TokenKind::Open => {
                let inner = self.or(depth + 1)?;
                match self.next() {
                    Some(Token {
                        kind: TokenKind::Close,
                        span,
                    }) => inner.ok_or_else(|| {
                        ParseError::new(
                            ParseErrorKind::EmptyParentheses,
                            token.span.start..span.end,
                        )
                    }),
                    _ => Err(ParseError::new(
                        ParseErrorKind::UnbalancedParenthesis,
                        token.span,
                    )),
                }
            }
            TokenKind::Close => Err(ParseError::new(
                ParseErrorKind::UnbalancedParenthesis,
                token.span,
            )),
            TokenKind::Term(term) => {
                let field = Field::from_name(&term.key).ok_or_else(|| {
                    ParseError::new(
                        ParseErrorKind::UnknownField(term.key.clone()),
                        token.span.start..token.span.start + term.key.len(),
                    )
                })?;
                Ok(Expr::Condition(value::condition(field, term, token.span)?))
            }
            TokenKind::Word(word) => Err(ParseError::new(
                ParseErrorKind::ExpectedCondition(word),
                token.span,
            )),
            TokenKind::Minus => Err(ParseError::new(
                ParseErrorKind::DanglingOperator("-"),
                token.span,
            )),
        }
    }

    fn directive(
        &mut self,
        name: &'static str,
        term: Term,
        span: Range<usize>,
    ) -> Result<(), ParseError> {
        if self.seen.contains(&name) {
            return Err(ParseError::new(
                ParseErrorKind::DuplicateDirective(name.to_owned()),
                span,
            ));
        }
        self.seen.push(name);
        if term.op != Op::Eq {
            return Err(ParseError::new(
                ParseErrorKind::UnsupportedOperator {
                    field: name.to_owned(),
                    operator: term.op.symbol(),
                },
                span,
            ));
        }
        let values = bare_values(name, term)?;
        match name {
            "group" => {
                self.query.group = names(
                    values,
                    Dimension::from_name,
                    ParseErrorKind::UnknownDimension,
                )?;
            }
            "metric" => {
                self.query.metrics =
                    names(values, Metric::from_name, ParseErrorKind::UnknownMetric)?;
            }
            "sort" => self.sort(values)?,
            _ => self.query.limit = Some(limit(&values, span)?),
        }
        Ok(())
    }

    fn sort(&mut self, values: Vec<RawValue>) -> Result<(), ParseError> {
        let mut keys = Vec::with_capacity(values.len());
        let mut last_explicit = false;
        for value in values {
            let (column, direction) = match value.text.rsplit_once(':') {
                Some((column, direction)) => (column, Some(direction_named(direction, &value)?)),
                None => (value.text.as_str(), None),
            };
            last_explicit = direction.is_some();
            keys.push(SortKey {
                column: column.to_owned(),
                direction: direction.unwrap_or_else(|| default_direction(column)),
            });
        }
        // `sort:hits desc` — a trailing direction word applies to the last key.
        if let Some(token) = self.peek()
            && let TokenKind::Word(word) = &token.kind
            && let Some(direction) = parse_direction(word)
            && !last_explicit
        {
            if let Some(last) = keys.last_mut() {
                last.direction = direction;
            }
            self.index += 1;
        }
        self.query.sort = keys;
        Ok(())
    }
}

fn misplaced(token: &Token) -> ParseError {
    let name = directive_name(token).unwrap_or("directive");
    ParseError::new(
        ParseErrorKind::MisplacedDirective(name.to_owned()),
        token.span.clone(),
    )
}

fn bare_values(name: &str, term: Term) -> Result<Vec<RawValue>, ParseError> {
    if let Some(quoted) = term.values.iter().find(|v| v.quoted) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidValue {
                field: name.to_owned(),
                expected: "unquoted names",
            },
            quoted.span.clone(),
        ));
    }
    Ok(term.values)
}

fn names<T>(
    values: Vec<RawValue>,
    lookup: impl Fn(&str) -> Option<T>,
    unknown: impl Fn(String) -> ParseErrorKind,
) -> Result<Vec<T>, ParseError> {
    values
        .into_iter()
        .map(|value| {
            lookup(&value.text).ok_or_else(|| ParseError::new(unknown(value.text), value.span))
        })
        .collect()
}

fn limit(values: &[RawValue], span: Range<usize>) -> Result<u64, ParseError> {
    let [value] = values else {
        return Err(ParseError::new(
            ParseErrorKind::ListNotAllowed("limit".to_owned()),
            span,
        ));
    };
    let digits_only = value.text.bytes().all(|b| b.is_ascii_digit());
    value
        .text
        .parse::<u64>()
        .ok()
        .filter(|n| digits_only && (1..=MAX_LIMIT).contains(n))
        .ok_or_else(|| {
            ParseError::new(
                ParseErrorKind::InvalidValue {
                    field: "limit".to_owned(),
                    expected: "a number between 1 and 1000000000",
                },
                value.span.clone(),
            )
        })
}

fn parse_direction(word: &str) -> Option<SortDirection> {
    match word.to_ascii_lowercase().as_str() {
        "asc" => Some(SortDirection::Asc),
        "desc" => Some(SortDirection::Desc),
        _ => None,
    }
}

fn direction_named(text: &str, value: &RawValue) -> Result<SortDirection, ParseError> {
    parse_direction(text).ok_or_else(|| {
        ParseError::new(
            ParseErrorKind::InvalidValue {
                field: "sort".to_owned(),
                expected: "column:asc or column:desc",
            },
            value.span.clone(),
        )
    })
}

/// Metrics sort largest first, everything else ascending.
fn default_direction(column: &str) -> SortDirection {
    if Metric::from_name(column).is_some() {
        SortDirection::Desc
    } else {
        SortDirection::Asc
    }
}
