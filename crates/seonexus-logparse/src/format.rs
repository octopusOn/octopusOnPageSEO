use std::fmt;

use crate::error::ParseError;
use crate::record::LogRecord;

/// A log format: turns one line into a [`LogRecord`].
///
/// Implementations must be cheap to call concurrently from many threads; imports parse
/// chunks of a file in parallel with a shared format instance.
pub trait LogFormat: Send + Sync + fmt::Debug {
    /// Stable machine identifier, e.g. `traefik-clf`. Stored with each site's settings.
    fn id(&self) -> &str;

    /// Human-readable name shown in the UI.
    fn display_name(&self) -> &str;

    /// How many fields the format captures. When several formats parse a sample equally
    /// well, detection prefers the most specific one.
    fn specificity(&self) -> usize;

    /// Parses a single line (without its line terminator).
    ///
    /// # Errors
    /// Returns a [`ParseError`] describing why the line does not match this format.
    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError>;
}
