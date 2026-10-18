//! Site archives (`.snxsite`): one file holding a site's database, hits, format and settings,
//! to hand a project to a colleague or move it to another computer.
//!
//! The archive is a ZIP file with `manifest.json`, `site.duckdb` and the Parquet hit files
//! (stored without recompression; they are already compressed).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use octopuson_core::Timestamp;
use octopuson_store::{Catalog, SiteStore, StoreError};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;

use crate::catalog::{
    AddSiteRequest, CatalogError, SiteInfo, add_client, add_site, remove_client, remove_site,
};
use crate::format_choice::FormatChoice;
use crate::settings::{SettingsError, SiteSettings, set_site_settings};
use crate::workspace::Workspace;

/// File extension of site archives.
pub const ARCHIVE_EXTENSION: &str = "snxsite";

// Written into every .snxsite archive; it keeps the project's first name so older archives import.
const FORMAT: &str = "seonexus-site";
const VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";

/// Errors of archive export and import.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ArchiveError {
    /// Storage failure, unknown or busy site.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The site could not be created (duplicate name, invalid client, …).
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// The archive's settings are invalid.
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// A file could not be read or written.
    #[error("cannot access `{path}`")]
    Io {
        /// File.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// The file is not a site archive this version can read.
    #[error("not a valid site archive: {0}")]
    Invalid(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    site: String,
    client: String,
    format_spec: String,
    #[serde(default)]
    settings: SiteSettings,
    exported_at: String,
    app_version: String,
}

/// Result of [`export_site`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArchiveReport {
    /// Site name.
    pub site: String,
    /// Written archive.
    pub path: String,
    /// Files in the archive (manifest excluded).
    pub files: usize,
    /// Archive size.
    pub bytes: u64,
}

/// Writes `site` with all its data to an archive at `path` (replaced atomically).
///
/// # Errors
/// See [`ArchiveError`]; a site in use by an import is reported as busy.
pub fn export_site(
    workspace: &Workspace,
    site: &str,
    path: &Path,
) -> Result<ArchiveReport, ArchiveError> {
    let record = Catalog::open(workspace.data_dir())?.site_by_name(site.trim())?;
    let snapshot =
        SiteStore::open(workspace.data_dir(), record.id, workspace.settings())?.into_snapshot()?;
    let files = snapshot.files();
    let manifest = Manifest {
        format: FORMAT.to_owned(),
        version: VERSION,
        site: record.name.clone(),
        client: record.client_name.clone(),
        format_spec: record.format_spec.clone(),
        settings: SiteSettings::from_json(&record.settings).unwrap_or_default(),
        exported_at: Timestamp::now().to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let partial = path.with_extension(format!("{ARCHIVE_EXTENSION}.partial"));
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source: io::Error| ArchiveError::Io { path, source }
    };
    let write = || -> Result<(), ArchiveError> {
        let file = File::create(&partial).map_err(io(&partial))?;
        let mut zip = zip::ZipWriter::new(file);
        let zip_error = |e: zip::result::ZipError| ArchiveError::Io {
            path: partial.clone(),
            source: io::Error::other(e),
        };
        zip.start_file(MANIFEST, SimpleFileOptions::default())
            .map_err(zip_error)?;
        let json = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| ArchiveError::Invalid(e.to_string()))?;
        zip.write_all(&json).map_err(io(&partial))?;
        let stored = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        for (name, source) in files {
            zip.start_file(name.as_str(), stored).map_err(zip_error)?;
            let mut input = File::open(source).map_err(io(source))?;
            io::copy(&mut input, &mut zip).map_err(io(source))?;
        }
        zip.finish().map_err(zip_error)?;
        Ok(())
    };
    if let Err(error) = write() {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    let files = files.len();
    drop(snapshot);
    fs::rename(&partial, path).map_err(io(path))?;
    let bytes = fs::metadata(path).map_or(0, |m| m.len());
    Ok(ArchiveReport {
        site: record.name,
        path: path.display().to_string(),
        files,
        bytes,
    })
}

/// Creates a site from an archive. `name` and `client` override the archive's (the client is
/// created when missing).
///
/// # Errors
/// See [`ArchiveError`]; an existing site name is a duplicate error. Nothing is left behind
/// when the import fails.
pub fn import_site(
    workspace: &Workspace,
    path: &Path,
    name: Option<&str>,
    client: Option<&str>,
) -> Result<SiteInfo, ArchiveError> {
    let file = File::open(path).map_err(|source| ArchiveError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| ArchiveError::Invalid(e.to_string()))?;
    let manifest = read_manifest(&mut archive)?;
    let name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&manifest.site);
    let client = client
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .unwrap_or(&manifest.client);
    let new_client = Catalog::open(workspace.data_dir())?
        .client_by_name(client)
        .is_err();
    if new_client {
        add_client(workspace, client)?;
    }
    let forget_client = || {
        if new_client && let Err(cleanup) = remove_client(workspace, client) {
            tracing::error!(%cleanup, "could not remove the client created for the import");
        }
    };
    let site = match add_site(
        workspace,
        &AddSiteRequest {
            client: client.to_owned(),
            name: name.to_owned(),
            format: FormatChoice::from_spec(&manifest.format_spec).unwrap_or(FormatChoice::Auto),
        },
    ) {
        Ok(site) => site,
        Err(error) => {
            forget_client();
            return Err(error.into());
        }
    };
    let mut restore = || -> Result<(), ArchiveError> {
        let id = Catalog::open(workspace.data_dir())?
            .site_by_name(&site.name)?
            .id;
        extract(&mut archive, workspace, id)?;
        // Opening validates the database and upgrades it if the archive is older.
        set_site_settings(workspace, &site.name, manifest.settings.clone())?;
        drop(SiteStore::open(
            workspace.data_dir(),
            id,
            workspace.settings(),
        )?);
        Ok(())
    };
    if let Err(error) = restore() {
        if let Err(cleanup) = remove_site(workspace, &site.name) {
            tracing::error!(%cleanup, "could not remove the partially imported site");
        }
        forget_client();
        return Err(error);
    }
    Ok(site)
}

