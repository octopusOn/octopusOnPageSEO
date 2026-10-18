//! Explainable SEO opportunities, independent of HTTP, credentials, storage and desktop UI.

mod analysis;
mod model;
pub use analysis::{analyze, analyze_with_cancel, normalized_url};
pub use model::*;

#[cfg(test)]
mod tests;
