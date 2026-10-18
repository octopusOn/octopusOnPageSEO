//! Streaming access-log parsers for SEO log analysis.
//!
//! # Overview
//!
//! - [`LogFormat`] is the strategy trait every format implements.
//! - [`TemplateFormat`] covers all line-oriented formats (Traefik CLF, Nginx/Apache
//!   combined, Apache common and user-defined Nginx `log_format` / Apache `LogFormat`
//!   strings). Built-in presets are templates too, so there is a single, well-tested
//!   scanning engine.
//! - [`TraefikJsonFormat`] handles Traefik's JSON access logs; [`JsonFormat`] any JSON log
//!   with recognisable keys (Caddy, Cloudflare, Google Cloud, Nginx JSON, …).
//! - [`W3cFormat`] reads IIS and CloudFront logs from their `#Fields:` header,
//!   [`DelimitedFormat`] CSV/TSV logs from their header row, [`ElbFormat`] AWS load balancer
//!   logs and [`HaproxyFormat`] HAProxy HTTP logs.
//! - [`FormatRegistry`] lists available formats and [`detect`] picks the best match for a
//!   sample of lines.
//! - [`input`] opens plain or compressed files (`.gz`, `.zst`, `.bz2`, `.zip`) and yields
//!   bounded-length lines.
//!
//! ```
//! use octopuson_logparse::{FormatRegistry, LogFormat};
//!
//! let registry = FormatRegistry::builtin();
//! let format = registry.get("combined").unwrap();
//! let line = r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] "GET /robots.txt HTTP/1.1" 200 68 "-" "Googlebot/2.1""#;
//! let record = format.parse(line).unwrap();
//! assert_eq!(record.status, 200);
//! assert_eq!(record.target, "/robots.txt");
//! ```

mod delimited;
mod detect;
mod elb;
mod error;
mod format;
mod haproxy;
pub mod input;
mod json;
mod json_any;
mod record;
mod registry;
mod scan;
mod structured;
mod template;
mod time;
mod tokens;
mod values;
mod w3c;

pub use delimited::DelimitedFormat;
pub use detect::{Detection, detect, header_formats};
pub use elb::ElbFormat;
pub use error::{Field, ParseError, ParseErrorKind};
pub use format::LogFormat;
pub use haproxy::HaproxyFormat;
pub use json::TraefikJsonFormat;
pub use json_any::JsonFormat;
pub use record::LogRecord;
pub use registry::FormatRegistry;
pub use template::{TemplateError, TemplateFormat};
pub use w3c::W3cFormat;
