//! Reading a crawl: tabs and ad-hoc queries streamed into table sinks.

use std::path::Path;

use duckdb::arrow::datatypes::DataType;
use duckdb::types::{TimeUnit, ValueRef};
use duckdb::{AccessMode, Config, Connection};
use octopuson_core::table::{Cell, Column, ColumnKind, TableSink};
use octopuson_core::{Date, Timestamp};

use crate::error::CrawlStoreError;

/// A predefined view of a crawl, like the tabs of the Spider window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrawlTab {
    /// One near-duplicate content witness per eligible URL.
    ContentMatches,
    /// Raw versus JavaScript-rendered page facts and diagnostics.
    Rendering,
    /// Materialized technical SEO findings.
    Issues,
    /// Internal URLs with their page facts and link counts.
    Internal,
    /// External URLs that were checked.
    External,
    /// Status codes of every URL.
    ResponseCodes,
    /// Every reference between URLs.
    Links,
    /// Images per page.
    Images,
    /// Headings per page.
    Headings,
    /// hreflang annotations.
    Hreflang,
    /// Structured data blocks.
    StructuredData,
    /// Sitemaps read.
    Sitemaps,
    /// URLs listed in sitemaps.
    SitemapUrls,
    /// robots.txt files.
    Robots,
}

impl CrawlTab {
    /// Every tab.
    pub const ALL: [Self; 14] = [
        Self::ContentMatches,
        Self::Rendering,
        Self::Issues,
        Self::Internal,
        Self::External,
        Self::ResponseCodes,
        Self::Links,
        Self::Images,
        Self::Headings,
        Self::Hreflang,
        Self::StructuredData,
        Self::Sitemaps,
        Self::SitemapUrls,
        Self::Robots,
    ];

    /// Kebab-case name used by the CLI and file names.
    pub fn name(self) -> &'static str {
        match self {
            Self::ContentMatches => "content-matches",
            Self::Issues => "issues",
            Self::Rendering => "rendering",
            Self::Internal => "internal",
            Self::External => "external",
            Self::ResponseCodes => "response-codes",
            Self::Links => "links",
            Self::Images => "images",
            Self::Headings => "headings",
            Self::Hreflang => "hreflang",
            Self::StructuredData => "structured-data",
            Self::Sitemaps => "sitemaps",
            Self::SitemapUrls => "sitemap-urls",
            Self::Robots => "robots",
        }
    }

    /// The tab named `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|tab| tab.name().eq_ignore_ascii_case(name.trim()))
    }

    pub(crate) fn view(self) -> &'static str {
        match self {
            Self::ContentMatches => "tab_content_matches",
            Self::Issues => "tab_issues",
            Self::Rendering => "tab_rendering",
            Self::Internal => "tab_internal",
            Self::External => "tab_external",
            Self::ResponseCodes => "tab_response_codes",
            Self::Links => "tab_links",
            Self::Images => "tab_images",
            Self::Headings => "tab_headings",
            Self::Hreflang => "tab_hreflang",
            Self::StructuredData => "tab_structured_data",
            Self::Sitemaps => "tab_sitemaps",
            Self::SitemapUrls => "tab_sitemap_urls",
            Self::Robots => "tab_robots",
        }
    }

    pub(crate) fn source(self) -> String {
        match self {
            Self::Links => crate::link_queries::LINK_ROWS.into(),
            Self::Hreflang => crate::hreflang_context::rows(),
            Self::Issues => format!(
                "(SELECT issue_id,url AS address FROM {} ORDER BY issue_id,url)",
                crate::hreflang_context::findings()
            ),
            Self::Internal | Self::External => crate::link_queries::url_rows(self),
            _ => self.view().into(),
        }
    }
}

/// Read access to one crawl.
pub struct CrawlReader {
    pub(crate) connection: Connection,
    anchor_ready: std::sync::OnceLock<()>,
}

