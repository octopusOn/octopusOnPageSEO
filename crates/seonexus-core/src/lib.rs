//! Shared domain types, identifiers and error primitives for octopusSEO products.

mod date;
pub mod table;
mod time;

pub use date::Date;
pub use time::{CivilDateTime, Timestamp};
