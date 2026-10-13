//! Apache `LogFormat` dialect.

use std::borrow::Cow;

use super::compiled::Token;
use super::error::TemplateError;
use super::spec::{DurationUnit, FieldSpec};

/// Splits an Apache format string into literals and `%` directive fields.
pub(super) fn tokenize(input: &str) -> Result<Vec<Token>, TemplateError> {
    let format = unescape(&strip_directive(input));
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut rest = format.as_str();

    while let Some(percent) = rest.find('%') {
        literal.push_str(&rest[..percent]);
        let directive = parse_directive(&rest[percent..])?;
        rest = &rest[percent + directive.len..];
        match directive.expand()? {
            Expansion::Literal(text) => literal.push_str(text),
            Expansion::Tokens(expanded) => {
                tokens.push(Token::literal(std::mem::take(&mut literal)));
                tokens.extend(expanded);
            }
        }
    }
    literal.push_str(rest);
    tokens.push(Token::literal(literal));
    Ok(tokens)
}

/// A parsed `%[conditions][<>]{arg}X` directive.
struct Directive<'a> {
    source: &'a str,
    argument: Option<&'a str>,
    letter: char,
    len: usize,
}

enum Expansion {
    Literal(&'static str),
    Tokens(Vec<Token>),
}

fn parse_directive(text: &str) -> Result<Directive<'_>, TemplateError> {
    let unterminated = || TemplateError::Unterminated(text.chars().take(16).collect());
    // Skip `%`, status conditions (`!400,501`) and the `<`/`>` request selectors.
    let mut index = 1 + text[1..]
        .bytes()
        .take_while(|b| b.is_ascii_digit() || matches!(b, b'!' | b',' | b'<' | b'>'))
        .count();
    let mut argument = None;
    if text[index..].starts_with('{') {
        let close = text[index..].find('}').ok_or_else(unterminated)?;
        argument = Some(&text[index + 1..index + close]);
        index += close + 1;
    }
    let letter = text[index..].chars().next().ok_or_else(unterminated)?;
    let len = index + letter.len_utf8();
    Ok(Directive {
        source: &text[..len],
        argument,
        letter,
        len,
    })
}

impl Directive<'_> {
    fn expand(&self) -> Result<Expansion, TemplateError> {
        if self.letter == '%' {
            return Ok(Expansion::Literal("%"));
        }
        if self.letter == 't' {
            return self.expand_time();
        }
        let spec = match (self.letter, self.argument) {
            ('h' | 'a', _) => FieldSpec::ClientIp,
            ('r', None) => FieldSpec::RequestLine,
            ('m', None) => FieldSpec::Method,
            ('U', None) => FieldSpec::Path,
            ('q', None) => FieldSpec::Query,
            ('H', None) => FieldSpec::Protocol,
            ('s', None) => FieldSpec::Status,
            ('b' | 'B' | 'O', None) => FieldSpec::Bytes,
            ('v' | 'V', None) => FieldSpec::Host,
            ('D', None) => FieldSpec::Duration(DurationUnit::Micros),
            ('T', unit) => FieldSpec::Duration(duration_unit(unit, self.source)?),
            ('i', Some(header)) => header_spec(header),
            _ => FieldSpec::Ignore,
        };
        Ok(Expansion::Tokens(vec![Token::field(spec, self.source)]))
    }

    /// `%t` logs `[dd/Mon/yyyy:HH:MM:SS +zzzz]` including the brackets.
    fn expand_time(&self) -> Result<Expansion, TemplateError> {
        let spec = match self.argument {
            None => {
                return Ok(Expansion::Tokens(vec![
                    Token::literal("["),
                    Token::field(FieldSpec::TimeClf, self.source),
                    Token::literal("]"),
                ]));
            }
            Some("sec") => FieldSpec::TimeEpochSeconds,
            Some(_) => {
                return Err(TemplateError::UnsupportedDirective {
                    directive: self.source.to_owned(),
                    hint: "custom strftime layouts are not supported; use %t or %{sec}t",
                });
            }
        };
        Ok(Expansion::Tokens(vec![Token::field(spec, self.source)]))
    }
}

fn duration_unit(argument: Option<&str>, source: &str) -> Result<DurationUnit, TemplateError> {
    match argument {
        None | Some("s") => Ok(DurationUnit::Seconds),
        Some("ms") => Ok(DurationUnit::Millis),
        Some("us") => Ok(DurationUnit::Micros),
        Some(_) => Err(TemplateError::UnsupportedDirective {
            directive: source.to_owned(),
            hint: "%T accepts only the s, ms or us units",
        }),
    }
}

fn header_spec(header: &str) -> FieldSpec {
    match header.to_ascii_lowercase().as_str() {
        "referer" => FieldSpec::Referer,
        "user-agent" => FieldSpec::UserAgent,
        "host" => FieldSpec::Host,
        "x-forwarded-for" => FieldSpec::ForwardedFor,
        _ => FieldSpec::Ignore,
    }
}

/// Accepts a pasted `LogFormat "…" nickname` line and returns the quoted format.
fn strip_directive(input: &str) -> Cow<'_, str> {
    let trimmed = input.trim();
    let Some(after_keyword) = trimmed.strip_prefix("LogFormat") else {
        return Cow::Borrowed(trimmed);
    };
    let Some(body) = after_keyword.trim_start().strip_prefix('"') else {
        return Cow::Borrowed(after_keyword.trim());
    };
    let mut end = body.len();
    let mut escaped = false;
    for (index, c) in body.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => {
                end = index;
                break;
            }
            _ => escaped = false,
        }
    }
    Cow::Owned(body[..end].to_owned())
}

/// Resolves the C-style escapes Apache allows inside format strings.
fn unescape(format: &str) -> String {
    let mut out = String::with_capacity(format.len());
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_directive_expands_to_bracketed_clf() {
        let tokens = tokenize("%t").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::literal(""),
                Token::literal("["),
                Token::field(FieldSpec::TimeClf, "%t"),
                Token::literal("]"),
                Token::literal(""),
            ]
        );
    }

    #[test]
    fn modifiers_and_headers() {
        let tokens = tokenize("%>s %400,501{User-Agent}i %{ms}T %%").unwrap();
        let specs: Vec<_> = tokens
            .iter()
            .filter_map(|t| match t {
                Token::Field { spec, .. } => Some(*spec),
                Token::Literal(_) => None,
            })
            .collect();
        assert_eq!(
            specs,
            vec![
                FieldSpec::Status,
                FieldSpec::UserAgent,
                FieldSpec::Duration(DurationUnit::Millis)
            ]
        );
        assert_eq!(tokens.last(), Some(&Token::literal(" %")));
    }

    #[test]
    fn strips_pasted_directive_and_unescapes_quotes() {
        let line = r#"LogFormat "%h \"%r\" %>s" mini"#;
        assert_eq!(unescape(&strip_directive(line)), r#"%h "%r" %>s"#);
    }

    #[test]
    fn rejects_custom_strftime() {
        assert!(matches!(
            tokenize("%{%d/%b/%Y}t"),
            Err(TemplateError::UnsupportedDirective { .. })
        ));
    }
}
