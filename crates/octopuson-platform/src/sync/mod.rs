//! Keeping copies of a workspace the same: the desktop program and the account's web workspace.
//!
//! The web workspace is the hub; every desktop is a copy that sends its changes there and takes
//! the others' back. Records travel by a `uid` that is the same on every copy ([`kinds`]):
//! random for things made apart (a project, a report), drawn from what makes them unique where
//! the same thing may be made on two computers (a tracked keyword). Local ids never travel.
//!
//! Nothing in the program writes differently because of syncing. A scan ([`PlatformStore::sync_scan`])
//! compares every travelling record with the fingerprint it had last time (`sync_rows`) and gives
//! what changed a new change number. Large records that never change once finished — a rank run
//! with its checks, an AI run with its answers, a project's Search Console numbers — travel
//! whole, and only when finished, so a run still going on one computer is never touched by
//! another.
//!
//! When both sides changed a record, the later change wins, except that a deletion wins over an
//! edit of a record made apart, and times the program writes itself (last run) keep the later
//! value whichever side wins, so nothing scheduled runs twice. A change that refers to a
//! record not here yet waits in `sync_pending`.

mod apply;
pub(crate) mod kinds;
mod sql;

use duckdb::params;
use duckdb::types::Value as SqlValue;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{PlatformStore, PlatformStoreError};

use kinds::{GSC_TABLES, KINDS, Kind, Removal, Shape, kind_named};

/// Name of the change counter in `sync_state`.
const SEQ: &str = "seq";

/// Most changes looked at for one batch.
const BATCH_ROWS: usize = 2_000;

/// One record as it travels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncChange {
    /// Its kind (`project`, `tracked_keyword` …).
    pub kind: String,
    /// Its key, the same on every copy.
    pub key: String,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// When it changed, Unix milliseconds on the hub's clock.
    pub at: i64,
    /// The sender's change number.
    pub seq: i64,
    /// From a copy: the hub's change number of the version the copy changed (0: none).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub base: i64,
    /// Its fields, with the records it refers to named by uid; none when deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
const fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// Which side of a sync takes changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSide<'a> {
    /// The account's web workspace, taking a copy's changes.
    Hub {
        /// The copy sending them.
        replica: &'a str,
    },
    /// A copy, taking the hub's versions.
    Copy {
        /// The copy's last change already sent to the hub.
        pushed: i64,
    },
}

/// A change the hub took as it was sent, with the hub's number for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncAck {
    /// Its kind.
    pub kind: String,
    /// Its key.
    pub key: String,
    /// The hub's change number.
    pub seq: i64,
}

/// What taking a set of changes did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncApplied {
    /// Records written or deleted here.
    pub applied: usize,
    /// Hub: changes taken as sent.
    pub accepted: Vec<SyncAck>,
    /// Hub: records whose own version goes back instead (kind, key).
    pub rejected: Vec<(String, String)>,
    /// Changes waiting for a record they refer to.
    pub parked: usize,
    /// Changes dropped: what they referred to is gone.
    pub dropped: usize,
    /// Crawl folders of deleted audits, for the caller to delete.
    pub removed_crawls: Vec<String>,
    /// Report links that stopped working, for the caller to forget.
    pub removed_shares: Vec<String>,
}

impl SyncApplied {
    /// Adds what another set of changes did.
    pub fn absorb(&mut self, other: Self) {
        self.applied += other.applied;
        self.accepted.extend(other.accepted);
        self.rejected.extend(other.rejected);
        self.parked += other.parked;
        self.dropped += other.dropped;
        self.removed_crawls.extend(other.removed_crawls);
        self.removed_shares.extend(other.removed_shares);
    }
}

/// Changes to send.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncBatch {
    /// The changes, oldest first.
    pub changes: Vec<SyncChange>,
    /// The change number of the last one looked at (to continue after).
    pub last: i64,
    /// Whether more are waiting.
    pub more: bool,
}

/// A finished audit with a crawl, as crawls travel by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditCrawl {
    /// The audit's uid.
    pub uid: String,
    /// Its project's uid.
    pub project: String,
    /// Its crawl folder.
    pub crawl_id: String,
    /// RFC 3339.
    pub started_at: String,
}

/// What a record was last seen as.
#[derive(Debug, Clone)]
struct Entry {
    hash: Option<String>,
    seq: i64,
    changed_at: i64,
    origin: String,
    remote_seq: i64,
    deleted: bool,
}

