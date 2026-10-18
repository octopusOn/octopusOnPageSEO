//! Atomic XML and ZIP sitemap exports. Parts are streamed and index locations are site-root URLs.

use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    CrawlReader, CrawlStatus, CrawlStoreError, RenderMode, SitemapPlan, SitemapSelection,
    SpiderError, SpiderWorkspace,
};
use octopuson_crawl_store::{
    SITEMAP_FOOTER, SITEMAP_HEADER, SITEMAP_MAX_BYTES, SITEMAP_MAX_URLS, SitemapLayout,
    sitemap_escape, sitemap_url_entry,
};

const INDEX_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n";
const INDEX_FOOTER: &str = "</sitemapindex>\n";

/// A single XML sitemap or a ZIP containing a sitemap index and all parts.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SitemapFormat {
    /// At most 50,000 URLs and 50 MiB, including the envelope.
    Xml,
    /// One index and as many protocol-sized parts as needed.
    Zip,
}

/// Export result, displayed without passing XML document contents through desktop IPC.
#[derive(Debug, Serialize)]
pub struct SitemapExport {
    /// Number of URL entries, excluding the index.
    pub urls: u64,
    /// Number of sitemap parts (index is additional for ZIP).
    pub parts: u64,
}

impl SpiderWorkspace {
    fn sitemap_reader(&self, id: &str) -> Result<(CrawlReader, bool, Vec<String>), SpiderError> {
        let meta = self.meta(id)?;
        if meta.status == CrawlStatus::Running {
            return Err(invalid("finish the crawl before generating a sitemap"));
        }
        let mut warnings = Vec::new();
        if meta.status != CrawlStatus::Completed || meta.limit_reached {
            warnings.push("partial".into());
        }
        if meta.mode == crate::CrawlMode::List {
            warnings.push("list-mode".into());
        }
        Ok((
            self.library.open_archive(id)?,
            meta.config.rendering.mode == RenderMode::Javascript,
            warnings,
        ))
    }

    /// Previews a sitemap from one finished archive; no site requests are made.
    ///
    /// # Errors
    /// Invalid/running archive, invalid origin or database errors.
    pub fn sitemap_plan(
        &self,
        id: &str,
        selection: &SitemapSelection,
    ) -> Result<SitemapPlan, SpiderError> {
        let (reader, require_js, warnings) = self.sitemap_reader(id)?;
        let mut plan = reader.sitemap_plan(selection, require_js)?;
        plan.warnings = warnings;
        Ok(plan)
    }

    /// Streams a sitemap or ZIP package to a sibling temporary file before atomically replacing output.
    ///
    /// # Errors
    /// Invalid or empty selection, protocol limits for single XML, database, ZIP or output failures.
    pub fn export_sitemap(
        &self,
        id: &str,
        selection: &SitemapSelection,
        path: &Path,
        format: SitemapFormat,
    ) -> Result<SitemapExport, SpiderError> {
        let extension = match format {
            SitemapFormat::Xml => "xml",
            SitemapFormat::Zip => "zip",
        };
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        {
            return Err(invalid(
                "output extension must match the selected XML or ZIP sitemap format",
            ));
        }
        let (reader, require_js, _) = self.sitemap_reader(id)?;
        let plan = reader.sitemap_plan(selection, require_js)?;
        if plan.included == 0 {
            return Err(invalid("no eligible URLs in the selected sitemap origin"));
        }
        if matches!(format, SitemapFormat::Xml) && plan.parts > 1 {
            return Err(invalid(
                "this selection requires multiple sitemaps; choose the ZIP package",
            ));
        }
        let selection = SitemapSelection {
            origin: plan.origin.clone(),
        };
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let output_error = |source| SpiderError::Output {
            path: path.to_owned(),
            source,
        };
        let temporary = tempfile::NamedTempFile::new_in(parent).map_err(output_error)?;
        let mut output = temporary.reopen().map_err(output_error)?;
        match format {
            SitemapFormat::Xml => write_xml(&reader, &selection, require_js, &mut output)?,
            SitemapFormat::Zip => write_zip(&reader, &selection, require_js, &mut output)?,
        }
        output.sync_all().map_err(output_error)?;
        drop(output);
        temporary.persist(path).map_err(|e| output_error(e.error))?;
        Ok(SitemapExport {
            urls: plan.included,
            parts: plan.parts,
        })
    }
}

