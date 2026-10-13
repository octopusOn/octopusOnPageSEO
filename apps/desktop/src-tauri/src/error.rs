//! Errors as the frontend sees them.

use std::error::Error;

use seonexus_log_app::StoreError;
use seonexus_log_app::analysis::AnalysisError;
use seonexus_log_app::archive::ArchiveError;
use seonexus_log_app::bots::BotError;
use seonexus_log_app::catalog::CatalogError;
use seonexus_log_app::geo::GeoDataError;
use seonexus_log_app::import::ImportError;
use seonexus_log_app::inspect::InspectError;
use seonexus_log_app::query::QueryError;
use seonexus_log_app::saved::SavedQueryError;
use seonexus_log_app::settings::SettingsError;
use seonexus_log_app::table_file::TableFileError;
use serde::Serialize;

/// A failed command: a machine-readable kind plus the full error chain as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandError {
    /// `syntax`, `invalid-query`, `not-found`, `duplicate`, `busy`, `cancelled` or `error`.
    pub(crate) kind: &'static str,
    /// Human-readable message including causes.
    pub(crate) message: String,
    /// Byte range of a syntax error in `query`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) span: Option<[usize; 2]>,
    /// Query text a syntax error refers to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) query: Option<String>,
}

pub(crate) type CommandResult<T> = Result<T, CommandError>;

impl CommandError {
    fn new(kind: &'static str, error: &dyn Error) -> Self {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            let text = cause.to_string();
            if !message.contains(&text) {
                message.push_str(": ");
                message.push_str(&text);
            }
            source = cause.source();
        }
        Self {
            kind,
            message,
            span: None,
            query: None,
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: "error",
            message: message.into(),
            span: None,
            query: None,
        }
    }
}

fn store_kind(error: &StoreError) -> &'static str {
    match error {
        StoreError::SiteLocked(_) => "busy",
        StoreError::SiteNotFound(_)
        | StoreError::ClientNotFound(_)
        | StoreError::SavedQueryNotFound(_)
        | StoreError::ImportNotFound(_) => "not-found",
        StoreError::DuplicateClient(_)
        | StoreError::DuplicateSite(_)
        | StoreError::DuplicateSavedQuery(_) => "duplicate",
        _ => "error",
    }
}

impl From<StoreError> for CommandError {
    fn from(error: StoreError) -> Self {
        Self::new(store_kind(&error), &error)
    }
}

impl From<QueryError> for CommandError {
    fn from(error: QueryError) -> Self {
        let kind = match &error {
            QueryError::Parse { .. } => "syntax",
            QueryError::Invalid(_) | QueryError::UnknownTemplate { .. } => "invalid-query",
            QueryError::Store(store) => store_kind(store),
            _ => "error",
        };
        let mut command = Self::new(kind, &error);
        if let QueryError::Parse { text, source } = &error {
            let span = source.span();
            command.span = Some([span.start, span.end]);
            command.query = Some(text.clone());
            command.message = source.to_string();
        }
        command
    }
}

impl From<CatalogError> for CommandError {
    fn from(error: CatalogError) -> Self {
        let kind = match &error {
            CatalogError::Store(store) => store_kind(store),
            _ => "invalid-input",
        };
        Self::new(kind, &error)
    }
}

impl From<ImportError> for CommandError {
    fn from(error: ImportError) -> Self {
        let kind = match &error {
            ImportError::Cancelled => "cancelled",
            ImportError::Store(store) => store_kind(store),
            _ => "error",
        };
        Self::new(kind, &error)
    }
}

impl From<SettingsError> for CommandError {
    fn from(error: SettingsError) -> Self {
        let kind = match &error {
            SettingsError::Store(store) => store_kind(store),
            SettingsError::Corrupt(_) => "error",
            _ => "invalid-input",
        };
        Self::new(kind, &error)
    }
}

impl From<ArchiveError> for CommandError {
    fn from(error: ArchiveError) -> Self {
        let kind = match &error {
            ArchiveError::Store(store) | ArchiveError::Catalog(CatalogError::Store(store)) => {
                store_kind(store)
            }
            ArchiveError::Invalid(_) | ArchiveError::Settings(_) => "invalid-input",
            _ => "error",
        };
        Self::new(kind, &error)
    }
}

impl From<GeoDataError> for CommandError {
    fn from(error: GeoDataError) -> Self {
        Self::new("error", &error)
    }
}

impl From<InspectError> for CommandError {
    fn from(error: InspectError) -> Self {
        Self::new("error", &error)
    }
}

impl From<BotError> for CommandError {
    fn from(error: BotError) -> Self {
        let kind = match &error {
            BotError::Store(store) => store_kind(store),
            BotError::UnknownBot(_) | BotError::InvalidIp(_) | BotError::InvalidCustomBots(_) => {
                "invalid-input"
            }
            _ => "error",
        };
        Self::new(kind, &error)
    }
}

impl From<AnalysisError> for CommandError {
    fn from(error: AnalysisError) -> Self {
        match error {
            AnalysisError::Query(query) => query.into(),
            other => Self::new("error", &other),
        }
    }
}

impl From<SavedQueryError> for CommandError {
    fn from(error: SavedQueryError) -> Self {
        match error {
            SavedQueryError::Query(query) => query.into(),
            SavedQueryError::Store(store) => store.into(),
            SavedQueryError::InvalidName(_) | SavedQueryError::EmptyQuery => {
                Self::new("invalid-input", &error)
            }
            other => Self::new("error", &other),
        }
    }
}

impl From<TableFileError> for CommandError {
    fn from(error: TableFileError) -> Self {
        Self::new("error", &error)
    }
}

#[cfg(test)]
mod tests {
    use seonexus_log_app::query::{QuerySpec, check_query};

    use super::*;

    #[test]
    fn syntax_errors_carry_span_and_text() {
        let error = check_query(&QuerySpec::text("status:404 grop:url")).unwrap_err();
        let command = CommandError::from(error);
        assert_eq!(command.kind, "syntax");
        assert_eq!(command.span, Some([11, 15]));
        assert_eq!(command.query.as_deref(), Some("status:404 grop:url"));
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(json["kind"], "syntax");
    }

    #[test]
    fn store_errors_map_to_kinds() {
        let busy = CommandError::from(StoreError::SiteLocked("x".into()));
        assert_eq!(busy.kind, "busy");
        let missing = CommandError::from(StoreError::SiteNotFound("acme.com".into()));
        assert_eq!(missing.kind, "not-found");
        assert!(missing.message.contains("acme.com"));
        let json = serde_json::to_value(&missing).unwrap();
        assert!(json.get("span").is_none());
    }
}
