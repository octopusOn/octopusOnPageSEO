//! What a crawl produces.

use octopuson_html::{Directives, Hreflang, PageFacts};
use serde::Serialize;

/// Identifier of a URL within one crawl, in order of discovery (start URLs first).
pub type UrlId = u32;

/// A fetched (or robots-blocked) URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct UrlRecord {
    /// Identifier within the crawl.
    pub id: UrlId,
    /// Absolute URL as crawled.
    pub url: String,
    /// Clicks from a start URL; `None` for URLs only sitemaps or the list reached.
    pub depth: Option<u32>,
    /// On the crawled site (as opposed to an external link or resource).
    pub internal: bool,
    /// Milliseconds since the Unix epoch when the response arrived.
    pub crawled_at_ms: i64,
    /// HTTP status; `None` when blocked or no response arrived.
    pub status: Option<u16>,
    /// Why no response arrived ([`FetchError::code`](crate::FetchError::code)).
    pub error: Option<String>,
    /// robots.txt disallows the URL for the crawling agent.
    pub blocked_by_robots: bool,
    /// The robots.txt rule that decided, as written.
    pub robots_rule: Option<String>,
    /// What the response contains.
    pub content_kind: ContentKind,
    /// `Content-Type` header.
    pub content_type: Option<String>,
    /// Bytes of body received (decompressed).
    pub size_bytes: u64,
    /// The body was cut at the size limit.
    pub truncated: bool,
    /// Milliseconds until the response headers arrived.
    pub response_ms: u32,
    /// `HTTP/1.1`, `HTTP/2`, ...
    pub http_version: Option<String>,
    /// Absolute target of a redirect (`Location`).
    pub redirect_url: Option<String>,
    /// `X-Robots-Tag` values joined with `; `.
    pub x_robots_tag: Option<String>,
    /// Canonical from the HTTP `Link` header.
    pub header_canonical: Option<String>,
    /// Alternate language annotations from HTTP `Link`, independent of saved headers/HTML.
    pub header_hreflang: Vec<Hreflang>,
    /// `Last-Modified` header.
    pub last_modified: Option<String>,
    /// `Content-Encoding` header.
    pub content_encoding: Option<String>,
    /// `Cache-Control` header.
    pub cache_control: Option<String>,
    /// `Strict-Transport-Security` is sent.
    pub hsts: bool,
    /// Robots directives from meta tags and headers that apply to the crawling agent.
    pub directives: Directives,
    /// Whether search engines may index the URL, and why not.
    pub indexability: Indexability,
    /// Facts of a fetched HTML page.
    pub page: Option<PageFacts>,
    /// Every response header, when the crawl stores headers.
    pub headers: Vec<(String, String)>,
    /// The page's HTML, when the crawl stores HTML.
    pub html: Option<String>,
    /// Rendering diagnostics and the original page facts, when requested.
    pub rendering: Option<RenderRecord>,
    /// Optional local structured-data validation of the complete fetched/rendered document.
    pub schema_report: Option<octopuson_html::schema::SchemaReport>,
    /// Why the requested structured-data check was unavailable; never interpreted as a pass.
    pub schema_error: Option<String>,
}

impl UrlRecord {
    /// An empty record for `url`.
    pub fn new(id: UrlId, url: String, depth: Option<u32>, internal: bool) -> Self {
        Self {
            id,
            url,
            depth,
            internal,
            crawled_at_ms: 0,
            status: None,
            error: None,
            blocked_by_robots: false,
            robots_rule: None,
            content_kind: ContentKind::Unknown,
            content_type: None,
            size_bytes: 0,
            truncated: false,
            response_ms: 0,
            http_version: None,
            redirect_url: None,
            x_robots_tag: None,
            header_canonical: None,
            header_hreflang: Vec::new(),
            last_modified: None,
            content_encoding: None,
            cache_control: None,
            hsts: false,
            directives: Directives::default(),
            indexability: Indexability::NoResponse,
            page: None,
            headers: Vec::new(),
            html: None,
            rendering: None,
            schema_report: None,
            schema_error: None,
        }
    }
}

/// What a response contains, from its `Content-Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    /// HTML or XHTML.
    Html,
    /// Any `image/*`.
    Image,
    /// `text/css`.
    Css,
    /// JavaScript.
    JavaScript,
    /// `application/pdf`.
    Pdf,
    /// Web fonts.
    Font,
    /// XML (sitemaps, feeds).
    Xml,
    /// Anything else.
    Other,
    /// No response or no `Content-Type`.
    Unknown,
}

