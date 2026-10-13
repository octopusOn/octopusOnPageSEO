//! Query use-cases: run a filter or report against a site, stream the result table into a
//! sink or export it to a file, and list the built-in report templates.

mod error;
mod export;
mod spec;
mod suggest;

pub use error::QueryError;
pub use export::{ExportReport, ExportRequest, export_query};
pub use seonexus_query::sql::Page;
pub use seonexus_query::{Dimension, Field, FieldKind, Metric, Query};
pub use seonexus_report::ExportFormat;
pub use spec::{QuerySpec, TemplateInfo, templates};
pub use suggest::{MAX_SUGGESTIONS, SuggestRequest, ValueSuggestion, suggest_values};

use std::time::Instant;

use seonexus_core::Date;
use seonexus_core::table::{Column, TableSink};
use seonexus_query::sql::{CompileContext, compile};
use seonexus_query::to_dsl;
use seonexus_store::Catalog;
use serde::Serialize;

use crate::site_access::open_reader;
use crate::workspace::Workspace;

/// Input of [`run_query`].
#[derive(Debug, Clone)]
pub struct QueryRequest {
    /// Site name.
    pub site: String,
    /// What to run.
    pub spec: QuerySpec,
    /// Window of rows to return; `None` returns every row (up to the query's `limit`).
    pub page: Option<Page>,
}

/// Metadata of an executed query; the rows themselves went to the sink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueryOutcome {
    /// Site name.
    pub site: String,
    /// The executed query in canonical query-language form.
    pub query: String,
    /// Data source chosen by the planner, e.g. `daily URL rollup`.
    pub source: &'static str,
    /// Result columns.
    pub columns: Vec<Column>,
    /// Rows written to the sink.
    pub rows: u64,
    /// Execution time in milliseconds.
    pub elapsed_ms: u64,
}

/// Resolves a spec, looking saved queries up in the catalog.
pub(crate) fn resolve_spec(workspace: &Workspace, spec: &QuerySpec) -> Result<Query, QueryError> {
    if let QuerySpec::Saved { name, refine } = spec {
        let saved = Catalog::open(workspace.data_dir())?.saved_query(name.trim())?;
        return spec::with_refinement(&saved.query, refine.as_deref());
    }
    spec.resolve()
}

/// Parses query text into its syntax tree (for editors that show it as a form).
///
/// # Errors
/// [`QueryError::Parse`].
pub fn parse_query(text: &str) -> Result<Query, QueryError> {
    QuerySpec::text(text).resolve()
}

/// A filterable field as shown by query editors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldInfo {
    /// Query-language name.
    pub name: &'static str,
    /// `text`, `integer`, `date` or `boolean`.
    pub kind: &'static str,
}

/// Everything a query editor offers: fields, grouping keys and aggregates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuerySchema {
    /// Filterable fields.
    pub fields: Vec<FieldInfo>,
    /// `group:` names.
    pub dimensions: Vec<&'static str>,
    /// `metric:` names.
    pub metrics: Vec<&'static str>,
}

/// The query language's vocabulary.
pub fn query_schema() -> QuerySchema {
    QuerySchema {
        fields: Field::ALL
            .iter()
            .map(|field| FieldInfo {
                name: field.name(),
                kind: match field.kind() {
                    FieldKind::Text => "text",
                    FieldKind::Integer => "integer",
                    FieldKind::Date => "date",
                    FieldKind::Boolean => "boolean",
                },
            })
            .collect(),
        dimensions: Dimension::ALL.iter().map(|d| d.name()).collect(),
        metrics: Metric::ALL.iter().map(|m| m.name()).collect(),
    }
}

/// A query that passed syntax and validation checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckedQuery {
    /// Canonical query-language form.
    pub query: String,
    /// Data source the planner would use.
    pub source: &'static str,
    /// Result columns.
    pub columns: Vec<Column>,
}

/// Parses, validates and plans a query without touching any site (for editors that check
/// queries while they are typed).
///
/// # Errors
/// [`QueryError::Parse`], [`QueryError::UnknownTemplate`] or [`QueryError::Invalid`].
pub fn check_query(spec: &QuerySpec) -> Result<CheckedQuery, QueryError> {
    let query = spec.resolve()?;
    let compiled = compile(
        &query,
        CompileContext {
            today: Date::today(),
        },
    )?;
    Ok(CheckedQuery {
        query: to_dsl(&query),
        source: compiled.source.label(),
        columns: compiled.columns,
    })
}

/// Runs a query against a site and streams the rows into `sink`.
///
/// A site without imported data yields an empty table with the query's columns.
///
/// # Errors
/// See [`QueryError`]. Syntax and validation errors are reported before the site is opened.
pub fn run_query(
    workspace: &Workspace,
    request: &QueryRequest,
    sink: &mut dyn TableSink,
) -> Result<QueryOutcome, QueryError> {
    let started = Instant::now();
    let query = resolve_spec(workspace, &request.spec)?;
    let compiled = compile(
        &query,
        CompileContext {
            today: Date::today(),
        },
    )?;
    let site = Catalog::open(workspace.data_dir())?.site_by_name(&request.site)?;
    let rows = if let Some(reader) = open_reader(workspace, site.id)? {
        reader.run(&compiled, request.page, sink)?
    } else {
        sink.begin(&compiled.columns)
            .and_then(|()| sink.finish())
            .map_err(QueryError::Output)?;
        0
    };
    Ok(QueryOutcome {
        site: site.name,
        query: to_dsl(&query),
        source: compiled.source.label(),
        columns: compiled.columns,
        rows,
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

#[cfg(test)]
mod tests;