fn write_xml(
    reader: &CrawlReader,
    selection: &SitemapSelection,
    require_js: bool,
    output: &mut dyn Write,
) -> Result<(), CrawlStoreError> {
    output
        .write_all(SITEMAP_HEADER.as_bytes())
        .map_err(CrawlStoreError::Output)?;
    let mut urls = 0;
    let mut bytes = (SITEMAP_HEADER.len() + SITEMAP_FOOTER.len()) as u64;
    reader.sitemap_urls(selection, require_js, |address| {
        let entry = sitemap_url_entry(address);
        urls += 1;
        bytes += entry.len() as u64;
        if urls > SITEMAP_MAX_URLS || bytes > SITEMAP_MAX_BYTES {
            return Err(selection_error("sitemap exceeds the protocol limit"));
        }
        output
            .write_all(entry.as_bytes())
            .map_err(CrawlStoreError::Output)
    })?;
    output
        .write_all(SITEMAP_FOOTER.as_bytes())
        .map_err(CrawlStoreError::Output)
}

fn write_zip(
    reader: &CrawlReader,
    selection: &SitemapSelection,
    require_js: bool,
    output: &mut std::fs::File,
) -> Result<(), CrawlStoreError> {
    let mut zip = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut names = Vec::new();
    let mut layout = SitemapLayout::default();
    reader.sitemap_urls(selection, require_js, |address| {
        let entry = sitemap_url_entry(address);
        if layout.add(entry.len() as u64) {
            if !names.is_empty() {
                zip.write_all(SITEMAP_FOOTER.as_bytes())
                    .map_err(CrawlStoreError::Output)?;
            }
            if names.len() as u64 == SITEMAP_MAX_URLS {
                return Err(selection_error("sitemap index exceeds the protocol limit"));
            }
            let name = format!("sitemap-{:05}.xml", names.len() + 1);
            zip.start_file(&name, options).map_err(zip_error)?;
            zip.write_all(SITEMAP_HEADER.as_bytes())
                .map_err(CrawlStoreError::Output)?;
            names.push(name);
        }
        zip.write_all(entry.as_bytes())
            .map_err(CrawlStoreError::Output)
    })?;
    zip.write_all(SITEMAP_FOOTER.as_bytes())
        .map_err(CrawlStoreError::Output)?;
    zip.start_file("sitemap.xml", options).map_err(zip_error)?;
    zip.write_all(INDEX_HEADER.as_bytes())
        .map_err(CrawlStoreError::Output)?;
    let mut index_bytes = (INDEX_HEADER.len() + INDEX_FOOTER.len()) as u64;
    for name in names {
        let location = format!("{}/{name}", selection.origin);
        if location.len() >= 2048 {
            return Err(selection_error(
                "sitemap index location exceeds the protocol URL length",
            ));
        }
        let entry = format!(
            "  <sitemap><loc>{}</loc></sitemap>\n",
            sitemap_escape(&location)
        );
        index_bytes += entry.len() as u64;
        if index_bytes > SITEMAP_MAX_BYTES {
            return Err(selection_error(
                "sitemap index exceeds the protocol byte limit",
            ));
        }
        zip.write_all(entry.as_bytes())
            .map_err(CrawlStoreError::Output)?;
    }
    zip.write_all(INDEX_FOOTER.as_bytes())
        .map_err(CrawlStoreError::Output)?;
    zip.finish().map_err(zip_error)?;
    Ok(())
}
fn zip_error(error: zip::result::ZipError) -> CrawlStoreError {
    CrawlStoreError::Output(io::Error::other(error))
}
fn selection_error(message: &str) -> CrawlStoreError {
    CrawlStoreError::InvalidSelection(message.into())
}
fn invalid(message: &str) -> SpiderError {
    selection_error(message).into()
}

#[cfg(test)]
mod tests;
