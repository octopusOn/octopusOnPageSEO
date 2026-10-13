//! Tokenizer of the query language.
//!
//! `field:value` pairs are lexed as one token so that values may contain characters such as
//! `-`, `:` or `*` without quoting; only whitespace, `(`, `)`, `,` and `"` end a bare value.

use std::ops::Range;

use super::error::{ParseError, ParseErrorKind};

/// Comparison written after `field:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Eq,
    Regex,
    Gt,
    Ge,
    Lt,
    Le,
}

impl Op {
    pub(super) const fn symbol(self) -> &'static str {
        match self {
            Self::Eq => ":",
            Self::Regex => ":~",
            Self::Gt => ":>",
            Self::Ge => ":>=",
            Self::Lt => ":<",
            Self::Le => ":<=",
        }
    }
}

/// One value of a `field:value[,value…]` term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RawValue {
    pub(super) text: String,
    pub(super) quoted: bool,
    pub(super) span: Range<usize>,
}

/// A `field<op>values` term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Term {
    pub(super) key: String,
    pub(super) op: Op,
    pub(super) values: Vec<RawValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TokenKind {
    Open,
    Close,
    Minus,
    Word(String),
    Term(Term),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Token {
    pub(super) kind: TokenKind,
    pub(super) span: Range<usize>,
}

/// Characters allowed in an unquoted value.
pub(super) fn is_bare_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '(' | ')' | ',' | '"')
}

