//! Shared domain types, identifiers and error primitives for OctopusOn SEO products.

mod date;
pub mod http;
pub mod table;
mod time;

pub use date::Date;
pub use time::{CivilDateTime, Timestamp};
