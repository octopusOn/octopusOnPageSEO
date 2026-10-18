//! Reading stored result pages again: testing parser selectors on them and repairing
//! snapshots after the selectors were fixed, without sending a request.

use seonexus_serp::mapping::{CompiledMapping, ParsedPage};
use seonexus_serp::profile::{ProfileOverride, apply_override, builtin};
use seonexus_serp::provider::RawPage;
use seonexus_serp::provider::builtin_specs;
use seonexus_serp::{SerpSnapshot, SnapshotBuilder, find_rank};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{SerpAppError, SerpWorkspace};

/// Provider id of the browser mode.
const BROWSER: &str = "browser";

/// A stored response as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPage {
    /// Result page number.
    pub page: u16,
    /// Address it was read from.
    pub url: String,
    /// HTTP status (0 for pages read in the browser).
    pub status: u16,
    /// Media type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Size in bytes.
    pub size: usize,
    /// The response, as text.
    pub body: String,
}

/// How stored pages of a snapshot are read.
enum Reader {
    /// An HTML parser profile (browser mode).
    Profile(Box<CompiledMapping>, String),
    /// A provider spec (built-in or the user's).
    Spec(Box<seonexus_serp::spec::CompiledSpec>),
}

impl Reader {
    fn parse(
        &self,
        engine: seonexus_serp::Engine,
        page: &RawPage,
    ) -> Result<ParsedPage, SerpAppError> {
        let url = Url::parse(&page.url)
            .map_err(|error| SerpAppError::InvalidProvider(format!("stored address: {error}")))?;
        let body = String::from_utf8_lossy(&page.body);
        match self {
            Self::Profile(mapping, _) => mapping.parse(&body, &url, page.page),
            Self::Spec(spec) => spec.parse(engine, &body, &url, page.page),
        }
        .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))
    }
}

impl SerpWorkspace {
    /// The stored responses of a snapshot.
    ///
    /// # Errors
    /// [`SerpAppError::Store`] for unknown snapshots and unreadable files.
    pub fn stored_pages(&self, snapshot_id: i64) -> Result<Vec<StoredPage>, SerpAppError> {
        Ok(self
            .store()?
            .raw_pages(snapshot_id)?
            .into_iter()
            .map(|page| StoredPage {
                page: page.page,
                size: page.body.len(),
                body: String::from_utf8_lossy(&page.body).into_owned(),
                url: page.url,
                status: page.status,
                content_type: page.content_type,
            })
            .collect())
    }

    /// Reads one stored page of a browser-mode snapshot with the parser profile in use, or
    /// with `patch` (an override being edited). Nothing is saved.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] for an invalid patch or a snapshot from an API;
    /// store errors.
    pub fn test_parser(
        &self,
        snapshot_id: i64,
        page: u16,
        patch: Option<&ProfileOverride>,
    ) -> Result<ParsedPage, SerpAppError> {
        let (snapshot, pages) = {
            let store = self.store()?;
            (store.snapshot(snapshot_id)?, store.raw_pages(snapshot_id)?)
        };
        if snapshot.provider != BROWSER {
            return Err(SerpAppError::InvalidProvider(format!(
                "snapshot {snapshot_id} came from `{}`; parser profiles read browser pages only",
                snapshot.provider
            )));
        }
        let engine = snapshot.query.engine;
        let reader = match patch {
            Some(patch) => {
                let base = builtin(engine)
                    .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
                let effective = apply_override(&base, patch)
                    .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
                let mapping = effective
                    .profile
                    .compile()
                    .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
                Reader::Profile(Box::new(mapping), effective.key(true))
            }
            None => self.reader(&snapshot)?,
        };
        let stored = pages
            .iter()
            .find(|stored| stored.page == page)
            .ok_or_else(|| {
                SerpAppError::InvalidQuery(format!("snapshot {snapshot_id} has no page {page}"))
            })?;
        reader.parse(engine, stored)
    }

    /// Reads a snapshot again from its stored responses with the parser in use, saves the
    /// new results and corrects the positions rank checks read from it.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] when the snapshot's provider cannot be read again
    /// (Yandex XML, a removed custom provider) or no result can be read; store errors.
    pub fn reparse_snapshot(&self, snapshot_id: i64) -> Result<SerpSnapshot, SerpAppError> {
        let (old, pages) = {
            let store = self.store()?;
            (store.snapshot(snapshot_id)?, store.raw_pages(snapshot_id)?)
        };
        if pages.is_empty() {
            return Err(SerpAppError::InvalidProvider(format!(
                "snapshot {snapshot_id} has no stored responses"
            )));
        }
        let reader = self.reader(&old)?;
        let mut builder = SnapshotBuilder::new(
            old.query.clone(),
            old.provider.clone(),
            old.fetched_at.clone(),
        );
        if let Some(revision) = &old.provider_rev {
            builder = builder.provider_rev(revision.clone());
        }
        builder = match (&reader, &old.parser_rev) {
            (Reader::Profile(_, key), _) => builder.parser_rev(key.clone()),
            (Reader::Spec(_), Some(revision)) => builder.parser_rev(revision.clone()),
            (Reader::Spec(_), None) => builder,
        };
        for page in &pages {
            let parsed = reader.parse(old.query.engine, page)?;
            if builder.add_page(parsed) == 0 || builder.is_full() {
                break;
            }
        }
        let snapshot = builder.finish();
        if snapshot.organic.is_empty() {
            return Err(SerpAppError::InvalidProvider(
                "no result could be read from the stored pages; the selectors may still be outdated"
                    .to_owned(),
            ));
        }
        let mut store = self.store()?;
        store.replace_contents(snapshot_id, &snapshot)?;
        for project_id in store.snapshot_projects(snapshot_id)? {
            let project = store.project(project_id)?;
            let rank = find_rank(&snapshot, &project.target);
            store.update_ranking(
                snapshot_id,
                project_id,
                rank.as_ref().map(|rank| rank.position),
                rank.as_ref().map(|rank| rank.url.as_str()),
                rank.as_ref()
                    .map_or(&[][..], |rank| rank.positions.as_slice()),
            )?;
        }
        Ok(snapshot)
    }

    /// The parser that reads a snapshot's provider today.
    fn reader(&self, snapshot: &SerpSnapshot) -> Result<Reader, SerpAppError> {
        if snapshot.provider == BROWSER {
            let (mapping, key) = self
                .compiled_parsers()?
                .remove(&snapshot.query.engine)
                .ok_or_else(|| {
                    SerpAppError::InvalidProvider(format!(
                        "no parser profile for {}",
                        snapshot.query.engine
                    ))
                })?;
            return Ok(Reader::Profile(Box::new(mapping), key));
        }
        let spec = builtin_specs()
            .unwrap_or_default()
            .into_iter()
            .chain(self.custom_specs()?)
            .find(|spec| spec.id == snapshot.provider)
            .ok_or_else(|| {
                SerpAppError::InvalidProvider(format!(
                    "snapshots of `{}` cannot be read again",
                    snapshot.provider
                ))
            })?;
        Ok(Reader::Spec(Box::new(spec.compile().map_err(|error| {
            SerpAppError::InvalidProvider(error.to_string())
        })?)))
    }
}
