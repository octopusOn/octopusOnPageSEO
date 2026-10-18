//! Keyword analyses.

use duckdb::{OptionalExt, params};
use seonexus_serp::{Engine, keyword_key};
use serde::{Deserialize, Serialize};

use crate::{SerpStore, SerpStoreError};

/// Scores of one page of an analysis, as stored for queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageScoreRow {
    /// Organic position (0 for the user's own page when it does not rank).
    pub position: u16,
    /// URL.
    pub url: String,
    /// Registrable domain.
    pub domain: String,
    /// Content score.
    pub content: Option<f64>,
    /// Technical score.
    pub technical: Option<f64>,
    /// Site score.
    pub site: Option<f64>,
    /// Blended strength.
    pub strength: Option<f64>,
    /// Words of main content.
    pub words: u32,
}

/// A stored analysis. `result`, `inputs` and `domains` are JSON documents of the scoring
/// crate and the application; the store does not interpret them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisRecord {
    /// Id (ignored when saving).
    pub id: i64,
    /// Snapshot analysed.
    pub snapshot_id: i64,
    /// Keyword.
    pub keyword: String,
    /// Engine.
    pub engine: Engine,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Device.
    pub device: String,
    /// When it was made, RFC 3339.
    pub created_at: String,
    /// The user's domain, if one was given.
    pub target_domain: Option<String>,
    /// Keyword difficulty.
    pub difficulty: f64,
    /// Difficulty band.
    pub band: String,
    /// Primary intent.
    pub intent: String,
    /// Opportunity for the target, if any.
    pub opportunity: Option<f64>,
    /// Monthly searches, if known.
    pub volume: Option<u64>,
    /// Model versions, JSON.
    pub models: String,
    /// The analysis, JSON.
    pub result: String,
    /// The scoring inputs, JSON.
    pub inputs: String,
    /// Facts about the domains, JSON.
    pub domains: Option<String>,
    /// Crawls the pages came from, JSON.
    pub crawl_ids: Option<String>,
    /// Page scores.
    pub pages: Vec<PageScoreRow>,
}

/// A stored analysis without its documents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSummary {
    /// Id.
    pub id: i64,
    /// Snapshot analysed.
    pub snapshot_id: i64,
    /// Keyword.
    pub keyword: String,
    /// Engine.
    pub engine: Engine,
    /// Country code.
    pub country: String,
    /// When it was made.
    pub created_at: String,
    /// The user's domain.
    pub target_domain: Option<String>,
    /// Keyword difficulty.
    pub difficulty: f64,
    /// Band.
    pub band: String,
    /// Primary intent.
    pub intent: String,
    /// Opportunity.
    pub opportunity: Option<f64>,
    /// Monthly searches.
    pub volume: Option<u64>,
}