pub(super) fn tokenize(text: &str) -> Result<Vec<Token>, ParseError> {
    let mut lexer = Lexer { text, pos: 0 };
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token()? {
        tokens.push(token);
    }
    Ok(tokens)
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn peek_second(&self) -> Option<char> {
        self.text[self.pos..].chars().nth(1)
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn eat_while(&mut self, accept: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&accept) {
            self.bump();
        }
    }

    fn error<T>(kind: ParseErrorKind, span: Range<usize>) -> Result<T, ParseError> {
        Err(ParseError::new(kind, span))
    }

    fn next_token(&mut self) -> Result<Option<Token>, ParseError> {
        self.eat_while(char::is_whitespace);
        let start = self.pos;
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        let kind = match c {
            '(' => {
                self.bump();
                TokenKind::Open
            }
            ')' => {
                self.bump();
                TokenKind::Close
            }
            '-' if self.peek_second().is_some_and(|next| !next.is_whitespace()) => {
                self.bump();
                TokenKind::Minus
            }
            '-' => return Self::error(ParseErrorKind::DanglingOperator("-"), start..start + 1),
            c if c.is_ascii_alphabetic() || c == '_' => self.word_or_term(start)?,
            c if is_bare_char(c) => {
                self.eat_while(is_bare_char);
                TokenKind::Word(self.text[start..self.pos].to_owned())
            }
            c => {
                return Self::error(
                    ParseErrorKind::UnexpectedChar(c),
                    start..start + c.len_utf8(),
                );
            }
        };
        Ok(Some(Token {
            kind,
            span: start..self.pos,
        }))
    }

    fn word_or_term(&mut self, start: usize) -> Result<TokenKind, ParseError> {
        self.eat_while(|c| c.is_ascii_alphanumeric() || c == '_');
        if self.peek() != Some(':') {
            self.eat_while(is_bare_char);
            return Ok(TokenKind::Word(self.text[start..self.pos].to_owned()));
        }
        let key = self.text[start..self.pos].to_owned();
        self.bump();
        let op = self.operator();
        let values = self.values()?;
        match self.peek() {
            None | Some(')') => {}
            Some(c) if c.is_whitespace() => {}
            Some(_) => {
                return Self::error(ParseErrorKind::MissingSeparator, self.pos..self.pos + 1);
            }
        }
        Ok(TokenKind::Term(Term { key, op, values }))
    }

    fn operator(&mut self) -> Op {
        let rest = &self.text[self.pos..];
        let (op, len) = if rest.starts_with('~') {
            (Op::Regex, 1)
        } else if rest.starts_with(">=") {
            (Op::Ge, 2)
        } else if rest.starts_with('>') {
            (Op::Gt, 1)
        } else if rest.starts_with("<=") {
            (Op::Le, 2)
        } else if rest.starts_with('<') {
            (Op::Lt, 1)
        } else {
            (Op::Eq, 0)
        };
        self.pos += len;
        op
    }

    fn values(&mut self) -> Result<Vec<RawValue>, ParseError> {
        let mut values = vec![self.value()?];
        while self.peek() == Some(',') {
            self.bump();
            values.push(self.value()?);
        }
        Ok(values)
    }

    fn value(&mut self) -> Result<RawValue, ParseError> {
        let start = self.pos;
        if self.peek() == Some('"') {
            return self.quoted(start);
        }
        self.eat_while(is_bare_char);
        if self.pos == start {
            return Self::error(ParseErrorKind::MissingValue, start..start);
        }
        Ok(RawValue {
            text: self.text[start..self.pos].to_owned(),
            quoted: false,
            span: start..self.pos,
        })
    }

    /// Reads `"…"`; `\"` and `\\` are escapes, any other backslash is kept literally so
    /// regular expressions such as `"\.php$"` need no doubling.
    fn quoted(&mut self, start: usize) -> Result<RawValue, ParseError> {
        self.bump();
        let mut text = String::new();
        loop {
            match self.peek() {
                None => {
                    return Self::error(ParseErrorKind::UnterminatedQuote, start..self.text.len());
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') if matches!(self.peek_second(), Some('"' | '\\')) => {
                    self.bump();
                    text.extend(self.peek());
                    self.bump();
                }
                Some(c) => {
                    text.push(c);
                    self.bump();
                }
            }
        }
        Ok(RawValue {
            text,
            quoted: true,
            span: start..self.pos,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<TokenKind> {
        tokenize(text)
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    fn term(key: &str, op: Op, values: &[&str]) -> TokenKind {
        TokenKind::Term(Term {
            key: key.into(),
            op,
            values: values
                .iter()
                .map(|v| RawValue {
                    text: (*v).into(),
                    quoted: false,
                    span: 0..0,
                })
                .collect(),
        })
    }

    fn strip_spans(kinds: Vec<TokenKind>) -> Vec<TokenKind> {
        kinds
            .into_iter()
            .map(|kind| match kind {
                TokenKind::Term(mut term) => {
                    for value in &mut term.values {
                        value.span = 0..0;
                        value.quoted = false;
                    }
                    TokenKind::Term(term)
                }
                other => other,
            })
            .collect()
    }

    #[test]
    fn lexes_terms_operators_and_groups() {
        let tokens = strip_spans(kinds("-(status:301,302 OR bytes:>=10) url:/a-b:c*"));
        assert_eq!(
            tokens,
            vec![
                TokenKind::Minus,
                TokenKind::Open,
                term("status", Op::Eq, &["301", "302"]),
                TokenKind::Word("OR".into()),
                term("bytes", Op::Ge, &["10"]),
                TokenKind::Close,
                term("url", Op::Eq, &["/a-b:c*"]),
            ]
        );
    }

    #[test]
    fn quoted_values_keep_regex_backslashes() {
        let tokens = kinds(r#"ua:~"\.php$ \"x\" \\""#);
        let TokenKind::Term(term) = &tokens[0] else {
            panic!("expected a term");
        };
        assert_eq!(term.op, Op::Regex);
        assert_eq!(term.values[0].text, r#"\.php$ "x" \"#);
        assert!(term.values[0].quoted);
    }

    #[test]
    fn reports_positions() {
        let error = tokenize("url:\"abc").unwrap_err();
        assert_eq!(error.kind(), &ParseErrorKind::UnterminatedQuote);
        assert_eq!(error.span(), 4..8);
        let error = tokenize("status: 404").unwrap_err();
        assert_eq!(error.kind(), &ParseErrorKind::MissingValue);
        assert_eq!(error.span(), 7..7);
        let error = tokenize("url:\"a\"b").unwrap_err();
        assert_eq!(error.kind(), &ParseErrorKind::MissingSeparator);
    }
}
