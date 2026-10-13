//! Nginx `log_format` dialect.

use std::borrow::Cow;

use super::compiled::Token;
use super::error::TemplateError;
use super::spec::{DurationUnit, FieldSpec};

/// Splits an Nginx format string into literals and `$variable` fields.
pub(super) fn tokenize(input: &str) -> Result<Vec<Token>, TemplateError> {
    let format = strip_directive(input);
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut rest = format.as_ref();

    while let Some(dollar) = rest.find('$') {
        literal.push_str(&rest[..dollar]);
        let after = &rest[dollar + 1..];
        let (name, consumed) = variable_name(after)?;
        if name.is_empty() {
            literal.push('$');
        } else {
            tokens.push(Token::literal(std::mem::take(&mut literal)));
            tokens.push(Token::field(variable_spec(name), format!("${name}")));
        }
        rest = &after[consumed..];
    }
    literal.push_str(rest);
    tokens.push(Token::literal(literal));
    Ok(tokens)
}

/// Returns the variable name after `$` and how many bytes it occupies (`name` or `{name}`).
fn variable_name(after: &str) -> Result<(&str, usize), TemplateError> {
    if let Some(braced) = after.strip_prefix('{') {
        let end = braced
            .find('}')
            .ok_or_else(|| TemplateError::Unterminated(format!("${{{braced}")))?;
        return Ok((&braced[..end], end + 2));
    }
    let len = after
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    Ok((&after[..len], len))
}

/// Accepts a pasted `log_format name [escape=…] '…' '…';` directive and returns the
/// concatenated quoted parts. Bare formats are returned unchanged.
fn strip_directive(input: &str) -> Cow<'_, str> {
    let trimmed = input.trim().trim_end_matches(';').trim_end();
    let is_directive = trimmed.starts_with("log_format") || trimmed.starts_with('\'');
    if !is_directive {
        return Cow::Borrowed(trimmed);
    }
    let mut format = String::with_capacity(trimmed.len());
    let mut open_quote = None;
    for c in trimmed.chars() {
        match open_quote {
            None if c == '\'' || c == '"' => open_quote = Some(c),
            None => {}
            Some(quote) if c == quote => open_quote = None,
            Some(_) => format.push(c),
        }
    }
    Cow::Owned(format)
}

fn variable_spec(name: &str) -> FieldSpec {
    match name {
        "remote_addr" | "realip_remote_addr" | "binary_remote_addr_text" => FieldSpec::ClientIp,
        "http_x_forwarded_for" | "proxy_add_x_forwarded_for" => FieldSpec::ForwardedFor,
        "time_local" => FieldSpec::TimeClf,
        "time_iso8601" => FieldSpec::TimeIso8601,
        "msec" => FieldSpec::TimeEpochSeconds,
        "request" => FieldSpec::RequestLine,
        "request_method" => FieldSpec::Method,
        "request_uri" => FieldSpec::Target,
        "uri" | "document_uri" => FieldSpec::Path,
        "args" | "query_string" => FieldSpec::Query,
        "server_protocol" => FieldSpec::Protocol,
        "status" => FieldSpec::Status,
        "body_bytes_sent" | "bytes_sent" => FieldSpec::Bytes,
        "http_referer" => FieldSpec::Referer,
        "http_user_agent" => FieldSpec::UserAgent,
        "host" | "http_host" | "server_name" => FieldSpec::Host,
        "request_time" => FieldSpec::Duration(DurationUnit::Seconds),
        "upstream_addr" => FieldSpec::Upstream,
        _ => FieldSpec::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_plain_and_braced_variables() {
        let tokens = tokenize("${remote_addr}:$status $$").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::literal(""),
                Token::field(FieldSpec::ClientIp, "$remote_addr"),
                Token::literal(":"),
                Token::field(FieldSpec::Status, "$status"),
                Token::literal(" $$"),
            ]
        );
    }

    #[test]
    fn strips_pasted_directive() {
        let directive = "log_format main escape=default '$remote_addr - [$time_local] '\n    '\"$request\" $status';";
        assert_eq!(
            strip_directive(directive),
            "$remote_addr - [$time_local] \"$request\" $status"
        );
    }

    #[test]
    fn unterminated_brace_is_an_error() {
        assert!(matches!(
            tokenize("${remote_addr"),
            Err(TemplateError::Unterminated(_))
        ));
    }
}