/// A value read from the database as JSON.
fn to_json(value: SqlValue) -> Result<Value, PlatformStoreError> {
    Ok(match value {
        SqlValue::Null => Value::Null,
        SqlValue::Boolean(value) => Value::Bool(value),
        SqlValue::TinyInt(value) => Value::from(value),
        SqlValue::SmallInt(value) => Value::from(value),
        SqlValue::Int(value) => Value::from(value),
        SqlValue::BigInt(value) => Value::from(value),
        SqlValue::UTinyInt(value) => Value::from(value),
        SqlValue::USmallInt(value) => Value::from(value),
        SqlValue::UInt(value) => Value::from(value),
        SqlValue::UBigInt(value) => Value::from(value),
        SqlValue::Float(value) => {
            serde_json::Number::from_f64(f64::from(value)).map_or(Value::Null, Value::Number)
        }
        SqlValue::Double(value) => {
            serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
        }
        SqlValue::Text(value) => Value::String(value),
        other => {
            return Err(PlatformStoreError::Corrupt(format!(
                "a value of an unexpected type travels: {other:?}"
            )));
        }
    })
}

/// A JSON value to write.
fn to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Boolean(*value),
        Value::Number(number) => number.as_i64().map_or_else(
            || {
                number.as_u64().map_or_else(
                    || SqlValue::Double(number.as_f64().unwrap_or_default()),
                    SqlValue::UBigInt,
                )
            },
            SqlValue::BigInt,
        ),
        Value::String(text) => SqlValue::Text(text.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

impl PlatformStore {
    /// A value the sync keeps (its cursor, the hub's epoch …).
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_value(&self, name: &str) -> Result<Option<String>, PlatformStoreError> {
        let mut statement = self
            .connection
            .prepare_cached("SELECT value FROM sync_state WHERE name = ?")?;
        let mut rows = statement.query_map(params![name], |row| row.get(0))?;
        Ok(rows.next().transpose()?)
    }

    /// Keeps a value of the sync; `None` forgets it.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_sync_value(
        &self,
        name: &str,
        value: Option<&str>,
    ) -> Result<(), PlatformStoreError> {
        match value {
            Some(value) => self.connection.execute(
                "INSERT INTO sync_state (name, value) VALUES (?, ?)
                 ON CONFLICT (name) DO UPDATE SET value = excluded.value",
                params![name, value],
            )?,
            None => self
                .connection
                .execute("DELETE FROM sync_state WHERE name = ?", params![name])?,
        };
        Ok(())
    }

    /// The latest change number given here.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_head(&self) -> Result<i64, PlatformStoreError> {
        Ok(self
            .sync_value(SEQ)?
            .and_then(|value| value.parse().ok())
            .unwrap_or(0))
    }

    /// Reserves `count` change numbers; returns the number before the first.
    fn reserve(&self, count: i64) -> Result<i64, PlatformStoreError> {
        let head = self.sync_head()?;
        if count > 0 {
            self.set_sync_value(SEQ, Some(&(head + count).to_string()))?;
        }
        Ok(head)
    }

    /// Gives every travelling record a uid.
    fn fill_uids(&self) -> Result<(), PlatformStoreError> {
        for kind in KINDS {
            if let Some(fill) = sql::fill(kind) {
                self.connection.execute(&fill, [])?;
            }
        }
        Ok(())
    }

    /// Notes what changed since the last scan; returns how many changes were noted.
    ///
    /// # Errors
    /// Database errors; nothing is noted then.
    pub fn sync_scan(&self, now_ms: i64) -> Result<u64, PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        self.fill_uids()?;
        let mut noted = 0_u64;
        for kind in KINDS {
            noted += self.scan_kind(kind, now_ms)?;
        }
        transaction.commit()?;
        Ok(noted)
    }

    /// Takes the fingerprints of the kinds named `kinds` as they are now without noting a change:
    /// after an update gave them new columns, records that only gained those columns are not
    /// this copy's changes (the hub sends their values again).
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_rebaseline(&self, kinds: &[String]) -> Result<(), PlatformStoreError> {
        for kind in kinds.iter().filter_map(|name| kind_named(name)) {
            self.connection.execute_batch(&format!(
                "CREATE OR REPLACE TEMP TABLE sync_now AS {}",
                sql::listing(kind)
            ))?;
            self.connection.execute(
                "UPDATE sync_rows SET hash = n.hash FROM sync_now n
                 WHERE sync_rows.kind = ? AND sync_rows.key = n.key AND NOT sync_rows.deleted",
                params![kind.name],
            )?;
            self.connection.execute_batch("DROP TABLE sync_now")?;
        }
        Ok(())
    }

    fn scan_kind(&self, kind: &Kind, now_ms: i64) -> Result<u64, PlatformStoreError> {
        self.connection.execute_batch(&format!(
            "CREATE OR REPLACE TEMP TABLE sync_now AS {}",
            sql::listing(kind)
        ))?;
        let changed: i64 = self.connection.query_row(
            "SELECT count(*) FROM sync_now n LEFT JOIN sync_rows s ON s.kind = ? AND s.key = n.key
             WHERE s.key IS NULL OR s.deleted OR s.hash IS DISTINCT FROM n.hash",
            params![kind.name],
            |row| row.get(0),
        )?;
        let base = self.reserve(changed)?;
        if changed > 0 {
            self.connection.execute(
                "INSERT INTO sync_rows (kind, key, parent, hash, seq, changed_at, origin, deleted)
                 SELECT ?, n.key, n.parent, n.hash, ? + row_number() OVER (ORDER BY n.key), ?, '', false
                 FROM sync_now n LEFT JOIN sync_rows s ON s.kind = ? AND s.key = n.key
                 WHERE s.key IS NULL OR s.deleted OR s.hash IS DISTINCT FROM n.hash
                 ON CONFLICT (kind, key) DO UPDATE SET parent = excluded.parent, hash = excluded.hash,
                     seq = excluded.seq, changed_at = excluded.changed_at, origin = '', deleted = false",
                params![kind.name, base, now_ms, kind.name],
            )?;
        }
        // Gone without a word: kept answers that expired, and what went with a deleted parent
        // (deleting the parent deletes them on every copy).
        let silent = matches!(kind.removal, Removal::Local);
        self.connection.execute(
            "DELETE FROM sync_rows WHERE kind = ? AND NOT deleted
               AND key NOT IN (SELECT key FROM sync_now)
               AND (? OR (parent IS NOT NULL AND NOT EXISTS (
                   SELECT 1 FROM sync_rows q WHERE q.kind || '/' || q.key = sync_rows.parent AND NOT q.deleted)))",
            params![kind.name, silent],
        )?;
        let removed: i64 = self.connection.query_row(
            "SELECT count(*) FROM sync_rows WHERE kind = ? AND NOT deleted AND key NOT IN (SELECT key FROM sync_now)",
            params![kind.name],
            |row| row.get(0),
        )?;
        let removed_base = self.reserve(removed)?;
        if removed > 0 {
            self.connection.execute(
                // Numbered: DuckDB binds an UPDATE's FROM before its SET.
                "UPDATE sync_rows SET deleted = true, hash = NULL, origin = '', changed_at = $1, seq = d.seq
                 FROM (SELECT key AS gone, $2 + row_number() OVER (ORDER BY key) AS seq FROM sync_rows
                       WHERE kind = $3 AND NOT deleted AND key NOT IN (SELECT key FROM sync_now)) AS d
                 WHERE sync_rows.kind = $3 AND sync_rows.key = d.gone",
                params![now_ms, removed_base, kind.name],
            )?;
        }
        self.connection.execute_batch("DROP TABLE sync_now")?;
        Ok(u64::try_from(changed + removed).unwrap_or_default())
    }

    fn entry(&self, kind: &str, key: &str) -> Result<Option<Entry>, PlatformStoreError> {
        let mut statement = self.connection.prepare_cached(
            "SELECT hash, seq, changed_at, origin, remote_seq, deleted FROM sync_rows WHERE kind = ? AND key = ?",
        )?;
        let mut rows = statement.query_map(params![kind, key], |row| {
            Ok(Entry {
                hash: row.get(0)?,
                seq: row.get(1)?,
                changed_at: row.get(2)?,
                origin: row.get(3)?,
                remote_seq: row.get(4)?,
                deleted: row.get(5)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    /// A record's fingerprint and parent as a scan would see it now; `None` when it does not
    /// travel (absent, or running).
    fn current(
        &self,
        kind: &Kind,
        key: &str,
    ) -> Result<Option<(String, Option<String>)>, PlatformStoreError> {
        let mut statement = self.connection.prepare_cached(&sql::one(kind))?;
        let mut rows = statement.query_map(params![key], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.next().transpose()?)
    }

    /// A record's data as it travels; `None` when it does not travel now.
    fn read_record(&self, kind: &Kind, key: &str) -> Result<Option<Value>, PlatformStoreError> {
        if matches!(kind.shape, Shape::Gsc) {
            return self.read_gsc(key);
        }
        let found = {
            let mut statement = self.connection.prepare_cached(&sql::read(kind))?;
            let width = kind.columns.len() + 1;
            let mut rows = statement.query_map(params![key], |row| {
                (0..width)
                    .map(|index| row.get::<_, SqlValue>(index))
                    .collect::<Result<Vec<_>, _>>()
            })?;
            rows.next().transpose()?
        };
        let Some(values) = found else {
            return Ok(None);
        };
        let mut values = values.into_iter();
        let mut data = Map::new();
        let parent = to_json(values.next().unwrap_or(SqlValue::Null))?;
        if let Some(field) = kind.parent.map(|parent| parent.field) {
            data.insert(field.to_owned(), parent);
        }
        for (name, value) in kind.columns.iter().zip(values) {
            data.insert((*name).to_owned(), to_json(value)?);
        }
        if let Shape::Run(children) = kind.shape {
            data.insert("rows".to_owned(), self.read_run_rows(kind, children, key)?);
        }
        Ok(Some(Value::Object(data)))
    }

    /// A run's rows, each an array: the uids it refers to, then its columns.
    fn read_run_rows(
        &self,
        kind: &Kind,
        children: &kinds::Children,
        key: &str,
    ) -> Result<Value, PlatformStoreError> {
        let mut select: Vec<String> = Vec::new();
        let mut joins: Vec<String> = Vec::new();
        for (index, (column, _, table)) in children.refs.iter().enumerate() {
            select.push(format!("r{index}.uid"));
            joins.push(format!(
                "JOIN {table} AS r{index} ON r{index}.id = c.\"{column}\""
            ));
        }
        select.extend(children.columns.iter().map(|name| format!("c.\"{name}\"")));
        let query = format!(
            "SELECT {} FROM {} AS c JOIN {} AS run ON run.id = c.run_id {} WHERE run.uid = ? ORDER BY c.checked_at",
            select.join(", "),
            children.table,
            kind.table,
            joins.join(" ")
        );
        let width = select.len();
        let mut statement = self.connection.prepare_cached(&query)?;
        let rows = statement.query_map(params![key], |row| {
            (0..width)
                .map(|index| row.get::<_, SqlValue>(index))
                .collect::<Result<Vec<_>, _>>()
        })?;
        let mut out = Vec::new();
        for row in rows {
            let values = row?
                .into_iter()
                .map(to_json)
                .collect::<Result<Vec<_>, _>>()?;
            out.push(Value::Array(values));
        }
        Ok(Value::Array(out))
    }

    /// A project's Search Console and Analytics numbers as they travel.
    fn read_gsc(&self, project: &str) -> Result<Option<Value>, PlatformStoreError> {
        let Some(project_id) = self.id_by_uid("projects", project)? else {
            return Ok(None);
        };
        let mut data = Map::new();
        data.insert("project".to_owned(), Value::String(project.to_owned()));
        let mut any = false;
        for table in GSC_TABLES {
            let columns: Vec<String> = table
                .columns
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect();
            let mut statement = self.connection.prepare_cached(&format!(
                "SELECT {} FROM {} WHERE project_id = ?",
                columns.join(", "),
                table.table
            ))?;
            let width = columns.len();
            let rows = statement.query_map(params![project_id], |row| {
                (0..width)
                    .map(|index| row.get::<_, SqlValue>(index))
                    .collect::<Result<Vec<_>, _>>()
            })?;
            let mut out = Vec::new();
            for row in rows {
                let values = row?
                    .into_iter()
                    .map(to_json)
                    .collect::<Result<Vec<_>, _>>()?;
                out.push(Value::Array(values));
            }
            any |= !out.is_empty();
            data.insert(table.field.to_owned(), Value::Array(out));
        }
        Ok(any.then_some(Value::Object(data)))
    }

    /// The local id of a record of `table` by uid.
    fn id_by_uid(&self, table: &str, uid: &str) -> Result<Option<i64>, PlatformStoreError> {
        let mut statement = self
            .connection
            .prepare_cached(&format!("SELECT id FROM {table} WHERE uid = ?"))?;
        let mut rows = statement.query_map(params![uid], |row| row.get(0))?;
        Ok(rows.next().transpose()?)
    }

    /// The change as it travels, read now; `None` when the record stopped travelling.
    fn outgoing(
        &self,
        kind: &Kind,
        key: &str,
        entry: &Entry,
        at: i64,
    ) -> Result<Option<SyncChange>, PlatformStoreError> {
        let data = if entry.deleted {
            None
        } else {
            match self.read_record(kind, key)? {
                Some(data) => Some(data),
                None => return Ok(None),
            }
        };
        Ok(Some(SyncChange {
            kind: kind.name.to_owned(),
            key: key.to_owned(),
            deleted: entry.deleted,
            at,
            seq: entry.seq,
            base: 0,
            data,
        }))
    }

    /// Changes in `sync_rows` matching `condition` (with `parameters`), oldest first, up to
    /// about `max_bytes` of JSON (always at least one).
    fn batch(
        &self,
        condition: &str,
        parameters: &[&dyn duckdb::ToSql],
        offset_ms: i64,
        max_bytes: usize,
        copy: bool,
    ) -> Result<SyncBatch, PlatformStoreError> {
        // Read first: the records are read with other queries.
        let rows: Vec<(String, String, Entry)> = {
            let mut statement = self.connection.prepare(&format!(
                "SELECT kind, key, hash, seq, changed_at, origin, remote_seq, deleted FROM sync_rows
                 WHERE {condition} ORDER BY seq LIMIT {BATCH_ROWS}"
            ))?;
            let rows = statement.query_map(parameters, |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    Entry {
                        hash: row.get(2)?,
                        seq: row.get(3)?,
                        changed_at: row.get(4)?,
                        origin: row.get(5)?,
                        remote_seq: row.get(6)?,
                        deleted: row.get(7)?,
                    },
                ))
            })?;
            rows.collect::<Result<_, _>>()?
        };
        let mut batch = SyncBatch {
            more: rows.len() == BATCH_ROWS,
            ..SyncBatch::default()
        };
        let mut bytes = 0_usize;
        for (kind, key, entry) in rows {
            if !batch.changes.is_empty() && bytes >= max_bytes {
                batch.more = true;
                break;
            }
            batch.last = entry.seq;
            let Some(kind) = kind_named(&kind) else {
                continue;
            };
            let Some(mut change) =
                self.outgoing(kind, &key, &entry, entry.changed_at + offset_ms)?
            else {
                continue;
            };
            if copy {
                change.base = entry.remote_seq;
            }
            bytes += change
                .data
                .as_ref()
                .map_or(64, |data| data.to_string().len() + 64);
            batch.changes.push(change);
        }
        Ok(batch)
    }

    /// A copy's own changes after its change `after`, to send to the hub. `offset_ms` turns
    /// this computer's clock into the hub's.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_local_changes(
        &self,
        after: i64,
        offset_ms: i64,
        max_bytes: usize,
    ) -> Result<SyncBatch, PlatformStoreError> {
        self.fill_uids()?;
        self.batch(
            "origin = '' AND seq > ?",
            &[&after],
            offset_ms,
            max_bytes,
            true,
        )
    }

    /// The hub's changes after its change `cursor` that did not come from `replica`.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_hub_changes(
        &self,
        cursor: i64,
        replica: &str,
        max_bytes: usize,
    ) -> Result<SyncBatch, PlatformStoreError> {
        self.fill_uids()?;
        let mut batch = self.batch(
            "seq > ? AND origin <> ?",
            &[&cursor, &replica],
            0,
            max_bytes,
            false,
        )?;
        if !batch.more {
            // Changes of this copy's own after the last one sent still move the cursor.
            batch.last = batch.last.max(self.sync_head()?);
        }
        Ok(batch)
    }

    /// Every current record of the kinds named `kinds` after change `after`, oldest first: for a
    /// copy that learnt these kinds (or their new columns) after it took their records.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_kind_records(
        &self,
        kinds: &[String],
        after: i64,
        max_bytes: usize,
    ) -> Result<SyncBatch, PlatformStoreError> {
        // Only names this program knows reach the query, so they can be written into it.
        let known: Vec<String> = kinds
            .iter()
            .filter_map(|name| kind_named(name))
            .map(|kind| format!("'{}'", kind.name))
            .collect();
        if known.is_empty() {
            return Ok(SyncBatch::default());
        }
        self.fill_uids()?;
        self.batch(
            &format!(
                "deleted = false AND seq > ? AND kind IN ({})",
                known.join(", ")
            ),
            &[&after],
            0,
            max_bytes,
            false,
        )
    }

    /// The hub's own versions of records (to send back when a copy's change lost).
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_versions(
        &self,
        keys: &[(String, String)],
    ) -> Result<Vec<SyncChange>, PlatformStoreError> {
        let mut out = Vec::new();
        for (kind, key) in keys {
            let (Some(kind), Some(entry)) = (kind_named(kind), self.entry(kind, key)?) else {
                continue;
            };
            if let Some(change) = self.outgoing(kind, key, &entry, entry.changed_at)? {
                out.push(change);
            }
        }
        Ok(out)
    }

    /// Notes the hub's numbers of changes it took as sent.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_acknowledge(&self, acks: &[SyncAck]) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for ack in acks {
            self.connection.execute(
                "UPDATE sync_rows SET remote_seq = ? WHERE kind = ? AND key = ?",
                params![ack.seq, ack.kind, ack.key],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// How many own changes after `after` wait to be sent.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_unsent(&self, after: i64) -> Result<u64, PlatformStoreError> {
        let count: i64 = self.connection.query_row(
            "SELECT count(*) FROM sync_rows WHERE origin = '' AND seq > ?",
            params![after],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    /// How many changes wait for a record they refer to.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_parked(&self) -> Result<u64, PlatformStoreError> {
        let count: i64 =
            self.connection
                .query_row("SELECT count(*) FROM sync_pending", [], |row| row.get(0))?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    /// Forgets everything the sync noted (not the data): the next scan sees every record as
    /// new. Values named in `keep` stay.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_forget(&self, keep: &[&str]) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        self.connection
            .execute_batch("DELETE FROM sync_rows; DELETE FROM sync_pending;")?;
        let names: Vec<String> = {
            let mut statement = self.connection.prepare("SELECT name FROM sync_state")?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for name in names {
            if !keep.contains(&name.as_str()) {
                self.connection
                    .execute("DELETE FROM sync_state WHERE name = ?", params![name])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Deletes every project with what it holds, the report branding and the kept answers (to
    /// take another copy's data instead); returns the crawl folders to delete.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_clear(&self) -> Result<Vec<String>, PlatformStoreError> {
        let mut crawls = Vec::new();
        for project in self.projects()? {
            crawls.extend(self.audit_crawls(project.id)?);
            self.delete_project(project.id)?;
        }
        self.connection
            .execute_batch("DELETE FROM report_branding; DELETE FROM cache;")?;
        Ok(crawls)
    }

    /// Finished audits with a crawl, oldest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_audit_crawls(&self) -> Result<Vec<AuditCrawl>, PlatformStoreError> {
        self.fill_uids()?;
        let mut statement = self.connection.prepare(
            "SELECT a.uid, p.uid, a.crawl_id, a.started_at FROM audits a JOIN projects p ON p.id = a.project_id
             WHERE a.crawl_id IS NOT NULL AND a.status IN ('done', 'stopped') AND a.uid IS NOT NULL
             ORDER BY a.started_at, a.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AuditCrawl {
                uid: row.get(0)?,
                project: row.get(1)?,
                crawl_id: row.get(2)?,
                started_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The crawl folder of the finished audit `uid`.
    ///
    /// # Errors
    /// Database errors.
    pub fn sync_audit_crawl(&self, uid: &str) -> Result<Option<String>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT crawl_id FROM audits WHERE uid = ? AND crawl_id IS NOT NULL AND status IN ('done', 'stopped')",
        )?;
        let mut rows = statement.query_map(params![uid], |row| row.get(0))?;
        Ok(rows.next().transpose()?)
    }

    /// How many projects the workspace has.
    ///
    /// # Errors
    /// Database errors.
    pub fn project_count(&self) -> Result<u64, PlatformStoreError> {
        let count: i64 = self
            .connection
            .query_row("SELECT count(*) FROM projects", [], |row| row.get(0))?;
        Ok(u64::try_from(count).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests;
