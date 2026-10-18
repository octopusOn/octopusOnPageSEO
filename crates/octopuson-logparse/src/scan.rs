//! Low-level byte scanning used by the template engine.

use memchr::{memchr, memchr2, memmem};

/// A literal separator between template fields with a precompiled substring searcher.
#[derive(Debug, Clone)]
pub(crate) struct Literal {
    text: Box<str>,
    finder: memmem::Finder<'static>,
}

impl Literal {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            finder: memmem::Finder::new(text.as_bytes()).into_owned(),
            text: text.into(),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn starts_with_quote(&self) -> bool {
        self.text.starts_with('"')
    }

    /// Finds the first occurrence of the literal in `haystack`.
    pub(crate) fn find(&self, haystack: &[u8]) -> Option<usize> {
        match self.text.as_bytes() {
            [single] => memchr(*single, haystack),
            _ => self.finder.find(haystack),
        }
    }

    /// Finds the first occurrence that is not escaped by a preceding backslash.
    ///
    /// Used for values inside quotes: Apache escapes embedded quotes as `\"`, so a quote
    /// preceded by a backslash belongs to the value rather than terminating it.
    pub(crate) fn find_unescaped(&self, haystack: &[u8]) -> Option<usize> {
        let needle = self.text.as_bytes();
        let first = *needle.first()?;
        if first == b'\\' {
            return self.find(haystack);
        }
        let mut cursor = 0;
        while cursor < haystack.len() {
            let at = cursor + memchr2(first, b'\\', &haystack[cursor..])?;
            if haystack[at] == b'\\' {
                cursor = at + 2;
                continue;
            }
            if haystack[at..].starts_with(needle) {
                return Some(at);
            }
            cursor = at + 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_single_and_multi_byte() {
        assert_eq!(Literal::new(" ").find(b"ab cd"), Some(2));
        assert_eq!(Literal::new("\" \"").find(b"x\" \"y"), Some(1));
        assert_eq!(Literal::new("ms").find(b"12"), None);
    }

    #[test]
    fn find_unescaped_skips_escaped_quotes() {
        let literal = Literal::new("\" ");
        assert_eq!(literal.find_unescaped(br#"say \"hi\" now" next"#), Some(14));
        assert_eq!(literal.find_unescaped(br"trailing backslash \"), None);
        assert_eq!(literal.find_unescaped(b"\\\\\" x"), Some(2));
    }
}
