//! Result pages of keywords, for clustering and site mapping.

use std::collections::HashMap;

use duckdb::params;
use serde::{Deserialize, Serialize};

use crate::{KwStore, KwStoreError, json, parse, stage_ids};

/// The top results of one keyword on one engine and device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerpRecord {
    /// The keyword.
    pub keyword_id: i64,
    /// Engine id.
    pub engine: String,
    /// `desktop` or `mobile`.
    pub device: String,
    /// Provider that returned it.
    pub provider: String,
    /// RFC 3339.
    pub fetched_at: String,
    /// Organic result addresses in rank order.
    pub urls: Vec<String>,
    /// Whether the addresses are sites only (Google's hidden links in the browser mode).
    pub site_only: bool,
    /// Result page features (`people-also-ask`, `local-pack` …).
    pub features: Vec<String>,
    /// People-also-ask questions.
    pub questions: Vec<String>,
    /// Related searches.
    pub related: Vec<String>,
    /// Stored raw response, relative to the data directory.
    pub raw_path: Option<String>,
}

impl KwStore {
    /// Stores result pages, replacing earlier ones of the same keyword, engine and device.
    ///
    /// # Errors
    /// Database errors.
    pub fn put_serps(&mut self, records: &[SerpRecord]) -> Result<(), KwStoreError> {
        let mut last = HashMap::new();
        for (index, record) in records.iter().enumerate() {
            last.insert(
                (
                    record.keyword_id,
                    record.engine.as_str(),
                    record.device.as_str(),
                ),
                index,
            );
        }
        let mut groups: HashMap<(&str, &str), Vec<i64>> = HashMap::new();
        for (keyword_id, engine, device) in last.keys() {
            groups
                .entry((engine, device))
                .or_default()
                .push(*keyword_id);
        }
        let transaction = self.connection.transaction()?;
        for ((engine, device), ids) in &groups {
            stage_ids(&transaction, ids)?;
            transaction.execute(
                "DELETE FROM serps WHERE engine = ? AND device = ?
                     AND keyword_id IN (SELECT id FROM incoming_ids)",
                params![engine, device],
            )?;
        }
        transaction.execute("DELETE FROM incoming_ids", [])?;
        {
            let mut appender = transaction.appender("serps")?;
            for (index, record) in records.iter().enumerate() {
                let key = (
                    record.keyword_id,
                    record.engine.as_str(),
                    record.device.as_str(),
                );
                if last.get(&key) != Some(&index) {
                    continue;
                }
                appender.append_row(params![
                    record.keyword_id,
                    record.engine,
                    record.device,
                    record.provider,
                    record.fetched_at,
                    json(&record.urls)?,
                    record.site_only,
                    json(&record.features)?,
                    json(&record.questions)?,
                    json(&record.related)?,
                    record.raw_path
                ])?;
            }
            appender.flush()?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The stored result page of a keyword.
    ///
    /// # Errors
    /// Database errors, invalid stored lists.
    pub fn serp(
        &self,
        keyword_id: i64,
        engine: &str,
        device: &str,
    ) -> Result<Option<SerpRecord>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT keyword_id, engine, device, provider, fetched_at, urls, site_only, features,
                    questions, related, raw_path
             FROM serps WHERE keyword_id = ? AND engine = ? AND device = ?",
        )?;
        let mut rows = statement.query_map(params![keyword_id, engine, device], read)?;
        rows.next().map(|row| finish(row?)).transpose()
    }

    /// When each keyword of a project last got a result page of an engine and device.
    ///
    /// # Errors
    /// Database errors.
    pub fn serp_ages(
        &self,
        project_id: i64,
        engine: &str,
        device: &str,
    ) -> Result<HashMap<i64, String>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT s.keyword_id, s.fetched_at FROM serps s
             JOIN keywords k ON k.id = s.keyword_id AND k.project_id = ?
             WHERE s.engine = ? AND s.device = ?",
        )?;
        let rows = statement.query_map(params![project_id, engine, device], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Result pages of keywords of a project.
    ///
    /// # Errors
    /// Database errors, invalid stored lists.
    pub fn serps(
        &mut self,
        project_id: i64,
        ids: &[i64],
        engine: &str,
        device: &str,
    ) -> Result<Vec<SerpRecord>, KwStoreError> {
        let transaction = self.connection.transaction()?;
        stage_ids(&transaction, ids)?;
        let records = {
            let mut statement = transaction.prepare(
                "SELECT s.keyword_id, s.engine, s.device, s.provider, s.fetched_at, s.urls,
                        s.site_only, s.features, s.questions, s.related, s.raw_path
                 FROM serps s JOIN keywords k ON k.id = s.keyword_id AND k.project_id = ?
                 WHERE s.engine = ? AND s.device = ? AND s.keyword_id IN (SELECT id FROM incoming_ids)
                 ORDER BY s.keyword_id",
            )?;
            let rows = statement.query_map(params![project_id, engine, device], read)?;
            rows.map(|row| finish(row?))
                .collect::<Result<Vec<_>, KwStoreError>>()?
        };
        transaction.execute("DELETE FROM incoming_ids", [])?;
        transaction.commit()?;
        Ok(records)
    }
}

type Raw = (SerpRecord, String, String, String, String);

fn read(row: &duckdb::Row<'_>) -> duckdb::Result<Raw> {
    Ok((
        SerpRecord {
            keyword_id: row.get(0)?,
            engine: row.get(1)?,
            device: row.get(2)?,
            provider: row.get(3)?,
            fetched_at: row.get(4)?,
            urls: Vec::new(),
            site_only: row.get(6)?,
            features: Vec::new(),
            questions: Vec::new(),
            related: Vec::new(),
            raw_path: row.get(10)?,
        },
        row.get(5)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

fn finish(
    (mut record, urls, features, questions, related): Raw,
) -> Result<SerpRecord, KwStoreError> {
    record.urls = parse(&urls)?;
    record.features = parse(&features)?;
    record.questions = parse(&questions)?;
    record.related = parse(&related)?;
    Ok(record)
}
