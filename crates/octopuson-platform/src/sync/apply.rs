//! Taking changes that arrive: deciding which side wins, writing records the way the program
//! does, and keeping changes that wait for a record they refer to.

use std::collections::HashMap;

use duckdb::params;
use duckdb::types::Value as SqlValue;
use serde_json::{Map, Value};

use super::kinds::{Children, GSC_TABLES, Key, Kind, Removal, Shape, kind_named, uid_table};
use super::{Entry, SyncAck, SyncApplied, SyncChange, SyncSide, sql, to_sql};
use crate::reports::{MAX_RECIPIENTS, valid_email};
use crate::{PlatformStore, PlatformStoreError};

/// Origin of the hub's versions on a copy.
const HUB: &str = "hub";

/// What writing a record did.
enum Written {
    /// Written; `modified` when what is stored differs from what arrived.
    Done { modified: bool },
    /// It refers to a record not here (`kind/uid`).
    Missing(String),
    /// It cannot be used.
    Invalid,
}

/// A JSON scalar's text, for comparing times.
fn text(value: &Value) -> Option<&str> {
    value.as_str()
}

/// Whether a value is a plain value a column can hold.
const fn scalar(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

/// Whether a JSON column's value holds JSON.
fn holds_json(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => serde_json::from_str::<Value>(text).is_ok(),
        _ => false,
    }
}

/// Report recipients the hub will e-mail: valid addresses only, at most the limit.
fn clean_recipients(value: &Value) -> Option<Value> {
    let addresses: Vec<String> = serde_json::from_str(value.as_str()?).ok()?;
    let clean: Vec<String> = addresses
        .into_iter()
        .map(|address| address.trim().to_lowercase())
        .filter(|address| valid_email(address))
        .take(MAX_RECIPIENTS)
        .collect();
    serde_json::to_string(&clean).ok().map(Value::String)
}

impl PlatformStore {
    /// Takes changes that arrived: from a copy on the hub, from the hub on a copy. `last` says
    /// no more changes follow in this sync, so a change still waiting for a record it refers to
    /// is applied without what is missing (that record was deleted).
    ///
    /// # Errors
    /// Database errors; changes taken before stay taken.
    pub fn sync_apply(
        &self,
        side: SyncSide<'_>,
        changes: &[SyncChange],
        now_ms: i64,
        last: bool,
    ) -> Result<SyncApplied, PlatformStoreError> {
        self.fill_uids()?;
        let mut out = SyncApplied::default();
        for change in changes {
            let transaction = self.connection.unchecked_transaction()?;
            let applied = self.apply_one(side, change, now_ms, false)?;
            transaction.commit()?;
            out.absorb(applied);
        }
        out.absorb(self.retry_parked(side, now_ms, last)?);
        Ok(out)
    }

