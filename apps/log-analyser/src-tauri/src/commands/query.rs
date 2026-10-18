use std::path::PathBuf;

use octopuson_log_app::query::{
    self, CheckedQuery, ExportReport, ExportRequest, Page, Query, QueryOutcome, QueryRequest,
    QuerySchema, QuerySpec, SuggestRequest, TemplateInfo, ValueSuggestion,
};
use octopuson_log_app::table::{Cell, Column, TableBuffer};
use octopuson_log_app::table_file::TableFile;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::blocking;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Largest page the UI may request at once.
const MAX_PAGE_ROWS: u64 = 10_000;

/// One page of a query result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryPage {
    outcome: QueryOutcome,
    rows: Vec<Vec<Cell>>,
    /// More rows follow this page.
    has_more: bool,
}

#[tauri::command]
pub(crate) async fn list_templates() -> CommandResult<Vec<TemplateInfo>> {
    Ok(query::templates())
}

#[tauri::command]
pub(crate) async fn check_query(spec: QuerySpec) -> CommandResult<CheckedQuery> {
    Ok(query::check_query(&spec)?)
}

#[tauri::command]
pub(crate) async fn parse_query(text: String) -> CommandResult<Query> {
    Ok(query::parse_query(&text)?)
}

#[tauri::command]
pub(crate) async fn query_schema() -> CommandResult<QuerySchema> {
    Ok(query::query_schema())
}

#[tauri::command]
pub(crate) async fn suggest_values(
    state: State<'_, AppState>,
    site: String,
    field: String,
    search: String,
    filter: Option<String>,
    limit: u64,
) -> CommandResult<Vec<ValueSuggestion>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = SuggestRequest {
            site,
            field,
            search,
            filter,
            limit,
        };
        Ok(query::suggest_values(&workspace, &request)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn run_query(
    state: State<'_, AppState>,
    site: String,
    spec: QuerySpec,
    offset: u64,
    limit: u64,
) -> CommandResult<QueryPage> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let limit = limit.clamp(1, MAX_PAGE_ROWS);
        let request = QueryRequest {
            site,
            spec,
            // One extra row tells whether another page exists.
            page: Some(Page {
                offset,
                size: limit + 1,
            }),
        };
        let mut buffer = TableBuffer::default();
        let mut outcome = query::run_query(&workspace, &request, &mut buffer)?;
        let has_more = buffer.rows.len() as u64 > limit;
        buffer
            .rows
            .truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        outcome.rows = buffer.rows.len() as u64;
        Ok(QueryPage {
            outcome,
            rows: buffer.rows,
            has_more,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_query(
    state: State<'_, AppState>,
    site: String,
    spec: QuerySpec,
    path: PathBuf,
) -> CommandResult<ExportReport> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = ExportRequest {
            site,
            spec,
            path,
            format: None,
        };
        Ok(query::export_query(&workspace, &request)?)
    })
    .await
}

/// A table held by the UI (analysis results) to be saved as a file.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TableData {
    columns: Vec<Column>,
    rows: Vec<Vec<serde_json::Value>>,
}

#[tauri::command]
pub(crate) async fn save_table(path: PathBuf, table: TableData) -> CommandResult<u64> {
    blocking(move || {
        let mut file = TableFile::create(&path, None)?;
        let sink = file.sink();
        let write = |error: std::io::Error| CommandError::internal(error.to_string());
        sink.begin(&table.columns).map_err(write)?;
        let mut cells = Vec::with_capacity(table.columns.len());
        for row in &table.rows {
            cells.clear();
            cells.extend(row.iter().zip(&table.columns).map(|(v, c)| to_cell(v, c)));
            sink.row(&cells).map_err(write)?;
        }
        sink.finish().map_err(write)?;
        file.commit()?;
        Ok(table.rows.len() as u64)
    })
    .await
}

/// Rebuilds a typed cell from its JSON form (see `Cell`'s serialization).
fn to_cell(value: &serde_json::Value, column: &Column) -> Cell {
    use octopuson_log_app::table::ColumnKind;
    use serde_json::Value;
    match (value, column.kind) {
        (Value::Null, _) => Cell::Null,
        (Value::Bool(flag), _) => Cell::Boolean(*flag),
        (Value::Number(number), ColumnKind::Integer) => number
            .as_i64()
            .map_or_else(|| Cell::Text(number.to_string()), Cell::Integer),
        (Value::Number(number), _) => number
            .as_f64()
            .map_or_else(|| Cell::Text(number.to_string()), Cell::Float),
        // Dates and timestamps travel as ISO text and are written as text.
        (Value::String(text), _) => Cell::Text(text.clone()),
        (other, _) => Cell::Text(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use octopuson_log_app::table::ColumnKind;
    use serde_json::json;

    use super::*;

    #[test]
    fn json_cells_round_trip_to_typed_cells() {
        let integer = Column::new("hits", ColumnKind::Integer);
        let float = Column::new("avg_ms", ColumnKind::Float);
        assert_eq!(to_cell(&json!(5), &integer), Cell::Integer(5));
        assert_eq!(to_cell(&json!(1.5), &float), Cell::Float(1.5));
        assert_eq!(to_cell(&json!(null), &integer), Cell::Null);
        assert_eq!(to_cell(&json!("x"), &integer), Cell::Text("x".into()));
        assert_eq!(to_cell(&json!(true), &integer), Cell::Boolean(true));
    }
}