fn read_manifest(archive: &mut zip::ZipArchive<File>) -> Result<Manifest, ArchiveError> {
    let entry = archive
        .by_name(MANIFEST)
        .map_err(|_| ArchiveError::Invalid("manifest.json is missing".into()))?;
    let manifest: Manifest =
        serde_json::from_reader(entry).map_err(|e| ArchiveError::Invalid(e.to_string()))?;
    if manifest.format != FORMAT || manifest.version > VERSION {
        return Err(ArchiveError::Invalid(format!(
            "unsupported archive format {} v{}",
            manifest.format, manifest.version
        )));
    }
    Ok(manifest)
}

/// Writes the data entries into the directory of site `id`; any other entry is an error.
fn extract(
    archive: &mut zip::ZipArchive<File>,
    workspace: &Workspace,
    id: octopuson_store::SiteId,
) -> Result<(), ArchiveError> {
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| ArchiveError::Invalid(e.to_string()))?;
        let entry_name = entry
            .name()
            .map_err(|e| ArchiveError::Invalid(e.to_string()))?
            .into_owned();
        if entry.is_dir() || entry_name == MANIFEST {
            continue;
        }
        let target = workspace
            .data_dir()
            .restore_path(id, &entry_name)
            .ok_or_else(|| ArchiveError::Invalid(format!("unexpected entry `{entry_name}`")))?;
        let io = |path: &Path| {
            let path = path.to_owned();
            move |source: io::Error| ArchiveError::Io { path, source }
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io(parent))?;
        }
        let mut output = File::create(&target).map_err(io(&target))?;
        io::copy(&mut entry, &mut output).map_err(io(&target))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    #[test]
    fn sites_round_trip_through_archives() {
        let fixture = Fixture::imported();
        let (_, before) = fixture.run("group:url sort:url:asc");
        let archive = fixture.path("acme.snxsite");
        let report = export_site(&fixture.workspace, "acme.com", &archive).unwrap();
        assert!(report.files >= 2);
        assert!(report.bytes > 0);

        // Same name: refused, nothing created.
        assert!(matches!(
            import_site(&fixture.workspace, &archive, None, None),
            Err(ArchiveError::Catalog(_))
        ));

        let copy = import_site(
            &fixture.workspace,
            &archive,
            Some("copy.acme.com"),
            Some("Colleague"),
        )
        .unwrap();
        assert_eq!(
            (copy.name.as_str(), copy.client.as_str()),
            ("copy.acme.com", "Colleague")
        );
        let mut table = octopuson_core::table::TableBuffer::default();
        let request = crate::query::QueryRequest {
            site: "copy.acme.com".into(),
            spec: crate::query::QuerySpec::text("group:url sort:url:asc"),
            page: None,
        };
        crate::query::run_query(&fixture.workspace, &request, &mut table).unwrap();
        assert_eq!(table.rows, before.rows);
    }

    #[test]
    fn invalid_archives_are_rejected_without_leftovers() {
        let fixture = Fixture::new();
        let not_zip = fixture.write("x.snxsite", "hello");
        assert!(matches!(
            import_site(&fixture.workspace, &not_zip, None, None),
            Err(ArchiveError::Invalid(_))
        ));
        let path = fixture.path("evil.snxsite");
        {
            let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
            zip.start_file(MANIFEST, SimpleFileOptions::default())
                .unwrap();
            zip.write_all(br#"{"format":"seonexus-site","version":1,"site":"evil.com","client":"X","format_spec":"auto","exported_at":"","app_version":""}"#).unwrap();
            zip.start_file("../../escape.txt", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(
            import_site(&fixture.workspace, &path, None, None),
            Err(ArchiveError::Invalid(_))
        ));
        assert!(
            crate::catalog::list_sites(&fixture.workspace)
                .unwrap()
                .iter()
                .all(|site| site.name != "evil.com")
        );
        assert!(
            crate::catalog::list_clients(&fixture.workspace)
                .unwrap()
                .iter()
                .all(|client| client.name != "X")
        );
    }
}
