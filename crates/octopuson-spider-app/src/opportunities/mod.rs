//! Optional GSC/GA4 opportunities. No writes to crawl archives or technical SEO scores.
#![allow(missing_docs)]

mod evidence;
mod files;
mod google;
mod imports;
mod oauth;
mod oauth_client;
mod runtime;
mod store;
mod xlsx;

pub use evidence::{CrawlEvidence, crawl_evidence};
pub use google::{GoogleCatalog, GoogleProperty, SyncProgress};
pub use imports::{ColumnMapping, ImportPreview, ImportRequest, ImportSample};
pub use oauth::{GoogleConnection, GoogleStatus, OAuthStart};
pub use octopuson_opportunities::*;
pub use runtime::{
    AnalysisSummary, OpportunityDetail, OpportunityJobs, OpportunityPage, OpportunityPageRequest,
    SyncJob,
};
pub use store::{OpportunityWorkspace, WorkspaceStatus};

pub(crate) fn now_ms() -> i64 {
    octopuson_core::Timestamp::now().as_unix_micros() / 1000
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn new_id() -> Result<String, String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes
        .iter()
        .flat_map(|b| {
            [
                char::from(HEX[usize::from(b >> 4)]),
                char::from(HEX[usize::from(b & 15)]),
            ]
        })
        .collect())
}
