//! Running queries whose rows are consumed by the analyses themselves.

use std::io;

use octopuson_core::table::{Cell, Column, TableSink};

use crate::query::{QueryError, QueryRequest, QuerySpec, run_query};
use crate::workspace::Workspace;

/// Passes every row to a closure.
struct Collect<F: FnMut(&[Cell])>(F);

impl<F: FnMut(&[Cell])> TableSink for Collect<F> {
    fn begin(&mut self, _columns: &[Column]) -> io::Result<()> {
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        (self.0)(cells);
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Runs query text against `site`, handing each row to `each`.
pub(super) fn for_each_row(
    workspace: &Workspace,
    site: &str,
    text: &str,
    each: impl FnMut(&[Cell]),
) -> Result<(), QueryError> {
    let request = QueryRequest {
        site: site.to_owned(),
        spec: QuerySpec::text(text),
        page: None,
    };
    run_query(workspace, &request, &mut Collect(each)).map(|_| ())
}

/// Joins an optional filter and the rest of a query.
pub(super) fn with_filter(filter: Option<&str>, rest: &str) -> String {
    match filter.map(str::trim) {
        Some(filter) if !filter.is_empty() => format!("{filter} {rest}"),
        _ => rest.to_owned(),
    }
}

/// Integer value of a cell (0 for anything else).
pub(super) fn integer(cell: Option<&Cell>) -> u64 {
    match cell {
        Some(Cell::Integer(value)) => u64::try_from(*value).unwrap_or(0),
        _ => 0,
    }
}

/// Numeric value of a cell; `None` for nulls and text.
#[allow(
    clippy::cast_precision_loss,
    reason = "counts shown as averages; far below 2^52"
)]
pub(super) fn float(cell: Option<&Cell>) -> Option<f64> {
    match cell {
        Some(Cell::Float(value)) => Some(*value),
        Some(Cell::Integer(value)) => Some(*value as f64),
        _ => None,
    }
}

/// Text form of a key cell; `None` for nulls.
pub(super) fn key(cell: Option<&Cell>) -> Option<String> {
    match cell {
        None | Some(Cell::Null) => None,
        Some(other) => Some(other.to_string()),
    }
}
