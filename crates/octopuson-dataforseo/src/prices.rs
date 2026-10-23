//! List prices for estimates before a task runs, US dollars.
//!
//! The answer's own `cost` is what gets recorded; these only say beforehand what a task will
//! about cost, so the user can confirm it. Credit prices multiply them by the gateway
//! catalog's `price_multiplier`.

/// A DataForSEO Labs live task.
pub const LABS_TASK: f64 = 0.012;

/// Each result row a Labs task returns.
pub const LABS_ROW: f64 = 0.000_12;

/// A Labs task of the endpoints priced ten times the default: `historical_rank_overview`,
/// `historical_bulk_traffic_estimation` and `domain_metrics_by_categories`.
pub const LABS_HISTORY_TASK: f64 = 0.12;

/// Each row (month or domain) those endpoints return.
pub const LABS_HISTORY_ROW: f64 = 0.001_2;

/// Each result page `historical_serps` returns (no task price).
pub const HISTORICAL_SERP: f64 = 0.000_12;

/// A Backlinks live request.
pub const BACKLINKS_TASK: f64 = 0.024;

/// Each result row a Backlinks request returns.
pub const BACKLINKS_ROW: f64 = 0.000_036;

/// A live Google result page of up to ten results.
pub const SERP_LIVE_PAGE: f64 = 0.002;

/// The AI Overview Google loads after the page, when asked for.
pub const SERP_AI_OVERVIEW: f64 = 0.002;

/// A result page of up to ten results asked through the standard queue (normal priority):
/// answered within minutes instead of at once, for less than a third of the live price.
pub const SERP_STANDARD_PAGE: f64 = 0.000_6;

/// The late AI Overview on a page asked through the standard queue.
pub const SERP_STANDARD_AI_OVERVIEW: f64 = 0.000_6;

/// A Google business profile asked live: an estimate on the safe side, as DataForSEO lists
/// $0.0015 through the standard queue and $0.003 with priority but no live price. What each
/// answer says it cost is what is recorded.
pub const BUSINESS_INFO_LIVE: f64 = 0.005_4;

/// A search of the business listings.
pub const LISTINGS_TASK: f64 = 0.01;

/// Each business a search of the listings returns.
pub const LISTINGS_ROW: f64 = 0.000_3;

/// Ten Google reviews through the standard queue (the only way they are read).
pub const REVIEWS_TEN: f64 = 0.000_75;

/// An LLM Responses task, before the model's own charge.
pub const LLM_TASK: f64 = 0.000_6;

/// An AI Mode answer.
pub const AI_MODE: f64 = 0.004;

/// An LLM Mentions task.
pub const MENTIONS_TASK: f64 = 0.10;

/// Each row an LLM Mentions task returns.
pub const MENTIONS_ROW: f64 = 0.001;

/// A Labs task returning up to `rows` rows.
pub fn labs(rows: u32) -> f64 {
    LABS_TASK + LABS_ROW * f64::from(rows)
}

/// A task of the ten-times Labs endpoints returning up to `rows` rows (months or domains).
pub fn labs_history(rows: u32) -> f64 {
    LABS_HISTORY_TASK + LABS_HISTORY_ROW * f64::from(rows)
}

/// A Backlinks request returning up to `rows` rows.
pub fn backlinks(rows: u32) -> f64 {
    BACKLINKS_TASK + BACKLINKS_ROW * f64::from(rows)
}

/// An LLM Mentions task returning up to `rows` rows.
pub fn mentions(rows: u32) -> f64 {
    MENTIONS_TASK + MENTIONS_ROW * f64::from(rows)
}
