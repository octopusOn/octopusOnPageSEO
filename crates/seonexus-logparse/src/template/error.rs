use crate::error::Field;

/// A custom log format string could not be compiled.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TemplateError {
    /// The format string contains no fields.
    #[error("the log format contains no fields")]
    Empty,
    /// Two fields follow each other without a separator, so their boundary is ambiguous.
    #[error("`{0}` and `{1}` are adjacent without a separator; their boundary is ambiguous")]
    AdjacentFields(String, String),
    /// A directive or variable is recognised but cannot be supported.
    #[error("unsupported directive `{directive}`: {hint}")]
    UnsupportedDirective {
        /// The directive as written.
        directive: String,
        /// How to fix it.
        hint: &'static str,
    },
    /// A `${…}` or `%{…}` argument is not closed.
    #[error("unterminated directive starting at `{0}`")]
    Unterminated(String),
    /// The format does not log a field required for analysis.
    #[error("the log format must include the {0}")]
    MissingRequired(Field),
}
