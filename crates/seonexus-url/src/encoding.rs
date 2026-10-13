//! Percent-encoding normalization (RFC 3986 section 6.2.2).

/// Decodes escapes of unreserved characters (`%41` → `A`), upper-cases the hex digits of all
/// other escapes and encodes a stray `%` that starts no valid escape as `%25`. Afterwards every
/// `%` starts a valid escape, so decoding cannot form new escapes and the result is stable.
pub(crate) fn normalize_escapes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex_value),
                bytes.get(index + 2).copied().and_then(hex_value),
            )
        {
            let byte = high * 16 + low;
            if is_unreserved(byte) {
                out.push(char::from(byte));
            } else {
                out.push('%');
                out.push(char::from(HEX[usize::from(high)]));
                out.push(char::from(HEX[usize::from(low)]));
            }
            index += 3;
            continue;
        }
        if bytes[index] == b'%' {
            out.push_str("%25");
            index += 1;
            continue;
        }
        // Copy one (possibly multi-byte) character.
        let rest = &text[index..];
        let Some(c) = rest.chars().next() else {
            break;
        };
        out.push(c);
        index += c.len_utf8();
    }
    out
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_unreserved_and_uppercases_the_rest() {
        assert_eq!(normalize_escapes("/%7euser/%61b%2fc"), "/~user/ab%2Fc");
        assert_eq!(normalize_escapes("/caf%c3%a9"), "/caf%C3%A9");
        assert_eq!(normalize_escapes("/100%/x%2"), "/100%25/x%252");
        // Found by the idempotence property: decoding must not form a new escape.
        assert_eq!(normalize_escapes("/%%61A"), "/%25aA");
        assert_eq!(normalize_escapes("/%25aA"), "/%25aA");
        assert_eq!(normalize_escapes("/ğüş"), "/ğüş");
    }
}