    fn apply_one(
        &self,
        side: SyncSide<'_>,
        change: &SyncChange,
        now_ms: i64,
        last: bool,
    ) -> Result<SyncApplied, PlatformStoreError> {
        let mut out = SyncApplied::default();
        let Some(kind) = kind_named(&change.kind) else {
            // A kind a newer program knows.
            return Ok(out);
        };
        if matches!(kind.removal, Removal::Local) && change.deleted {
            return Ok(out);
        }
        let entry = self.entry(kind.name, &change.key)?;
        let current = self.current(kind, &change.key)?;
        let seen = entry.as_ref().and_then(|entry| entry.hash.clone());
        // An edit here that no scan has seen yet counts as a change made now.
        let unscanned = current.as_ref().map(|(hash, _)| hash.clone()) != seen;
        let wins = match side {
            SyncSide::Hub { .. } => {
                let fast =
                    !unscanned && entry.as_ref().is_none_or(|entry| entry.seq == change.base);
                fast || self.decide(
                    kind,
                    change,
                    entry.as_ref(),
                    current.is_some(),
                    unscanned,
                    now_ms,
                )?
            }
            // The copy's own change that is not sent yet goes to the hub, which decides.
            SyncSide::Copy { pushed } => {
                !unscanned
                    && !entry
                        .as_ref()
                        .is_some_and(|entry| entry.origin.is_empty() && entry.seq > pushed)
            }
        };
        if !wins {
            if let SyncSide::Hub { .. } = side {
                if let Some(data) = change.data.as_ref().and_then(Value::as_object)
                    && !change.deleted
                {
                    self.keep_later(kind, &change.key, data, now_ms)?;
                }
                out.rejected
                    .push((kind.name.to_owned(), change.key.clone()));
            }
            return Ok(out);
        }
        if change.deleted {
            self.remove(kind, &change.key, &mut out)?;
            let seq = self.note(kind, change, None, side, false, now_ms)?;
            self.unpark(kind.name, &change.key)?;
            out.applied += 1;
            if let SyncSide::Hub { .. } = side {
                out.accepted.push(SyncAck {
                    kind: kind.name.to_owned(),
                    key: change.key.clone(),
                    seq,
                });
            }
            return Ok(out);
        }
        let Some(data) = change.data.as_ref().and_then(Value::as_object) else {
            out.dropped += 1;
            return Ok(out);
        };
        let written = match kind.shape {
            Shape::Gsc => self.write_gsc(&change.key, data)?,
            Shape::Run(children) => {
                self.write_run(kind, children, side, &change.key, data, last, &mut out)?
            }
            Shape::Row | Shape::Cache => self.write_row(kind, side, &change.key, data, &mut out)?,
        };
        match written {
            Written::Missing(waiting) if !last => {
                self.park(side, change, &waiting)?;
                out.parked += 1;
            }
            Written::Missing(_) | Written::Invalid => {
                self.unpark(kind.name, &change.key)?;
                out.dropped += 1;
                if let SyncSide::Hub { .. } = side {
                    out.rejected
                        .push((kind.name.to_owned(), change.key.clone()));
                }
            }
            Written::Done { modified } => {
                let now = self.current(kind, &change.key)?;
                if now.is_some() {
                    let seq = self.note(kind, change, now, side, modified, now_ms)?;
                    if let (SyncSide::Hub { .. }, false) = (side, modified) {
                        out.accepted.push(SyncAck {
                            kind: kind.name.to_owned(),
                            key: change.key.clone(),
                            seq,
                        });
                    }
                }
                self.unpark(kind.name, &change.key)?;
                out.applied += 1;
            }
        }
        Ok(out)
    }