impl ContentKind {
    /// Classifies a `Content-Type` header value.
    pub fn of(content_type: Option<&str>) -> Self {
        let Some(content_type) = content_type else {
            return Self::Unknown;
        };
        let mime = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match mime.as_str() {
            "" => Self::Unknown,
            "text/html" | "application/xhtml+xml" => Self::Html,
            "text/css" => Self::Css,
            "application/pdf" => Self::Pdf,
            "text/javascript"
            | "application/javascript"
            | "application/x-javascript"
            | "application/ecmascript"
            | "text/ecmascript" => Self::JavaScript,
            "application/xml" | "text/xml" | "application/rss+xml" | "application/atom+xml" => {
                Self::Xml
            }
            mime if mime.starts_with("image/") => Self::Image,
            mime if mime.starts_with("font/") || mime.contains("font") => Self::Font,
            _ => Self::Other,
        }
    }

    /// Lowercase name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Image => "image",
            Self::Css => "css",
            Self::JavaScript => "javascript",
            Self::Pdf => "pdf",
            Self::Font => "font",
            Self::Xml => "xml",
            Self::Other => "other",
            Self::Unknown => "unknown",
        }
    }
}

/// Whether search engines may index a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Indexability {
    /// A 2xx response without noindex whose canonical (if any) is itself.
    Indexable,
    /// Disallowed by robots.txt.
    BlockedByRobots,
    /// No HTTP response.
    NoResponse,
    /// A 301, 302, 303, 307 or 308 response.
    Redirected,
    /// 4xx.
    ClientError,
    /// 5xx.
    ServerError,
    /// Another HTTP response, including 3xx without automatic navigation semantics.
    OtherHttp,
    /// A `noindex` directive (meta tag or `X-Robots-Tag`).
    Noindex,
    /// The canonical points to another URL.
    Canonicalised,
}

impl Indexability {
    /// Whether the URL is indexable.
    pub fn is_indexable(self) -> bool {
        self == Self::Indexable
    }

    /// Kebab-case name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Indexable => "indexable",
            Self::BlockedByRobots => "blocked-by-robots",
            Self::NoResponse => "no-response",
            Self::Redirected => "redirected",
            Self::ClientError => "client-error",
            Self::ServerError => "server-error",
            Self::OtherHttp => "other-http",
            Self::Noindex => "noindex",
            Self::Canonicalised => "canonicalised",
        }
    }
}

/// How a page references another URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrawlLinkKind {
    /// `<a href>` or `<area href>`.
    Hyperlink,
    /// `<img src>`.
    Image,
    /// `<link rel="stylesheet">`.
    Stylesheet,
    /// `<script src>`.
    Script,
    /// `<iframe src>`.
    Iframe,
    /// `<video>`, `<audio>` or `<source>`.
    Media,
    /// `rel="canonical"` (HTML or HTTP header).
    Canonical,
    /// `hreflang` alternate (HTML or HTTP header).
    Hreflang,
    /// `rel="next"`.
    Next,
    /// `rel="prev"`.
    Prev,
    /// `<meta http-equiv="refresh">`.
    MetaRefresh,
    /// `Location` of a 3xx response.
    Redirect,
    /// `rel="amphtml"`.
    Amp,
}

impl CrawlLinkKind {
    /// Kebab-case name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hyperlink => "hyperlink",
            Self::Image => "image",
            Self::Stylesheet => "stylesheet",
            Self::Script => "script",
            Self::Iframe => "iframe",
            Self::Media => "media",
            Self::Canonical => "canonical",
            Self::Hreflang => "hreflang",
            Self::Next => "next",
            Self::Prev => "prev",
            Self::MetaRefresh => "meta-refresh",
            Self::Redirect => "redirect",
            Self::Amp => "amp",
        }
    }

    /// Resources a page loads, as opposed to references to other documents.
    pub fn is_resource(self) -> bool {
        matches!(
            self,
            Self::Image | Self::Stylesheet | Self::Script | Self::Iframe | Self::Media
        )
    }
}

