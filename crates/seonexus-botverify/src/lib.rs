//! Search-engine and AI crawler identification and verification.
//!
//! - [`catalog`] lists known bots with their user-agent patterns and verification sources.
//! - [`Classifier`] maps a user-agent string to a [`BotDef`]; [`custom`] adds user-defined
//!   bots ahead of the catalog.
//! - [`IpNet`] / [`IpRangeSet`] parse and match CIDR prefixes.
//! - [`ranges`] parses operators' published IP lists and the bundled snapshot.
//! - [`Verifier`] decides whether an `(ip, bot)` pair is genuine using IP lists and
//!   forward-confirmed reverse DNS through the [`DnsResolver`] port.
//!
//! The crate performs no I/O itself; network access is injected by adapters. See ADR 0005.

pub mod catalog;
mod category;
mod cidr;
mod classify;
pub mod custom;
pub mod ranges;
pub mod robots;
mod verify;

pub use catalog::{BotDef, CATALOG_VERSION};
pub use category::BotCategory;
pub use cidr::{IpNet, IpNetError, IpRangeSet};
pub use classify::Classifier;
pub use custom::{CustomBot, CustomBotError, validate_custom_bots};
pub use verify::{DnsError, DnsResolver, Method, Verdict, Verification, Verifier};
