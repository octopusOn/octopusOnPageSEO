//! Stream exports into a sibling temporary file before replacing the destination.

use crate::{
    CrawlReader, CrawlStoreError, CrawlTab, ExportFormat, ExportedTab, SpiderError,
    TableExportRequest,
};
use octopuson_core::table::TableSink;
use std::path::Path;

pub(crate) fn export_tab(
    reader: &CrawlReader,
    tab: CrawlTab,
    path: &Path,
    format: Option<ExportFormat>,
) -> Result<ExportedTab, SpiderError> {
    write(tab, path, format, |sink| reader.tab(tab, sink))
}

pub(crate) fn export_selection(
    reader: &CrawlReader,
    selection: &TableExportRequest,
    path: &Path,
    format: Option<ExportFormat>,
) -> Result<ExportedTab, SpiderError> {
    let tab = CrawlTab::from_name(&selection.request.tab)
        .ok_or_else(|| CrawlStoreError::InvalidSelection(selection.request.tab.clone()))?;
    write(tab, path, format, |sink| {
        reader.export_selection(selection, sink)
    })
}

fn write(
    tab: CrawlTab,
    path: &Path,
    format: Option<ExportFormat>,
    render: impl FnOnce(&mut dyn TableSink) -> Result<u64, CrawlStoreError>,
) -> Result<ExportedTab, SpiderError> {
    let rows = write_rows(path, format, render)?;
    Ok(ExportedTab {
        tab,
        path: path.to_owned(),
        rows,
    })
}

pub(crate) fn write_rows(
    path: &Path,
    format: Option<ExportFormat>,
    render: impl FnOnce(&mut dyn TableSink) -> Result<u64, CrawlStoreError>,
) -> Result<u64, SpiderError> {
    let format = format
        .or_else(|| ExportFormat::from_path(path))
        .ok_or_else(|| SpiderError::UnknownFormat(path.to_owned()))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output_error = |source| SpiderError::Output {
        path: path.to_owned(),
        source,
    };
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(output_error)?;
    let mut sink = format.writer(temporary.reopen().map_err(output_error)?);
    let rows = render(sink.as_mut())?;
    drop(sink);
    temporary.as_file().sync_all().map_err(output_error)?;
    temporary
        .persist(path)
        .map_err(|failure| output_error(failure.error))?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use octopuson_core::table::{Cell, Column, ColumnKind};

    #[test]
    fn failed_export_preserves_existing_file_and_cleans_temporary_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        std::fs::write(&path, "original report").unwrap();
        let result = write(CrawlTab::Internal, &path, None, |sink| {
            sink.begin(&[Column::new("address", ColumnKind::Text)])
                .map_err(CrawlStoreError::Output)?;
            sink.row(&[Cell::Text("partial row".into())])
                .map_err(CrawlStoreError::Output)?;
            Err(CrawlStoreError::Output(std::io::Error::other(
                "simulated export failure",
            )))
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original report");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
