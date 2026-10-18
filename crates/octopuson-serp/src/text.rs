//! Small text helpers shared by the parsers.

/// Collapses runs of whitespace (including no-break spaces) into one space and trims.
pub(crate) fn squash(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() || ch == '\u{200b}' {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

/// `None` for an empty string after [`squash`].
pub(crate) fn non_empty(text: &str) -> Option<String> {
    let text = squash(text);
    (!text.is_empty()).then_some(text)
}

/// The first number in `text`, allowing `,`, `.` and spaces as group separators
/// (`About 1,230,000 results (0.42 seconds)` → 1230000; `Yaklaşık 1.230.000 sonuç` → 1230000).
pub(crate) fn first_count(text: &str) -> Option<u64> {
    let chars: Vec<char> = text.chars().collect();
    let start = chars.iter().position(char::is_ascii_digit)?;
    let mut digits = String::new();
    let mut index = start;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if is_group_separator(ch)
            && chars.get(index + 1).is_some_and(char::is_ascii_digit)
            && chars.get(index + 2).is_some_and(char::is_ascii_digit)
            && chars.get(index + 3).is_some_and(char::is_ascii_digit)
            && !chars.get(index + 4).is_some_and(char::is_ascii_digit)
        {
            // A group separator is followed by exactly three digits.
        } else {
            break;
        }
        index += 1;
    }
    digits.parse().ok()
}

fn is_group_separator(ch: char) -> bool {
    matches!(ch, ',' | '.' | ' ' | '\u{a0}' | '\u{202f}' | '\'')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squashes_whitespace() {
        assert_eq!(squash("  a \n\t b\u{a0}c  "), "a b c");
        assert_eq!(non_empty(" \n "), None);
    }

    #[test]
    fn reads_result_counts() {
        assert_eq!(
            first_count("About 1,230,000 results (0.42 seconds)"),
            Some(1_230_000)
        );
        assert_eq!(
            first_count("Yaklaşık 1.230.000 sonuç bulundu (0,42 saniye)"),
            Some(1_230_000)
        );
        assert_eq!(first_count("12 300 results"), Some(12_300));
        assert_eq!(first_count("Page 2 of about 4,500 results"), Some(2));
        assert_eq!(first_count("no digits"), None);
    }
}
