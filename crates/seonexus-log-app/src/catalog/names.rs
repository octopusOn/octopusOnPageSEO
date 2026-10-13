/// Longest accepted client or site name, in characters.
const MAX_NAME_CHARS: usize = 120;

/// A client or site name is not acceptable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NameError {
    /// Empty or whitespace only.
    #[error("name must not be empty")]
    Empty,
    /// Longer than the limit.
    #[error("name must be at most {MAX_NAME_CHARS} characters")]
    TooLong,
    /// Contains control characters such as newlines.
    #[error("name must not contain control characters")]
    ControlCharacter,
}

/// Trims and validates a user-supplied name.
pub(crate) fn normalize_name(raw: &str) -> Result<&str, NameError> {
    let name = raw.trim();
    if name.is_empty() {
        Err(NameError::Empty)
    } else if name.chars().count() > MAX_NAME_CHARS {
        Err(NameError::TooLong)
    } else if name.chars().any(char::is_control) {
        Err(NameError::ControlCharacter)
    } else {
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_validates() {
        assert_eq!(normalize_name("  Acme Ltd "), Ok("Acme Ltd"));
        assert_eq!(normalize_name("   "), Err(NameError::Empty));
        assert_eq!(normalize_name("a\nb"), Err(NameError::ControlCharacter));
        assert_eq!(normalize_name(&"x".repeat(121)), Err(NameError::TooLong));
    }
}
