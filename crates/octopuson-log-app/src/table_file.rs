//! Writing result tables to CSV, JSON or XLSX files atomically.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use octopuson_core::table::TableSink;
use octopuson_report::ExportFormat;

/// A result file could not be created or completed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TableFileError {
    /// The format could not be derived from the file name.
    #[error("cannot tell the export format of `{0}`; use a .csv, .json or .xlsx file name")]
    UnknownFormat(PathBuf),
    /// Writing failed.
    #[error("cannot write `{path}`")]
    Write {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
}

/// A table being written under a temporary name; [`commit`](Self::commit) renames it into
/// place. Dropping it uncommitted deletes the partial file, so failures never leave a
/// truncated export behind.
pub struct TableFile {
    path: PathBuf,
    partial: PathBuf,
    format: ExportFormat,
    sink: Option<Box<dyn TableSink>>,
}

impl std::fmt::Debug for TableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableFile")
            .field("path", &self.path)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

impl TableFile {
    /// Creates the temporary file; `format` defaults to the one implied by the extension.
    ///
    /// # Errors
    /// See [`TableFileError`].
    pub fn create(path: &Path, format: Option<ExportFormat>) -> Result<Self, TableFileError> {
        let format = format
            .or_else(|| ExportFormat::from_path(path))
            .ok_or_else(|| TableFileError::UnknownFormat(path.to_owned()))?;
        let partial = partial_path(path);
        let file = File::create(&partial).map_err(|source| TableFileError::Write {
            path: path.to_owned(),
            source,
        })?;
        Ok(Self {
            path: path.to_owned(),
            partial,
            format,
            sink: Some(format.writer(file)),
        })
    }

    /// The format being written.
    pub fn format(&self) -> ExportFormat {
        self.format
    }

    /// Destination path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The sink to stream the table into (call `finish` on it before committing).
    pub fn sink(&mut self) -> &mut dyn TableSink {
        // `sink` is only taken by `commit`, which consumes `self`.
        self.sink
            .get_or_insert_with(|| Box::new(octopuson_core::table::TableBuffer::default()))
            .as_mut()
    }

    /// Moves the finished file into place.
    ///
    /// # Errors
    /// [`TableFileError::Write`] when the rename fails.
    pub fn commit(mut self) -> Result<(), TableFileError> {
        // Closes the file before renaming (required on Windows).
        drop(self.sink.take());
        fs::rename(&self.partial, &self.path).map_err(|source| TableFileError::Write {
            path: self.path.clone(),
            source,
        })?;
        self.partial = PathBuf::new();
        Ok(())
    }
}

impl Drop for TableFile {
    fn drop(&mut self) {
        drop(self.sink.take());
        if !self.partial.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.partial);
        }
    }
}

fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use octopuson_core::table::{Cell, Column, ColumnKind};

    use super::*;

    #[test]
    fn commits_or_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.csv");
        let mut file = TableFile::create(&path, None).unwrap();
        let sink = file.sink();
        sink.begin(&[Column::new("a", ColumnKind::Integer)])
            .unwrap();
        sink.row(&[Cell::Integer(1)]).unwrap();
        sink.finish().unwrap();
        file.commit().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "a\r\n1\r\n");

        let abandoned = dir.path().join("x.json");
        drop(TableFile::create(&abandoned, None).unwrap());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(matches!(
            TableFile::create(&dir.path().join("x.txt"), None),
            Err(TableFileError::UnknownFormat(_))
        ));
        assert_eq!(
            partial_path(Path::new("out/report.xlsx")),
            Path::new("out/report.xlsx.partial")
        );
    }
}
