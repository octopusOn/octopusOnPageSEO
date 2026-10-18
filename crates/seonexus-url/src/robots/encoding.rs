//! Comparable REP paths: encoded UTF-8, uppercase escapes, decoded unreserved ASCII.

use std::borrow::Cow;

const HEX: &[u8; 16] = b"0123456789ABCDEF";

pub(super) fn normalize(path: &str) -> Cow<'_, str> {
    if path.is_ascii() && !path.contains('%') {
        return Cow::Borrowed(path);
    }
    // Reuse URL-key normalization, including its handling of stray '%' characters.
    // In particular, /%%61A must not turn into a new /%aA escape on a later pass.
    let escaped = crate::encoding::normalize_escapes(path);
    if escaped.is_ascii() {
        return if escaped == path {
            Cow::Borrowed(path)
        } else {
            Cow::Owned(escaped)
        };
    }
    let mut normalized = String::with_capacity(escaped.len());
    for byte in escaped.bytes() {
        if byte.is_ascii() {
            normalized.push(char::from(byte));
        } else {
            push_escape(&mut normalized, byte);
        }
    }
    Cow::Owned(normalized)
}

fn push_escape(target: &mut String, byte: u8) {
    target.push('%');
    target.push(char::from(HEX[usize::from(byte >> 4)]));
    target.push(char::from(HEX[usize::from(byte & 15)]));
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn normalizing_twice_preserves_the_same_octets(path in ".{0,200}") {
            let normalized = normalize(&path);
            let repeated = normalize(&normalized);
            prop_assert_eq!(repeated.as_ref(), normalized.as_ref());
            prop_assert!(normalized.is_ascii());
        }
    }

    #[test]
    fn canonical_ascii_is_returned_unchanged_and_stray_escapes_do_not_merge() {
        for path in ["/normal/path?a=1&b=2", "/a%2Fb", "/a%252F"] {
            assert!(matches!(normalize(path), Cow::Borrowed(_)), "{path}");
        }
        for (input, expected) in [
            ("/a%", "/a%25"),
            ("/a%2", "/a%252"),
            ("/a%not-hex", "/a%25not-hex"),
            ("/%%61A", "/%25aA"),
        ] {
            assert_eq!(normalize(input), expected);
            assert_eq!(normalize(expected), expected);
        }
        assert_eq!(normalize("/café/%7e/%2f"), "/caf%C3%A9/~/%2F");
    }
}
