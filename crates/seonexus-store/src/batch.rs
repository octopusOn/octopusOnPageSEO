//! Columnar staging of parsed hits as Arrow record batches.
//!
//! Batches are built on worker threads (one per chunk) and appended to DuckDB on the import
//! thread, so the expensive row-to-column conversion runs in parallel.

use std::net::IpAddr;
use std::sync::{Arc, LazyLock};

use crate::error::StoreError;
use crate::site::BotTag;
use duckdb::arrow::array::{
    ArrayBuilder, ArrayRef, Int64Builder, StringBuilder, TimestampMicrosecondBuilder,
    UInt16Builder, UInt64Builder,
};
use duckdb::arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use duckdb::arrow::record_batch::RecordBatch;
use seonexus_logparse::LogRecord;

/// Column definitions of the staging table; must match [`STAGING_TABLE_SQL`] order.
static SCHEMA: LazyLock<SchemaRef> = LazyLock::new(|| {
    let text = |name: &str, nullable: bool| Field::new(name, DataType::Utf8, nullable);
    Arc::new(Schema::new(vec![
        Field::new(
            "ts",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        ),
        text("ip", false),
        text("method", false),
        text("target", false),
        text("protocol", false),
        Field::new("status", DataType::UInt16, false),
        Field::new("bytes", DataType::UInt64, true),
        text("referer", true),
        text("user_agent", true),
        Field::new("duration_us", DataType::Int64, true),
        text("host", true),
        text("router", true),
        text("upstream", true),
        Field::new("request_seq", DataType::UInt64, true),
        Field::new("line_hash", DataType::UInt64, false),
        text("bot", true),
        text("bot_category", true),
    ]))
});

/// DDL of the per-import staging table matching [`HitBatchBuilder`] batches.
pub(crate) const STAGING_TABLE_SQL: &str = "
    CREATE TEMP TABLE staging_hits (
        ts TIMESTAMP NOT NULL,
        ip VARCHAR NOT NULL,
        method VARCHAR NOT NULL,
        target VARCHAR NOT NULL,
        protocol VARCHAR NOT NULL,
        status USMALLINT NOT NULL,
        bytes UBIGINT,
        referer VARCHAR,
        user_agent VARCHAR,
        duration_us BIGINT,
        host VARCHAR,
        router VARCHAR,
        upstream VARCHAR,
        request_seq UBIGINT,
        line_hash UBIGINT NOT NULL,
        bot VARCHAR,
        bot_category VARCHAR
    );
";

/// A batch of hits ready to be appended to an import session.
#[derive(Debug, Clone)]
pub struct HitBatch {
    pub(crate) batch: RecordBatch,
}

impl HitBatch {
    /// Number of hits in the batch.
    pub fn len(&self) -> usize {
        self.batch.num_rows()
    }

    /// Whether the batch holds no hits.
    pub fn is_empty(&self) -> bool {
        self.batch.num_rows() == 0
    }
}

/// Accumulates [`LogRecord`]s column by column.
#[derive(Debug)]
pub struct HitBatchBuilder {
    ts: TimestampMicrosecondBuilder,
    ip: StringBuilder,
    method: StringBuilder,
    target: StringBuilder,
    protocol: StringBuilder,
    status: UInt16Builder,
    bytes: UInt64Builder,
    referer: StringBuilder,
    user_agent: StringBuilder,
    duration_us: Int64Builder,
    host: StringBuilder,
    router: StringBuilder,
    upstream: StringBuilder,
    request_seq: UInt64Builder,
    line_hash: UInt64Builder,
    bot: StringBuilder,
    bot_category: StringBuilder,
    ip_text: String,
}

