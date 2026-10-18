//! Opening sites for reading with up-to-date bot classification.

use octopuson_store::{SiteId, SiteReader, SiteStore, StoreError};

use crate::bots::{bot_context, classifier};
use crate::geo::{current_release_key, refresh_site_geo};
use crate::workspace::Workspace;

/// Opens a site read-only; `None` if nothing was imported yet.
///
/// When the stored classification predates the current bot catalog, the site is briefly
/// opened for writing to re-classify user agents first (without DNS).
pub(crate) fn open_reader(
    workspace: &Workspace,
    site: SiteId,
) -> Result<Option<SiteReader>, StoreError> {
    let Some(reader) = SiteReader::open(workspace.data_dir(), site, workspace.settings())? else {
        return Ok(None);
    };
    let stale_bots = reader.bot_catalog_version()? != classifier(workspace).version();
    let geo = current_release_key(workspace);
    let stale_geo = geo.is_some() && reader.geo_release()? != geo;
    if !stale_bots && !stale_geo {
        return Ok(Some(reader));
    }
    drop(reader);
    {
        let mut store = SiteStore::open(workspace.data_dir(), site, workspace.settings())?;
        if stale_bots {
            let context = bot_context(workspace, false);
            let outcome = store.reclassify_user_agents(&context.classifier, &context.verifier)?;
            tracing::info!(
                changed = outcome.changed_user_agents,
                "user agents re-classified with the current bot catalog"
            );
        }
        if stale_geo {
            refresh_site_geo(workspace, &mut store)?;
        }
    }
    SiteReader::open(workspace.data_dir(), site, workspace.settings())
}
