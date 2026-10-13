//! Space-separated lines whose fields may be wrapped in `"…"`, `[…]` or `{…}` (AWS load
//! balancers, HAProxy).

/// Splits `line` at runs of spaces; a field starting with `"`, `[` or `{` extends to the
/// matching closing character (a `\"` inside quotes does not close them). Fields are returned
/// with their delimiters; see [`unwrap`]. `None` for an unterminated field.
pub(crate) fn split_tokens(line: &str) -> Option<Vec<&str>> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            i += 1;
            continue;
        }
        let start = i;
        let close = match bytes[i] {
            b'"' => Some(b'"'),
            b'[' => Some(b']'),
            b'{' => Some(b'}'),
            _ => None,
        };
        match close {
            Some(close) => {
                i += 1;
                loop {
                    let byte = *bytes.get(i)?;
                    if byte == b'\\' && close == b'"' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    if byte == close {
                        break;
                    }
                }
            }
            None => {
                while i < bytes.len() && bytes[i] != b' ' {
                    i += 1;
                }
            }
        }
        tokens.push(line.get(start..i)?);
    }
    Some(tokens)
}

/// A token without its surrounding `"…"`, `[…]` or `{…}`.
pub(crate) fn unwrap(token: &str) -> &str {
    for (open, close) in [('"', '"'), ('[', ']'), ('{', '}')] {
        if token.len() >= 2 && token.starts_with(open) && token.ends_with(close) {
            return &token[1..token.len() - 1];
        }
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_wrapped_fields_together() {
        let tokens = split_tokens(r#"a  "b c \" d" [e f] {g|h i} j"#).unwrap();
        assert_eq!(tokens, ["a", r#""b c \" d""#, "[e f]", "{g|h i}", "j"]);
        assert_eq!(unwrap(tokens[2]), "e f");
        assert_eq!(unwrap("plain"), "plain");
        assert!(split_tokens(r#"a "open"#).is_none());
    }
}
