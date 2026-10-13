//! Export file formats.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use seonexus_core::table::TableSink;

use crate::{CsvWriter, JsonWriter, XlsxWriter};

/// A supported export file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Comma-separated values.
    Csv,
    /// JSON array of objects.
    Json,
    /// Excel workbook.
    Xlsx,
}

impl ExportFormat {
    /// Every format, for help texts and file dialogs.
    pub const ALL: [Self; 3] = [Self::Csv, Self::Json, Self::Xlsx];

    /// Format for a file extension (case-insensitive, without the dot).
    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.extension().eq_ignore_ascii_case(extension))
    }

    /// Format implied by the extension of `path`.
    pub fn from_path(path: &Path) -> Option<Self> {
        Self::from_extension(path.extension()?.to_str()?)
    }

    /// Canonical file extension.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Xlsx => "xlsx",
        }
    }

    /// A sink writing this format to `file`.
    pub fn writer(self, file: File) -> Box<dyn TableSink> {
        let out = BufWriter::new(file);
        match self {
            Self::Csv => Box::new(CsvWriter::new(out)),
            Self::Json => Box::new(JsonWriter::new(out)),
            Self::Xlsx => Box::new(XlsxWriter::new(out)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_extensions_case_insensitively() {
        assert_eq!(
            ExportFormat::from_path(Path::new("out/Report.XLSX")),
            Some(ExportFormat::Xlsx)
        );
        assert_eq!(ExportFormat::from_extension("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::from_path(Path::new("report")), None);
        assert_eq!(ExportFormat::from_path(Path::new("report.txt")), None);
    }
}