impl Default for HitBatchBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HitBatchBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self {
            ts: TimestampMicrosecondBuilder::new(),
            ip: StringBuilder::new(),
            method: StringBuilder::new(),
            target: StringBuilder::new(),
            protocol: StringBuilder::new(),
            status: UInt16Builder::new(),
            bytes: UInt64Builder::new(),
            referer: StringBuilder::new(),
            user_agent: StringBuilder::new(),
            duration_us: Int64Builder::new(),
            host: StringBuilder::new(),
            router: StringBuilder::new(),
            upstream: StringBuilder::new(),
            request_seq: UInt64Builder::new(),
            line_hash: UInt64Builder::new(),
            bot: StringBuilder::new(),
            bot_category: StringBuilder::new(),
            ip_text: String::with_capacity(39),
        }
    }

    /// Appends one hit. `line_hash` identifies the raw log line for de-duplication; `bot` is
    /// the classification of its user agent.
    pub fn push(&mut self, record: &LogRecord<'_>, line_hash: u64, bot: Option<BotTag>) {
        self.ts.append_value(record.timestamp.as_unix_micros());
        self.ip
            .append_value(format_ip(record.client_ip, &mut self.ip_text));
        self.method.append_value(&record.method);
        self.target.append_value(&record.target);
        self.protocol.append_value(&record.protocol);
        self.status.append_value(record.status);
        self.bytes.append_option(record.bytes);
        self.referer.append_option(record.referer.as_deref());
        self.user_agent.append_option(record.user_agent.as_deref());
        self.duration_us
            .append_option(record.duration_micros.map(saturating_i64));
        self.host.append_option(record.host.as_deref());
        self.router.append_option(record.router.as_deref());
        self.upstream.append_option(record.upstream.as_deref());
        self.request_seq.append_option(record.request_seq);
        self.line_hash.append_value(line_hash);
        self.bot.append_option(bot.map(|tag| tag.id));
        self.bot_category.append_option(bot.map(|tag| tag.category));
    }

    /// Number of hits pushed since the last [`finish`](Self::finish).
    pub fn len(&self) -> usize {
        self.line_hash.len()
    }

    /// Whether no hits are pending.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Takes the accumulated hits as a batch, leaving the builder empty.
    ///
    /// # Errors
    /// Only on internal schema mismatches, which indicate a bug.
    pub fn finish(&mut self) -> Result<HitBatch, StoreError> {
        let columns: Vec<ArrayRef> = vec![
            Arc::new(self.ts.finish()),
            Arc::new(self.ip.finish()),
            Arc::new(self.method.finish()),
            Arc::new(self.target.finish()),
            Arc::new(self.protocol.finish()),
            Arc::new(self.status.finish()),
            Arc::new(self.bytes.finish()),
            Arc::new(self.referer.finish()),
            Arc::new(self.user_agent.finish()),
            Arc::new(self.duration_us.finish()),
            Arc::new(self.host.finish()),
            Arc::new(self.router.finish()),
            Arc::new(self.upstream.finish()),
            Arc::new(self.request_seq.finish()),
            Arc::new(self.line_hash.finish()),
            Arc::new(self.bot.finish()),
            Arc::new(self.bot_category.finish()),
        ];
        let batch = RecordBatch::try_new(Arc::clone(&SCHEMA), columns)?;
        Ok(HitBatch { batch })
    }
}

fn format_ip(ip: IpAddr, buffer: &mut String) -> &str {
    use std::fmt::Write;
    buffer.clear();
    // Writing to a String cannot fail.
    let _ = write!(buffer, "{ip}");
    buffer
}

fn saturating_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use seonexus_core::Timestamp;

    use super::*;

    fn record() -> LogRecord<'static> {
        LogRecord {
            timestamp: Timestamp::from_unix_micros(1_000_000),
            client_ip: "66.249.66.1".parse().unwrap(),
            method: Cow::Borrowed("GET"),
            target: Cow::Borrowed("/a"),
            protocol: Cow::Borrowed("HTTP/1.1"),
            status: 200,
            bytes: Some(10),
            referer: None,
            user_agent: Some(Cow::Borrowed("Googlebot")),
            duration_micros: Some(5),
            host: None,
            router: None,
            upstream: None,
            request_seq: None,
        }
    }

    #[test]
    fn builds_batches_and_resets() {
        let mut builder = HitBatchBuilder::new();
        builder.push(&record(), 1, None);
        let tag = BotTag {
            id: "googlebot-desktop",
            category: "search",
        };
        builder.push(&record(), 2, Some(tag));
        assert_eq!(builder.len(), 2);
        let batch = builder.finish().unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.batch.num_columns(), SCHEMA.fields().len());
        assert!(builder.is_empty());
        assert!(builder.finish().unwrap().is_empty());
    }
}
