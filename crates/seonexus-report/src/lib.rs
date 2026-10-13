//! Exporters (CSV, XLSX, JSON) and branded HTML reports.
//!
//! Every exporter implements [`seonexus_core::table::TableSink`], so query results stream
//! straight from the store into the file without being buffered.

mod csv;
mod format;
mod json;
mod xlsx;

pub use csv::CsvWriter;
pub use format::ExportFormat;
pub use json::JsonWriter;
pub use xlsx::{MAX_XLSX_ROWS, XlsxWriter};