/// One reference from a crawled page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct LinkRecord {
    /// The page containing the reference.
    pub source: UrlId,
    /// The referenced URL's identifier.
    pub target: UrlId,
    /// The referenced URL.
    pub target_url: String,
    /// How it is referenced.
    pub kind: CrawlLinkKind,
    /// Anchor text, an image's alt text, or the language code of an hreflang alternate.
    pub text: String,
    /// Observed hyperlink naming evidence; resources and header references have none.
    pub name: Option<octopuson_html::LinkName>,
    /// `rel="nofollow"`.
    pub nofollow: bool,
    /// `rel="ugc"`.
    pub ugc: bool,
    /// `rel="sponsored"`.
    pub sponsored: bool,
    /// The target is on the crawled site.
    pub internal: bool,
    /// Order of the reference within the page.
    pub position: u32,
}

/// The robots.txt of one origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsRecord {
    /// `https://example.com/robots.txt`.
    pub url: String,
    /// How it was answered.
    pub status: RobotsStatus,
    /// Last received HTTP status, including a redirect before a network/chain failure.
    pub http_status: Option<u16>,
    /// The file's text (empty unless found).
    pub text: String,
    /// `Sitemap:` lines.
    pub sitemaps: Vec<String>,
}

/// How a robots.txt request was answered (REP, with conservative HTTP 429 handling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RobotsStatus {
    /// 2xx: its rules apply.
    Found,
    /// 4xx except 429: no robots restrictions were found.
    Missing,
    /// 429, 5xx, unusable redirect chain or no response: Respect blocks this origin for
    /// the current crawl.
    Unreachable,
}

impl RobotsStatus {
    /// Kebab-case name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::Missing => "missing",
            Self::Unreachable => "unreachable",
        }
    }
}

/// One fetched sitemap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SitemapRecord {
    /// The sitemap's URL.
    pub url: String,
    /// HTTP status; `None` without a response.
    pub status: Option<u16>,
    /// Original Location header, retained independently of optional header storage.
    pub redirect_location: Option<String>,
    /// Resolved redirect destination; HTTP fragments are removed before scheduling.
    pub redirect_url: Option<String>,
    /// What it contains; `None` when it could not be read.
    pub kind: Option<SitemapKind>,
    /// Why it could not be read.
    pub error: Option<String>,
    /// Page URLs it lists.
    pub entries: Vec<SitemapEntry>,
    /// Child sitemaps of an index.
    pub children: Vec<String>,
}

/// The format of a sitemap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SitemapKind {
    /// `<urlset>`.
    UrlSet,
    /// `<sitemapindex>`.
    Index,
    /// One URL per line.
    Text,
}

impl SitemapKind {
    /// Kebab-case name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UrlSet => "urlset",
            Self::Index => "index",
            Self::Text => "text",
        }
    }
}

/// A page URL a sitemap lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SitemapEntry {
    /// `<loc>`.
    pub url: String,
    /// `<lastmod>` as written.
    pub lastmod: Option<String>,
}

/// A well-known file of a start origin, such as `/llms.txt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteFileRecord {
    /// The file's URL.
    pub url: String,
    /// HTTP status; `None` without a response.
    pub status: Option<u16>,
    /// `Content-Type` header.
    pub content_type: Option<String>,
    /// The body of a 2xx response.
    pub text: String,
    /// Why no response arrived.
    pub error: Option<String>,
}

/// Rendering outcome and comparison with server-returned HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderRecord {
    /// Rendering succeeded, so facts/links use the resulting DOM.
    pub rendered: bool,
    /// URL after browser execution.
    pub url: Option<String>,
    /// Why the raw HTML was used instead of a rendered DOM.
    pub error: Option<String>,
    /// Rendering duration, including time waiting for a browser page.
    pub elapsed_ms: u32,
    /// The serialized DOM exceeded the configured body limit.
    pub truncated: bool,
    /// Server-returned title before JavaScript execution.
    pub raw_title: Option<String>,
    /// Meta description before JavaScript execution.
    pub raw_meta_description: Option<String>,
    /// Original HTML canonical, falling back to the HTTP Link canonical.
    pub raw_canonical: Option<String>,
    /// Original HTML plus HTTP indexing directives; None for older imported diagnostics.
    pub raw_noindex: Option<bool>,
    /// Original HTML plus HTTP follow directives; None for older imported diagnostics.
    pub raw_nofollow: Option<bool>,
    /// Words in the original HTML.
    pub raw_word_count: u32,
    /// Hyperlinks in the original HTML.
    pub raw_link_count: u32,
    /// Hyperlinks after rendering, when successful.
    pub rendered_link_count: Option<u32>,
    /// Original HTML, only when HTML storage is enabled.
    pub raw_html: Option<String>,
}
