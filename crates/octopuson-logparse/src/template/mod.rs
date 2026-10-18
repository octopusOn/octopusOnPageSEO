//! Line-oriented log formats described by templates.

mod apache;
mod builder;
mod compiled;
mod error;
mod nginx;
pub(crate) mod presets;
mod spec;

pub use error::TemplateError;

use crate::error::ParseError;
use crate::format::LogFormat;
use crate::record::LogRecord;
use compiled::{Template, Token};

/// A log format defined by a field template.
///
/// Built-in presets (`traefik-clf`, `combined`, `common`) and user formats compiled from an
/// Nginx `log_format` or Apache `LogFormat` string all use this type.
#[derive(Debug, Clone)]
pub struct TemplateFormat {
    id: String,
    display_name: String,
    template: Template,
}

impl TemplateFormat {
    /// Compiles an Nginx `log_format` string.
    ///
    /// Accepts either the bare format (`$remote_addr - $remote_user [$time_local] …`) or a
    /// full `log_format name '…' '…';` directive copied from `nginx.conf`.
    ///
    /// # Errors
    /// Returns [`TemplateError`] if the format is ambiguous, unsupported or lacks a field
    /// required for analysis (timestamp, client address, request, status).
    pub fn from_nginx(
        id: impl Into<String>,
        display_name: impl Into<String>,
        log_format: &str,
    ) -> Result<Self, TemplateError> {
        Self::compile(id, display_name, nginx::tokenize(log_format)?)
    }

    /// Compiles an Apache `LogFormat` string such as
    /// `%h %l %u %t "%r" %>s %b "%{Referer}i" "%{User-Agent}i"`.
    ///
    /// Accepts the bare format or a full `LogFormat "…" nickname` line from `httpd.conf`.
    ///
    /// # Errors
    /// Returns [`TemplateError`] if the format is ambiguous, unsupported or lacks a field
    /// required for analysis (timestamp, client address, request, status).
    pub fn from_apache(
        id: impl Into<String>,
        display_name: impl Into<String>,
        log_format: &str,
    ) -> Result<Self, TemplateError> {
        Self::compile(id, display_name, apache::tokenize(log_format)?)
    }

    pub(crate) fn compile(
        id: impl Into<String>,
        display_name: impl Into<String>,
        tokens: Vec<Token>,
    ) -> Result<Self, TemplateError> {
        Ok(Self {
            id: id.into(),
            display_name: display_name.into(),
            template: Template::compile(tokens)?,
        })
    }
}

impl LogFormat for TemplateFormat {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn specificity(&self) -> usize {
        self.template.specificity()
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        self.template.parse(line)
    }
}

#[cfg(test)]
mod tests;
