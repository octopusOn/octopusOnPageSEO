//! Compiled templates: a literal prefix followed by `(field, terminator)` items.

use super::builder::RecordBuilder;
use super::error::TemplateError;
use super::spec::FieldSpec;
use crate::error::{Field, ParseError, ParseErrorKind};
use crate::record::LogRecord;
use crate::scan::Literal;

/// Source-level template element produced by presets and dialect tokenizers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    Literal(String),
    Field { spec: FieldSpec, source: String },
}

impl Token {
    pub(crate) fn literal(text: impl Into<String>) -> Self {
        Self::Literal(text.into())
    }

    pub(crate) fn field(spec: FieldSpec, source: impl Into<String>) -> Self {
        Self::Field {
            spec,
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct Item {
    spec: FieldSpec,
    terminator: Option<Literal>,
    /// The value sits between quotes, so escaped quotes must be skipped while scanning.
    quoted: bool,
}

/// A validated, ready-to-run line template.
#[derive(Debug, Clone)]
pub(crate) struct Template {
    prefix: Literal,
    items: Vec<Item>,
}

impl Template {
    /// Validates tokens and precompiles separators.
    pub(crate) fn compile(tokens: Vec<Token>) -> Result<Self, TemplateError> {
        let tokens = merge_adjacent_literals(tokens);
        let mut iter = tokens.into_iter().peekable();
        let prefix = match iter.next_if(|t| matches!(t, Token::Literal(_))) {
            Some(Token::Literal(text)) => text,
            _ => String::new(),
        };

        let mut items = Vec::new();
        let mut previous_literal = prefix.clone();
        while let Some(token) = iter.next() {
            let Token::Field { spec, source } = token else {
                continue;
            };
            let terminator = match iter.next_if(|t| matches!(t, Token::Literal(_))) {
                Some(Token::Literal(text)) => Some(text),
                _ => None,
            };
            if terminator.is_none()
                && let Some(Token::Field { source: next, .. }) = iter.peek()
            {
                return Err(TemplateError::AdjacentFields(source, next.clone()));
            }
            let terminator = terminator.map(|text| Literal::new(&text));
            let quoted = previous_literal.ends_with('"')
                && terminator.as_ref().is_some_and(Literal::starts_with_quote);
            previous_literal = terminator
                .as_ref()
                .map(|l| l.as_str().to_owned())
                .unwrap_or_default();
            items.push(Item {
                spec,
                terminator,
                quoted,
            });
        }

        let template = Self {
            prefix: Literal::new(&prefix),
            items,
        };
        template.validate()?;
        Ok(template)
    }

    fn validate(&self) -> Result<(), TemplateError> {
        const REQUIRED: [Field; 4] = [
            Field::Timestamp,
            Field::ClientIp,
            Field::Request,
            Field::Status,
        ];
        if self.items.is_empty() {
            return Err(TemplateError::Empty);
        }
        let missing = REQUIRED
            .into_iter()
            .find(|field| !self.items.iter().any(|item| item.spec.satisfies(*field)));
        missing.map_or(Ok(()), |field| Err(TemplateError::MissingRequired(field)))
    }

    /// Number of captured (non-ignored) fields; used to prefer richer formats on ties.
    pub(crate) fn specificity(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.spec != FieldSpec::Ignore)
            .count()
    }

    /// Parses one line. Trailing whitespace and line terminators are ignored.
    pub(crate) fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let line = line.trim_end();
        if line.is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        let mismatch = |at| ParseError::new(ParseErrorKind::LayoutMismatch, at);
        if !line.starts_with(self.prefix.as_str()) {
            return Err(mismatch(0));
        }

        let bytes = line.as_bytes();
        let mut pos = self.prefix.len();
        let mut record = RecordBuilder::default();
        for item in &self.items {
            let rest = &bytes[pos..];
            let (value_len, separator_len) = match &item.terminator {
                Some(literal) => {
                    let found = if item.quoted {
                        literal.find_unescaped(rest)
                    } else {
                        literal.find(rest)
                    };
                    (found.ok_or(mismatch(pos))?, literal.len())
                }
                None if item.spec.is_single_token() => (
                    rest.iter()
                        .position(u8::is_ascii_whitespace)
                        .unwrap_or(rest.len()),
                    0,
                ),
                None => (rest.len(), 0),
            };
            let value = line.get(pos..pos + value_len).ok_or(mismatch(pos))?;
            record.apply(item.spec, value, pos)?;
            pos += value_len + separator_len;
        }
        if pos < line.len() {
            return Err(ParseError::new(ParseErrorKind::TrailingData, pos));
        }
        record.finish(line.len())
    }
}

fn merge_adjacent_literals(tokens: Vec<Token>) -> Vec<Token> {
    let mut merged: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens {
        match (merged.last_mut(), token) {
            (_, Token::Literal(text)) if text.is_empty() => {}
            (Some(Token::Literal(previous)), Token::Literal(text)) => previous.push_str(&text),
            (_, token) => merged.push(token),
        }
    }
    merged
}