    /// Whether a copy's change wins on the hub over the hub's own change of the same record.
    fn decide(
        &self,
        kind: &Kind,
        change: &SyncChange,
        entry: Option<&Entry>,
        here: bool,
        unscanned: bool,
        now_ms: i64,
    ) -> Result<bool, PlatformStoreError> {
        if kind.random() {
            // A record made apart that one side deleted stays deleted.
            if change.deleted {
                return Ok(true);
            }
            if !here && entry.is_some_and(|entry| entry.deleted) {
                return Ok(false);
            }
        }
        if matches!(kind.shape, Shape::Cache) {
            let incoming = change
                .data
                .as_ref()
                .and_then(|data| data.get("fetched_at"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let mut statement = self
                .connection
                .prepare_cached("SELECT fetched_at FROM cache WHERE \"key\" = ?")?;
            let mut rows =
                statement.query_map(params![change.key], |row| row.get::<_, String>(0))?;
            let local = rows.next().transpose()?;
            return Ok(local.is_none_or(|local| incoming > local));
        }
        let local_at = if unscanned {
            now_ms
        } else {
            entry.map_or(0, |entry| entry.changed_at)
        };
        Ok(change.at > local_at)
    }

    /// Notes what a record is now after taking a change; returns its change number here.
    fn note(
        &self,
        kind: &Kind,
        change: &SyncChange,
        now: Option<(String, Option<String>)>,
        side: SyncSide<'_>,
        modified: bool,
        now_ms: i64,
    ) -> Result<i64, PlatformStoreError> {
        let (hash, parent) = now.map_or((None, None), |(hash, parent)| (Some(hash), parent));
        let deleted = hash.is_none();
        let previous = self.entry(kind.name, &change.key)?;
        let (seq, origin, changed_at, remote_seq) = match side {
            SyncSide::Hub { replica } => {
                let seq = self.reserve(1)? + 1;
                if modified {
                    (seq, String::new(), now_ms.max(change.at), 0)
                } else {
                    (seq, replica.to_owned(), change.at, 0)
                }
            }
            SyncSide::Copy { .. } => {
                if modified {
                    // What is kept here differs from the hub's: it goes back.
                    (self.reserve(1)? + 1, String::new(), now_ms, change.seq)
                } else {
                    let seq = previous.as_ref().map_or(0, |entry| entry.seq);
                    (seq, HUB.to_owned(), change.at, change.seq)
                }
            }
        };
        self.connection.execute(
            "INSERT INTO sync_rows (kind, key, parent, hash, seq, changed_at, origin, remote_seq, deleted)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (kind, key) DO UPDATE SET parent = coalesce(excluded.parent, sync_rows.parent),
                 hash = excluded.hash, seq = excluded.seq, changed_at = excluded.changed_at,
                 origin = excluded.origin, remote_seq = excluded.remote_seq, deleted = excluded.deleted",
            params![
                kind.name, change.key, parent, hash, seq, changed_at, origin, remote_seq, deleted
            ],
        )?;
        Ok(seq)
    }

    /// On the hub, when a copy's change lost: keeps the later of the times the program writes
    /// itself, so work done on the copy is not done again here.
    fn keep_later(
        &self,
        kind: &Kind,
        key: &str,
        data: &Map<String, Value>,
        now_ms: i64,
    ) -> Result<(), PlatformStoreError> {
        if kind.later.is_empty() || !matches!(kind.shape, Shape::Row) {
            return Ok(());
        }
        let Some(locator) = self.locator(kind, key)? else {
            return Ok(());
        };
        let (condition, _) = sql::locate(kind);
        let mut changed = false;
        for name in kind.later {
            let Some(incoming) = data.get(*name).and_then(text) else {
                continue;
            };
            let query = format!(
                "UPDATE {} SET \"{name}\" = ? WHERE {condition} AND (\"{name}\" IS NULL OR \"{name}\" < ?)",
                kind.table
            );
            let mut parameters: Vec<SqlValue> = vec![SqlValue::Text(incoming.to_owned())];
            parameters.extend(locator.clone());
            parameters.push(SqlValue::Text(incoming.to_owned()));
            changed |= self
                .connection
                .execute(&query, duckdb::params_from_iter(parameters))?
                > 0;
        }
        if changed && let Some((hash, parent)) = self.current(kind, key)? {
            let seq = self.reserve(1)? + 1;
            self.connection.execute(
                "UPDATE sync_rows SET hash = ?, parent = coalesce(?, parent), seq = ?, changed_at = ?, origin = ''
                 WHERE kind = ? AND key = ?",
                params![hash, parent, seq, now_ms, kind.name, key],
            )?;
        }
        Ok(())
    }

    /// The parameter finding a record's row by [`sql::locate`]; `None` when its parent is not
    /// here (a project's settings without the project).
    fn locator(&self, kind: &Kind, key: &str) -> Result<Option<Vec<SqlValue>>, PlatformStoreError> {
        Ok(match sql::locate(kind).1 {
            Some(true) => Some(vec![SqlValue::Text(key.to_owned())]),
            Some(false) => {
                let table = kind.parent.map_or("projects", |parent| parent.table);
                self.id_by_uid(table, key)?
                    .map(|id| vec![SqlValue::BigInt(id)])
            }
            None => Some(Vec::new()),
        })
    }

    /// Carries out a deletion that arrived, as the program itself deletes.
    #[allow(clippy::too_many_lines)] // one arm per kind of removal
    fn remove(
        &self,
        kind: &Kind,
        key: &str,
        out: &mut SyncApplied,
    ) -> Result<(), PlatformStoreError> {
        let by_uid = |table: &str| self.id_by_uid(table, key);
        match kind.removal {
            Removal::Local => {}
            Removal::Project => {
                if let Some(id) = by_uid("projects")? {
                    out.removed_crawls.extend(self.audit_crawls(id)?);
                    self.delete_project(id)?;
                }
            }
            Removal::Tracker => {
                if let Some(id) = by_uid("rank_trackers")? {
                    self.connection
                        .execute("DELETE FROM rank_checks WHERE tracker_id = ?", params![id])?;
                    self.connection
                        .execute("DELETE FROM rank_trackers WHERE id = ?", params![id])?;
                }
            }
            Removal::Keyword => {
                if let Some(id) = by_uid("tracked_keywords")? {
                    self.connection
                        .execute("DELETE FROM rank_checks WHERE keyword_id = ?", params![id])?;
                    self.connection
                        .execute("DELETE FROM tracked_keywords WHERE id = ?", params![id])?;
                }
            }
            Removal::Prompt => {
                if let Some(id) = by_uid("ai_prompts")? {
                    self.connection
                        .execute("DELETE FROM ai_answers WHERE prompt_id = ?", params![id])?;
                    self.connection
                        .execute("DELETE FROM ai_prompts WHERE id = ?", params![id])?;
                }
            }
            Removal::List => {
                if let Some(id) = by_uid("keyword_lists")? {
                    self.connection
                        .execute("DELETE FROM list_keywords WHERE list_id = ?", params![id])?;
                    self.connection
                        .execute("DELETE FROM keyword_lists WHERE id = ?", params![id])?;
                }
            }
            Removal::Report => {
                if let Some(id) = by_uid("reports")? {
                    let token: Option<String> = self.connection.query_row(
                        "SELECT share_token FROM reports WHERE id = ?",
                        params![id],
                        |row| row.get(0),
                    )?;
                    out.removed_shares.extend(token);
                    self.connection.execute(
                        "DELETE FROM report_deliveries WHERE report_id = ?",
                        params![id],
                    )?;
                    self.connection
                        .execute("DELETE FROM reports WHERE id = ?", params![id])?;
                }
            }
            Removal::Brief => {
                if let Some(id) = by_uid("content_briefs")? {
                    self.delete_content_brief(id)?;
                }
            }
            Removal::Audit => {
                if let Some(id) = by_uid("audits")? {
                    let crawl: Option<String> = self.connection.query_row(
                        "SELECT crawl_id FROM audits WHERE id = ?",
                        params![id],
                        |row| row.get(0),
                    )?;
                    out.removed_crawls.extend(crawl);
                    self.connection
                        .execute("DELETE FROM audits WHERE id = ?", params![id])?;
                }
            }
            Removal::Gsc => {
                if let Some(project) = by_uid("projects")? {
                    for table in GSC_TABLES {
                        self.connection.execute(
                            &format!("DELETE FROM {} WHERE project_id = ?", table.table),
                            params![project],
                        )?;
                    }
                }
            }
            Removal::Row => {
                if let Shape::Run(children) = kind.shape
                    && let Some(id) = by_uid(kind.table)?
                {
                    self.connection.execute(
                        &format!("DELETE FROM {} WHERE run_id = ?", children.table),
                        params![id],
                    )?;
                }
                if let Some(locator) = self.locator(kind, key)? {
                    let (condition, _) = sql::locate(kind);
                    self.connection.execute(
                        &format!("DELETE FROM {} WHERE {condition}", kind.table),
                        duckdb::params_from_iter(locator),
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Writes one row: updates it when it is here, inserts it otherwise.
    #[allow(clippy::too_many_lines)] // one row's checks and writes, in order
    fn write_row(
        &self,
        kind: &Kind,
        side: SyncSide<'_>,
        key: &str,
        data: &Map<String, Value>,
        out: &mut SyncApplied,
    ) -> Result<Written, PlatformStoreError> {
        let mut parent_id: Option<i64> = None;
        if let Some(parent) = kind.parent {
            match data.get(parent.field) {
                Some(Value::String(uid)) => match self.id_by_uid(parent.table, uid)? {
                    Some(id) => parent_id = Some(id),
                    None => return Ok(Written::Missing(format!("{}/{uid}", parent.kind))),
                },
                Some(Value::Null) | None if kind.optional_parent => {}
                _ => return Ok(Written::Invalid),
            }
            if matches!(kind.key, Key::Parent)
                && data.get(parent.field).and_then(Value::as_str) != Some(key)
            {
                return Ok(Written::Invalid);
            }
        }
        let mut values: Vec<Value> = kind
            .columns
            .iter()
            .map(|name| data.get(*name).cloned().unwrap_or(Value::Null))
            .collect();
        if !values.iter().all(scalar) {
            return Ok(Written::Invalid);
        }
        let index = |name: &str| kind.columns.iter().position(|column| *column == name);
        for name in kind.json {
            if let Some(at) = index(name)
                && !holds_json(&values[at])
            {
                return Ok(Written::Invalid);
            }
        }
        let (condition, takes) = sql::locate(kind);
        let locator: Vec<SqlValue> = match takes {
            Some(true) => vec![SqlValue::Text(key.to_owned())],
            Some(false) => parent_id.map(SqlValue::BigInt).into_iter().collect(),
            None => Vec::new(),
        };
        let report = kind.name == "report";
        let mut watched: Vec<&str> = kind.later.to_vec();
        if report {
            watched.push("share_token");
        }
        let existing: Option<Vec<Option<String>>> = {
            let selected: Vec<String> = if watched.is_empty() {
                vec!["1".to_owned()]
            } else {
                watched.iter().map(|name| format!("\"{name}\"")).collect()
            };
            let mut statement = self.connection.prepare(&format!(
                "SELECT {} FROM {} WHERE {condition}",
                selected.join(", "),
                kind.table
            ))?;
            let width = watched.len();
            let mut rows =
                statement.query_map(duckdb::params_from_iter(locator.clone()), |row| {
                    (0..width)
                        .map(|at| row.get::<_, Option<String>>(at))
                        .collect::<Result<Vec<_>, _>>()
                })?;
            rows.next().transpose()?
        };
        let mut modified = false;
        if let Some(existing) = &existing {
            for (name, here) in kind.later.iter().zip(existing) {
                let Some(at) = index(name) else { continue };
                if here.as_deref() > text(&values[at]) {
                    values[at] = here.clone().map_or(Value::Null, Value::String);
                    modified = true;
                }
            }
        }
        if let (true, SyncSide::Hub { .. }) = (report, side) {
            // Links are made and counted on the web: a copy may only stop sharing.
            let here = existing
                .as_ref()
                .and_then(|existing| existing.last().cloned().flatten());
            if let Some(at) = index("share_token") {
                let incoming = values[at].as_str().map(str::to_owned);
                match (&here, &incoming) {
                    (Some(token), None) => out.removed_shares.push(token.clone()),
                    (here, incoming) if here != incoming => {
                        values[at] = here.clone().map_or(Value::Null, Value::String);
                        modified = true;
                    }
                    _ => {}
                }
            }
            if let Some(at) = index("recipients") {
                let clean =
                    clean_recipients(&values[at]).unwrap_or_else(|| Value::String("[]".to_owned()));
                if clean != values[at] {
                    values[at] = clean;
                    modified = true;
                }
            }
        }
        let quoted: Vec<String> = kind
            .columns
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect();
        let mut parameters: Vec<SqlValue> = values.iter().map(to_sql).collect();
        if existing.is_some() {
            let assignments: Vec<String> =
                quoted.iter().map(|name| format!("{name} = ?")).collect();
            parameters.extend(locator);
            self.connection.execute(
                &format!(
                    "UPDATE {} SET {} WHERE {condition}",
                    kind.table,
                    assignments.join(", ")
                ),
                duckdb::params_from_iter(parameters),
            )?;
            return Ok(Written::Done { modified });
        }
        let mut columns = quoted;
        match kind.key {
            Key::Uid => {
                columns.push("uid".to_owned());
                parameters.push(SqlValue::Text(key.to_owned()));
            }
            Key::Column(name) => {
                columns.push(format!("\"{name}\""));
                parameters.push(SqlValue::Text(key.to_owned()));
            }
            Key::Fixed(_) => {
                columns.push("id".to_owned());
                parameters.push(SqlValue::BigInt(1));
            }
            Key::Parent => {}
        }
        if let Some(parent) = kind.parent {
            columns.push(format!("\"{}\"", parent.column));
            parameters.push(parent_id.map_or(SqlValue::Null, SqlValue::BigInt));
            for name in kind.inherit {
                let value: Option<i64> = match parent_id {
                    Some(id) => self.connection.query_row(
                        &format!("SELECT \"{name}\" FROM {} WHERE id = ?", parent.table),
                        params![id],
                        |row| row.get(0),
                    )?,
                    None => None,
                };
                columns.push(format!("\"{name}\""));
                parameters.push(value.map_or(SqlValue::Null, SqlValue::BigInt));
            }
        }
        let marks = vec!["?"; columns.len()].join(", ");
        self.connection.execute(
            &format!(
                "INSERT INTO {} ({}) VALUES ({marks})",
                kind.table,
                columns.join(", ")
            ),
            duckdb::params_from_iter(parameters),
        )?;
        Ok(Written::Done { modified })
    }

    /// Writes a finished run with its rows, replacing the rows it had here.
    #[allow(clippy::too_many_arguments)]
    fn write_run(
        &self,
        kind: &Kind,
        children: &Children,
        side: SyncSide<'_>,
        key: &str,
        data: &Map<String, Value>,
        last: bool,
        out: &mut SyncApplied,
    ) -> Result<Written, PlatformStoreError> {
        let Some(project) = data.get("project").and_then(Value::as_str) else {
            return Ok(Written::Invalid);
        };
        let Some(project_id) = self.id_by_uid("projects", project)? else {
            return Ok(Written::Missing(format!("project/{project}")));
        };
        let mut maps: Vec<HashMap<String, i64>> = Vec::new();
        for (_, _, table) in children.refs {
            let mut statement = self.connection.prepare_cached(&format!(
                "SELECT uid, id FROM {table} WHERE project_id = ? AND uid IS NOT NULL"
            ))?;
            let rows = statement.query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            maps.push(rows.collect::<Result<_, _>>()?);
        }
        let width = children.refs.len() + children.columns.len();
        let rows = match data.get("rows") {
            Some(Value::Array(rows)) => rows.as_slice(),
            None => &[],
            Some(_) => return Ok(Written::Invalid),
        };
        let mut ready: Vec<Vec<SqlValue>> = Vec::with_capacity(rows.len());
        'rows: for row in rows {
            let Some(values) = row.as_array().filter(|values| values.len() == width) else {
                return Ok(Written::Invalid);
            };
            if !values.iter().all(scalar) {
                return Ok(Written::Invalid);
            }
            let mut out_row: Vec<SqlValue> = Vec::with_capacity(width + 2);
            for (at, (_, ref_kind, _)) in children.refs.iter().enumerate() {
                let Some(uid) = values[at].as_str() else {
                    return Ok(Written::Invalid);
                };
                match maps[at].get(uid) {
                    Some(id) => out_row.push(SqlValue::BigInt(*id)),
                    // Deleted meanwhile: its checks went with it.
                    None if last => continue 'rows,
                    None => return Ok(Written::Missing(format!("{ref_kind}/{uid}"))),
                }
            }
            for (name, value) in children.columns.iter().zip(&values[children.refs.len()..]) {
                if children.json.contains(name) && !holds_json(value) {
                    return Ok(Written::Invalid);
                }
                out_row.push(to_sql(value));
            }
            ready.push(out_row);
        }
        let written = self.write_row(kind, side, key, data, out)?;
        if !matches!(written, Written::Done { .. }) {
            return Ok(written);
        }
        let Some(run_id) = self.id_by_uid(kind.table, key)? else {
            return Ok(Written::Invalid);
        };
        self.connection.execute(
            &format!("DELETE FROM {} WHERE run_id = ?", children.table),
            params![run_id],
        )?;
        let mut columns: Vec<String> = vec!["run_id".to_owned(), "project_id".to_owned()];
        columns.extend(
            children
                .refs
                .iter()
                .map(|(column, _, _)| format!("\"{column}\"")),
        );
        columns.extend(children.columns.iter().map(|name| format!("\"{name}\"")));
        let marks = vec!["?"; columns.len()].join(", ");
        let mut statement = self.connection.prepare(&format!(
            "INSERT INTO {} ({}) VALUES ({marks})",
            children.table,
            columns.join(", ")
        ))?;
        for row in ready {
            let mut parameters = vec![SqlValue::BigInt(run_id), SqlValue::BigInt(project_id)];
            parameters.extend(row);
            statement.execute(duckdb::params_from_iter(parameters))?;
        }
        Ok(written)
    }

    /// Replaces a project's Search Console and Analytics numbers with what arrived.
    fn write_gsc(
        &self,
        key: &str,
        data: &Map<String, Value>,
    ) -> Result<Written, PlatformStoreError> {
        let Some(project_id) = self.id_by_uid("projects", key)? else {
            return Ok(Written::Missing(format!("project/{key}")));
        };
        let mut tables: Vec<Vec<Vec<SqlValue>>> = Vec::new();
        for table in GSC_TABLES {
            let rows = match data.get(table.field) {
                Some(Value::Array(rows)) => rows.as_slice(),
                None => &[],
                Some(_) => return Ok(Written::Invalid),
            };
            let mut ready = Vec::with_capacity(rows.len());
            for row in rows {
                let Some(values) = row
                    .as_array()
                    .filter(|values| values.len() == table.columns.len())
                else {
                    return Ok(Written::Invalid);
                };
                if !values.iter().all(scalar) {
                    return Ok(Written::Invalid);
                }
                let mut out_row = vec![SqlValue::BigInt(project_id)];
                out_row.extend(values.iter().map(to_sql));
                ready.push(out_row);
            }
            tables.push(ready);
        }
        for (table, rows) in GSC_TABLES.iter().zip(tables) {
            self.connection.execute(
                &format!("DELETE FROM {} WHERE project_id = ?", table.table),
                params![project_id],
            )?;
            let mut appender = self.connection.appender(table.table)?;
            for row in rows {
                appender.append_row(duckdb::appender_params_from_iter(row))?;
            }
            appender.flush()?;
        }
        Ok(Written::Done { modified: false })
    }

    /// Keeps a change until the record it refers to arrives.
    fn park(
        &self,
        side: SyncSide<'_>,
        change: &SyncChange,
        waiting: &str,
    ) -> Result<(), PlatformStoreError> {
        let origin = match side {
            SyncSide::Hub { replica } => replica,
            SyncSide::Copy { .. } => HUB,
        };
        let text = serde_json::to_string(change)
            .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))?;
        self.connection.execute(
            "INSERT INTO sync_pending (kind, key, origin, change, waiting) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (kind, key) DO UPDATE SET origin = excluded.origin, change = excluded.change,
                 waiting = excluded.waiting",
            params![change.kind, change.key, origin, text, waiting],
        )?;
        Ok(())
    }

    fn unpark(&self, kind: &str, key: &str) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM sync_pending WHERE kind = ? AND key = ?",
            params![kind, key],
        )?;
        Ok(())
    }

    /// Whether the record a change waits for (`kind/uid`) is here.
    fn arrived(&self, waiting: &str) -> Result<bool, PlatformStoreError> {
        let Some((kind, uid)) = waiting.split_once('/') else {
            return Ok(true);
        };
        let Some(table) = uid_table(kind) else {
            return Ok(true);
        };
        Ok(self.id_by_uid(table, uid)?.is_some())
    }

    /// Applies the waiting changes whose record arrived; with `last`, those of this sync's
    /// sender that still wait too, without what they miss.
    fn retry_parked(
        &self,
        side: SyncSide<'_>,
        now_ms: i64,
        last: bool,
    ) -> Result<SyncApplied, PlatformStoreError> {
        let mut out = SyncApplied::default();
        loop {
            let waiting: Vec<(String, String, String)> = {
                let mut statement = self
                    .connection
                    .prepare("SELECT origin, change, waiting FROM sync_pending")?;
                let rows =
                    statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
                rows.collect::<Result<_, _>>()?
            };
            let mut progress = false;
            for (origin, text, waits_for) in waiting {
                let here = match side {
                    SyncSide::Hub { .. } => SyncSide::Hub { replica: &origin },
                    copy @ SyncSide::Copy { .. } => copy,
                };
                let finish = last
                    && match side {
                        SyncSide::Hub { replica } => origin == replica,
                        SyncSide::Copy { .. } => true,
                    };
                if !finish && !self.arrived(&waits_for)? {
                    continue;
                }
                let transaction = self.connection.unchecked_transaction()?;
                let applied = match serde_json::from_str::<SyncChange>(&text) {
                    Ok(change) => self.apply_one(here, &change, now_ms, finish)?,
                    Err(_) => SyncApplied::default(),
                };
                if applied.parked == 0 {
                    if let Ok(change) = serde_json::from_str::<SyncChange>(&text) {
                        self.unpark(&change.kind, &change.key)?;
                    }
                    progress = true;
                }
                transaction.commit()?;
                out.absorb(applied);
            }
            if !progress {
                break;
            }
        }
        Ok(out)
    }
}
