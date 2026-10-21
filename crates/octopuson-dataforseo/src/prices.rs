//! List prices for estimates before a task runs, US dollars.
//!
//! The answer's own `cost` is what gets recorded; these only say beforehand what a task will
//! about cost, so the user can confirm it. Credit prices multiply them by the gateway
//! catalog's `price_multiplier`.

/// A DataForSEO Labs live task.
pub const LABS_TASK: f64 = 0.012;

/// Each result row a Labs task returns.
pub const LABS_ROW: f64 = 0.000_12;

/// A Backlinks live request.
pub const BACKLINKS_TASK: f64 = 0.024;

/// Each result row a Backlinks request returns.
pub const BACKLINKS_ROW: f64 = 0.000_036;

/// A Labs task returning up to `rows` rows.
pub fn labs(rows: u32) -> f64 {
    LABS_TASK + LABS_ROW * f64::from(rows)
}

/// A Backlinks request returning up to `rows` rows.
pub fn backlinks(rows: u32) -> f64 {
    BACKLINKS_TASK + BACKLINKS_ROW * f64::from(rows)
}