impl SerpStore {
    /// Saves an analysis and its page scores. Returns its id.
    ///
    /// # Errors
    /// Database errors.
    pub fn save_analysis(&mut self, record: &AnalysisRecord) -> Result<i64, SerpStoreError> {
        let transaction = self.connection.transaction()?;
        let id: i64 = transaction.query_row(
            "INSERT INTO analyses (snapshot_id, keyword, keyword_key, engine, country, language,
                 device, created_at, target_domain, difficulty, band, intent, opportunity, volume,
                 models, result, inputs, domains, crawl_ids)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                record.snapshot_id,
                record.keyword,
                keyword_key(&record.keyword, &record.language),
                record.engine.as_str(),
                record.country,
                record.language,
                record.device,
                record.created_at,
                record.target_domain,
                record.difficulty,
                record.band,
                record.intent,
                record.opportunity,
                record.volume,
                record.models,
                record.result,
                record.inputs,
                record.domains,
                record.crawl_ids,
            ],
            |row| row.get(0),
        )?;
        insert_pages(&transaction, id, &record.pages)?;
        transaction.commit()?;
        Ok(id)
    }

    /// Replaces the scores of an analysis (after scoring it with a new model).
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn update_analysis(&mut self, record: &AnalysisRecord) -> Result<(), SerpStoreError> {
        let transaction = self.connection.transaction()?;
        let updated = transaction.execute(
            "UPDATE analyses SET difficulty = ?, band = ?, intent = ?, opportunity = ?, models = ?,
                 result = ? WHERE id = ?",
            params![
                record.difficulty,
                record.band,
                record.intent,
                record.opportunity,
                record.models,
                record.result,
                record.id
            ],
        )?;
        if updated == 0 {
            return Err(SerpStoreError::NotFound(format!("analysis {}", record.id)));
        }
        transaction.execute(
            "DELETE FROM page_scores WHERE analysis_id = ?",
            params![record.id],
        )?;
        insert_pages(&transaction, record.id, &record.pages)?;
        transaction.commit()?;
        Ok(())
    }

    /// A stored analysis.
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn analysis(&self, id: i64) -> Result<AnalysisRecord, SerpStoreError> {
        let mut record = self
            .connection
            .query_row(
                "SELECT id, snapshot_id, keyword, engine, country, language, device, created_at,
                        target_domain, difficulty, band, intent, opportunity, volume, models, result,
                        inputs, domains, crawl_ids
                 FROM analyses WHERE id = ?",
                params![id],
                |row| {
                    Ok(AnalysisRecord {
                        id: row.get(0)?,
                        snapshot_id: row.get(1)?,
                        keyword: row.get(2)?,
                        engine: row
                            .get::<_, String>(3)?
                            .parse()
                            .unwrap_or(Engine::Google),
                        country: row.get(4)?,
                        language: row.get(5)?,
                        device: row.get(6)?,
                        created_at: row.get(7)?,
                        target_domain: row.get(8)?,
                        difficulty: row.get(9)?,
                        band: row.get(10)?,
                        intent: row.get(11)?,
                        opportunity: row.get(12)?,
                        volume: row.get(13)?,
                        models: row.get(14)?,
                        result: row.get(15)?,
                        inputs: row.get(16)?,
                        domains: row.get(17)?,
                        crawl_ids: row.get(18)?,
                        pages: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| SerpStoreError::NotFound(format!("analysis {id}")))?;
        let mut statement = self.connection.prepare(
            "SELECT position, url, domain, content, technical, site, strength, words
             FROM page_scores WHERE analysis_id = ? ORDER BY position = 0, position",
        )?;
        record.pages = statement
            .query_map(params![id], |row| {
                Ok(PageScoreRow {
                    position: row.get(0)?,
                    url: row.get(1)?,
                    domain: row.get(2)?,
                    content: row.get(3)?,
                    technical: row.get(4)?,
                    site: row.get(5)?,
                    strength: row.get(6)?,
                    words: row.get(7)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(record)
    }

    /// Stored analyses, newest first, optionally of one keyword.
    ///
    /// # Errors
    /// Database errors.
    pub fn analyses(
        &self,
        keyword: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AnalysisSummary>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, snapshot_id, keyword, engine, country, created_at, target_domain,
                    difficulty, band, intent, opportunity, volume, keyword_key, language
             FROM analyses ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                AnalysisSummary {
                    id: row.get(0)?,
                    snapshot_id: row.get(1)?,
                    keyword: row.get(2)?,
                    engine: row.get::<_, String>(3)?.parse().unwrap_or(Engine::Google),
                    country: row.get(4)?,
                    created_at: row.get(5)?,
                    target_domain: row.get(6)?,
                    difficulty: row.get(7)?,
                    band: row.get(8)?,
                    intent: row.get(9)?,
                    opportunity: row.get(10)?,
                    volume: row.get(11)?,
                },
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
            ))
        })?;
        let mut summaries = Vec::new();
        for row in rows {
            let (summary, key, language) = row?;
            if keyword.is_some_and(|keyword| keyword_key(keyword, &language) != key) {
                continue;
            }
            summaries.push(summary);
            if summaries.len() >= limit.max(1) {
                break;
            }
        }
        Ok(summaries)
    }
}

fn insert_pages(
    connection: &duckdb::Connection,
    id: i64,
    pages: &[PageScoreRow],
) -> Result<(), SerpStoreError> {
    let mut appender = connection.appender("page_scores")?;
    let mut seen = std::collections::HashSet::new();
    for page in pages.iter().filter(|page| seen.insert(page.url.clone())) {
        appender.append_row(params![
            id,
            page.position,
            page.url,
            page.domain,
            page.content,
            page.technical,
            page.site,
            page.strength,
            page.words
        ])?;
    }
    Ok(())
}
