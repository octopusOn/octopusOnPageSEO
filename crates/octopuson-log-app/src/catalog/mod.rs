//! Client and site management use-cases.

pub(crate) mod names;
mod overview;

pub use names::NameError;
pub use overview::{ImportSummary, SiteOverview, site_overview};

use octopuson_logparse::FormatRegistry;
use octopuson_store::{Catalog, Client, NewSite, Site, SiteStore, StoreError};
use serde::Serialize;

use crate::format_choice::{FormatChoice, FormatError};
use crate::workspace::Workspace;
use names::normalize_name;

/// Errors of client and site management.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CatalogError {
    /// Invalid client or site name.
    #[error(transparent)]
    InvalidName(#[from] NameError),
    /// Invalid log format choice.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// Storage failure, duplicate or missing entry.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// A client as shown to users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientInfo {
    /// Identifier.
    pub id: i64,
    /// Name.
    pub name: String,
    /// Creation time (RFC 3339).
    pub created_at: String,
}

/// A site as shown to users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteInfo {
    /// Identifier.
    pub id: i64,
    /// Owning client's name.
    pub client: String,
    /// Name.
    pub name: String,
    /// Stored format specification (`auto`, `builtin:<id>`, `nginx:<format>`, …).
    pub format: String,
    /// Creation time (RFC 3339).
    pub created_at: String,
}

/// Creates a client.
///
/// # Errors
/// See [`CatalogError`].
pub fn add_client(workspace: &Workspace, name: &str) -> Result<ClientInfo, CatalogError> {
    let name = normalize_name(name)?;
    let catalog = Catalog::open(workspace.data_dir())?;
    Ok(client_info(catalog.add_client(name)?))
}

/// Lists clients by name.
///
/// # Errors
/// See [`CatalogError`].
pub fn list_clients(workspace: &Workspace) -> Result<Vec<ClientInfo>, CatalogError> {
    let catalog = Catalog::open(workspace.data_dir())?;
    Ok(catalog.clients()?.into_iter().map(client_info).collect())
}

/// Input of [`add_site`].
#[derive(Debug, Clone)]
pub struct AddSiteRequest {
    /// Name of an existing client.
    pub client: String,
    /// New site name, e.g. the domain.
    pub name: String,
    /// Log format used when importing.
    pub format: FormatChoice,
}

/// Creates a site under an existing client.
///
/// # Errors
/// See [`CatalogError`].
pub fn add_site(workspace: &Workspace, request: &AddSiteRequest) -> Result<SiteInfo, CatalogError> {
    let name = normalize_name(&request.name)?;
    request.format.validate(&FormatRegistry::builtin())?;
    let catalog = Catalog::open(workspace.data_dir())?;
    let site = catalog.add_site(NewSite {
        client_name: request.client.trim(),
        name,
        format_spec: &request.format.to_spec(),
    })?;
    Ok(site_info(site))
}

/// Lists sites by client and name.
///
/// # Errors
/// See [`CatalogError`].
pub fn list_sites(workspace: &Workspace) -> Result<Vec<SiteInfo>, CatalogError> {
    let catalog = Catalog::open(workspace.data_dir())?;
    Ok(catalog.sites()?.into_iter().map(site_info).collect())
}

/// Deletes a client that owns no sites.
///
/// # Errors
/// See [`CatalogError`].
pub fn remove_client(workspace: &Workspace, name: &str) -> Result<(), CatalogError> {
    Catalog::open(workspace.data_dir())?.remove_client(name.trim())?;
    Ok(())
}

/// Changes the log format used by later imports of a site.
///
/// # Errors
/// See [`CatalogError`].
pub fn set_site_format(
    workspace: &Workspace,
    name: &str,
    format: &FormatChoice,
) -> Result<SiteInfo, CatalogError> {
    format.validate(&FormatRegistry::builtin())?;
    let catalog = Catalog::open(workspace.data_dir())?;
    let site = catalog.site_by_name(name.trim())?;
    let spec = format.to_spec();
    catalog.set_site_format(site.id, &spec)?;
    Ok(site_info(Site {
        format_spec: spec,
        ..site
    }))
}

/// Deletes a site with all its imported data.
///
/// # Errors
/// See [`CatalogError`]; a site in use by an import or reader is reported as busy and left
/// untouched.
pub fn remove_site(workspace: &Workspace, name: &str) -> Result<(), CatalogError> {
    let catalog = Catalog::open(workspace.data_dir())?;
    let site = catalog.site_by_name(name.trim())?;
    // Holding the store's exclusive lock first guarantees nobody is using the site.
    let store = SiteStore::open(workspace.data_dir(), site.id, workspace.settings())?;
    catalog.remove_site(site.id)?;
    store.destroy()?;
    Ok(())
}

/// Deletes one import of a site (e.g. a wrong file) with its hits and rollups.
///
/// # Errors
/// See [`CatalogError`]; [`StoreError::ImportNotFound`] for an unknown id and a busy site
/// while it is imported or queried.
pub fn delete_import(
    workspace: &Workspace,
    site: &str,
    import_id: i64,
) -> Result<(), CatalogError> {
    let site = Catalog::open(workspace.data_dir())?.site_by_name(site.trim())?;
    if !SiteStore::exists(workspace.data_dir(), site.id) {
        return Err(StoreError::ImportNotFound(import_id).into());
    }
    let mut store = SiteStore::open(workspace.data_dir(), site.id, workspace.settings())?;
    store.delete_import(import_id)?;
    Ok(())
}

fn client_info(client: Client) -> ClientInfo {
    ClientInfo {
        id: client.id.get(),
        name: client.name,
        created_at: client.created_at.to_string(),
    }
}

pub(crate) fn site_info(site: Site) -> SiteInfo {
    SiteInfo {
        id: site.id.get(),
        client: site.client_name,
        name: site.name,
        format: site.format_spec,
        created_at: site.created_at.to_string(),
    }
}

#[cfg(test)]
mod tests;