impl CrawlReader {
    pub(crate) fn open(database: &Path) -> Result<Self, CrawlStoreError> {
        let connection = connect(database, true)?;
        let version: i64 = connection.query_row(
            "SELECT coalesce(max(version),0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        let audit_required = version > 0
            && connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM crawl_info WHERE key='audit_required' AND value='1')",
                [],
                |row| row.get::<_, bool>(0),
            )?;
        if version < crate::schema::MIN_READ_VERSION || audit_required {
            drop(connection);
            let mut upgrade = connect(database, false)?;
            crate::schema::migrate(&mut upgrade)?;
            crate::audit::run(&upgrade)?;
            drop(upgrade);
            return Ok(Self::from_connection(connect(database, true)?));
        }
        Ok(Self::from_connection(connection))
    }

    pub(crate) fn from_connection(connection: Connection) -> Self {
        Self {
            connection,
            anchor_ready: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn prepare_anchors(&self) -> Result<(), CrawlStoreError> {
        if self.anchor_ready.get().is_none() {
            crate::anchor_context::prepare(&self.connection)?;
            crate::schema_checks::prepare(&self.connection)?;
            let _ = self.anchor_ready.set(());
        }
        Ok(())
    }

    /// Streams `tab` into `sink`; returns the row count.
    ///
    /// # Errors
    /// Database errors and sink failures.
    pub fn tab(&self, tab: CrawlTab, sink: &mut dyn TableSink) -> Result<u64, CrawlStoreError> {
        self.query(&format!("SELECT * FROM {}", tab.source()), sink)
    }

    /// Rows of `tab`.
    ///
    /// # Errors
    /// Database errors.
    pub fn count(&self, tab: CrawlTab) -> Result<u64, CrawlStoreError> {
        self.prepare_anchors()?;
        let count: i64 = self.connection.query_row(
            &format!("SELECT count(*) FROM {}", tab.source()),
            [],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Streams the result of a read-only SQL query over the crawl's tables and `tab_*` views
    /// into `sink`; returns the row count.
    ///
    /// # Errors
    /// Database errors (including invalid SQL) and sink failures.
    pub fn query(&self, sql: &str, sink: &mut dyn TableSink) -> Result<u64, CrawlStoreError> {
        self.prepare_anchors()?;
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = statement.query([])?;
        let columns: Vec<Column> = match rows.as_ref() {
            Some(statement) => (0..statement.column_count())
                .map(|index| {
                    let name = statement
                        .column_name(index)
                        .map_or_else(|_| format!("column_{index}"), Clone::clone);
                    Column::new(name, kind(&statement.column_type(index)))
                })
                .collect(),
            None => Vec::new(),
        };
        sink.begin(&columns).map_err(CrawlStoreError::Output)?;
        let mut cells = Vec::with_capacity(columns.len());
        let mut count = 0_u64;
        while let Some(row) = rows.next()? {
            cells.clear();
            for index in 0..columns.len() {
                cells.push(cell(row.get_ref(index)?));
            }
            sink.row(&cells).map_err(CrawlStoreError::Output)?;
            count += 1;
        }
        sink.finish().map_err(CrawlStoreError::Output)?;
        Ok(count)
    }
}

/// Opens a crawl database; extensions are never downloaded or loaded at runtime.
pub(crate) fn connect(database: &Path, read_only: bool) -> Result<Connection, CrawlStoreError> {
    let mut config = Config::default()
        .enable_autoload_extension(false)?
        .with("autoinstall_known_extensions", "false")?;
    if read_only {
        config = config.access_mode(AccessMode::ReadOnly)?;
    }
    Ok(Connection::open_with_flags(database, config)?)
}

fn kind(data_type: &DataType) -> ColumnKind {
    match data_type {
        DataType::Boolean => ColumnKind::Boolean,
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => ColumnKind::Integer,
        DataType::Float16 | DataType::Float32 | DataType::Float64 | DataType::Decimal128(..) => {
            ColumnKind::Float
        }
        DataType::Date32 | DataType::Date64 => ColumnKind::Date,
        DataType::Timestamp(..) => ColumnKind::Timestamp,
        _ => ColumnKind::Text,
    }
}

fn cell(value: ValueRef<'_>) -> Cell {
    match value {
        ValueRef::Null => Cell::Null,
        ValueRef::Boolean(flag) => Cell::Boolean(flag),
        ValueRef::TinyInt(n) => Cell::Integer(n.into()),
        ValueRef::SmallInt(n) => Cell::Integer(n.into()),
        ValueRef::Int(n) => Cell::Integer(n.into()),
        ValueRef::BigInt(n) => Cell::Integer(n),
        ValueRef::UTinyInt(n) => Cell::Integer(n.into()),
        ValueRef::USmallInt(n) => Cell::Integer(n.into()),
        ValueRef::UInt(n) => Cell::Integer(n.into()),
        ValueRef::UBigInt(n) => {
            i64::try_from(n).map_or_else(|_| Cell::Text(n.to_string()), Cell::Integer)
        }
        ValueRef::HugeInt(n) => {
            i64::try_from(n).map_or_else(|_| Cell::Text(n.to_string()), Cell::Integer)
        }
        ValueRef::Float(f) => Cell::Float(f.into()),
        ValueRef::Double(f) => Cell::Float(f),
        ValueRef::Text(bytes) => Cell::Text(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Date32(days) => Date::from_days_since_epoch(days).map_or(Cell::Null, Cell::Date),
        ValueRef::Timestamp(unit, raw) => {
            let micros = match unit {
                TimeUnit::Second => raw.saturating_mul(1_000_000),
                TimeUnit::Millisecond => raw.saturating_mul(1_000),
                TimeUnit::Microsecond => raw,
                TimeUnit::Nanosecond => raw / 1_000,
            };
            Cell::Timestamp(Timestamp::from_unix_micros(micros))
        }
        other => Cell::Text(format!("{other:?}")),
    }
}
