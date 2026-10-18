//! SEO facts of an HTML document.
//!
//! [`decode_html`] turns a response body into text using the charset the server or the
//! document declares, and [`parse_page`] extracts what search engines read from it: titles,
//! meta descriptions, robots directives, canonicals, hreflang annotations, pagination, headings,
//! links (with anchor text and `rel` values), images, structured data, social tags and the
//! visible text with its fingerprints for duplicate detection.
//!
//! Parsing uses html5ever, the HTML5 parsing algorithm browsers implement, so malformed markup
//! is read the way Google reads it.

mod decode;
mod directives;
mod page;
pub mod schema;
mod text;

pub use decode::decode_html;
pub use directives::{Directives, parse_directives};
pub use page::{
    Heading, Hreflang, Image, Link, LinkKind, LinkName, MetaRefresh, PageFacts, SocialTags,
    StructuredData, StructuredDataFormat, parse_page,
};
pub use text::{hamming_distance, simhash};
