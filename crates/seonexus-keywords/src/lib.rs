//! The domain of the OctopusOn SEO Keyword Research: keywords as people type them, the identities
//! that merge their variants, expansion patterns for discovery and filters; grouping keywords
//! into clusters by wording ([`lexical`]), by the result pages they share ([`serp`]) or both
//! ([`hybrid`]); cluster totals ([`summary`]), clusters against a site ([`mapping`]) and manual placements
//! kept across clusterings ([`edit`]).
//!
//! Everything here is pure and deterministic: the same keywords, numbers and options give
//! the same clusters, so a stored clustering can be explained and repeated.

pub mod cluster;
pub mod edit;
pub mod expand;
pub mod filter;
pub mod hybrid;
pub mod lexical;
pub mod mapping;
pub mod normalize;
pub mod serp;
pub mod summary;

pub use cluster::{Cluster, Clustering, Member};
pub use edit::{Placement, place};
pub use expand::{Expansion, expand};
pub use filter::{Filter, FilterError, FilterSpec};
pub use hybrid::cluster_hybrid;
pub use lexical::{Input, LexicalOptions, cluster_lexical};
pub use mapping::{ClusterMap, SitePage, map_cluster, site_domain};
pub use normalize::{Normalized, normalize};
pub use serp::{Level, Mode, SerpInput, SerpOptions, cluster_serp, result_key};
pub use summary::{ClusterSummary, KeywordFacts, summarize};

#[cfg(test)]
mod tests;
