//! Keywords with where they came from, their tags, search numbers and intents.

use std::collections::HashMap;

use duckdb::types::Value;
use duckdb::{params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::{KwStore, KwStoreError, MAX_PAGE, json, parse, stage_ids};

/// Separator of aggregated tags and sources; removed from tags on input.
const SEPARATOR: char = '\u{1f}';

/// A keyword to add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewKeyword {
    /// Display text.
    pub keyword: String,
    /// Identity: variants with the same key are one keyword.
    pub key: String,
    /// Word stems, for wording clusters.
    pub stems: Vec<String>,
    /// Where it came from (`import`, `wordstat`, `dataforseo-ideas` …).
    pub source: String,
    /// The seed or competitor it was found for.
    pub seed: Option<String>,
}

/// What an addition did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Added {
    /// Keywords given.
    pub received: u64,
    /// New keywords.
    pub added: u64,
    /// Keywords the project already had (their new source is recorded).
    pub known: u64,
}

/// Whether a keyword takes part in clustering and exports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeywordStatus {
    /// Used.
    #[default]
    Active,
    /// Left out by the user; kept so it is not discovered again.
    Excluded,
}

impl KeywordStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Excluded => "excluded",
        }
    }

    fn parse(text: &str) -> Result<Self, KwStoreError> {
        match text {
            "active" => Ok(Self::Active),
            "excluded" => Ok(Self::Excluded),
            other => Err(KwStoreError::Corrupt(format!("keyword status `{other}`"))),
        }
    }
}

/// Searches in one month.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyVolume {
    /// `YYYY-MM`.
    pub month: String,
    /// Searches.
    pub volume: u64,
}

/// Search numbers of a keyword from one source, engine and country.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    /// The keyword.
    pub keyword_id: i64,
    /// Source id.
    pub source: String,
    /// Engine the numbers are for.
    pub engine: String,
    /// Country code.
    pub country: String,
    /// Average monthly searches.
    pub volume: Option<u64>,
    /// Lower end, where only a range is known.
    pub volume_low: Option<u64>,
    /// Upper end, where only a range is known.
    pub volume_high: Option<u64>,
    /// Cost per click, US dollars.
    pub cpc_usd: Option<f64>,
    /// Advertiser competition, 0–1.
    pub competition: Option<f64>,
    /// Keyword difficulty, 0–100.
    pub difficulty: Option<f64>,
    /// Searches per month, oldest first.
    pub monthly: Vec<MonthlyVolume>,
    /// RFC 3339.
    pub fetched_at: String,
}

/// The intent of a keyword.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentRecord {
    /// The keyword.
    pub keyword_id: i64,
    /// Main intent (`informational`, `commercial` …).
    pub primary: String,
    /// Second intent, when there is one.
    pub secondary: Option<String>,
    /// Where it came from (`words`, `serp`, `dataforseo`).
    pub source: String,
}

/// Order of a keyword page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeywordSort {
    /// Alphabetical, ignoring case and diacritics.
    Keyword,
    /// Monthly searches.
    #[default]
    Volume,
    /// Cost per click.
    Cpc,
    /// Advertiser competition.
    Competition,
    /// Keyword difficulty.
    Difficulty,
    /// Intent.
    Intent,
    /// Cluster number of the newest clustering.
    Cluster,
    /// Order of addition.
    Added,
}

impl KeywordSort {
    const fn column(self) -> &'static str {
        match self {
            Self::Keyword => "k.key",
            Self::Volume => "n.volume",
            Self::Cpc => "n.cpc_usd",
            Self::Competition => "n.competition",
            Self::Difficulty => "n.difficulty",
            Self::Intent => "i.primary_intent",
            Self::Cluster => "mem.number",
            Self::Added => "k.id",
        }
    }
}

/// Which keywords a page shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeywordQuery {
    /// Text the identity must contain (fold it like the identity: lower case, no diacritics).
    pub search: Option<String>,
    /// Only this status; every status when absent.
    pub status: Option<KeywordStatus>,
    /// Only keywords with this tag.
    pub tag: Option<String>,
    /// Only keywords found by this source.
    pub source: Option<String>,
    /// Only this intent.
    pub intent: Option<String>,
    /// At least these searches.
    pub min_volume: Option<u64>,
    /// At most these searches.
    pub max_volume: Option<u64>,
    /// Only members of this cluster of the newest clustering.
    pub cluster: Option<u32>,
    /// Order.
    pub sort: KeywordSort,
    /// Largest first.
    pub descending: bool,
    /// Rows to skip.
    pub offset: u64,
    /// Rows to return, at most [`MAX_PAGE`].
    pub limit: u64,
}

impl Default for KeywordQuery {
    fn default() -> Self {
        Self {
            search: None,
            status: Some(KeywordStatus::Active),
            tag: None,
            source: None,
            intent: None,
            min_volume: None,
            max_volume: None,
            cluster: None,
            sort: KeywordSort::Volume,
            descending: true,
            offset: 0,
            limit: 100,
        }
    }
}

/// One keyword as listed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordRow {
    /// Keyword id.
    pub id: i64,
    /// Display text.
    pub keyword: String,
    /// Used or left out.
    pub status: KeywordStatus,
    /// Tags, sorted.
    pub tags: Vec<String>,
    /// Sources that found it, sorted.
    pub sources: Vec<String>,
    /// Monthly searches: the primary engine's newest number, else any newest number.
    pub volume: Option<u64>,
    /// Cost per click, US dollars.
    pub cpc_usd: Option<f64>,
    /// Advertiser competition, 0–1.
    pub competition: Option<f64>,
    /// Keyword difficulty, 0–100.
    pub difficulty: Option<f64>,
    /// Searches per month behind `volume`.
    pub monthly: Vec<MonthlyVolume>,
    /// Main intent.
    pub intent: Option<String>,
    /// Cluster number in the newest clustering.
    pub cluster: Option<u32>,
    /// Whether it is its cluster's pillar.
    pub pillar: bool,
    /// Whether a result page of the primary engine is stored.
    pub serp: bool,
}

/// A page of keywords.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordPage {
    /// Keywords matching the query.
    pub total: u64,
    /// The requested rows.
    pub rows: Vec<KeywordRow>,
}

/// What clustering needs of an active keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterInput {
    /// Keyword id.
    pub id: i64,
    /// Display text.
    pub keyword: String,
    /// Monthly searches.
    pub volume: Option<u64>,
    /// Cost per click, US dollars.
    pub cpc_usd: Option<f64>,
    /// Keyword difficulty.
    pub difficulty: Option<f64>,
    /// Main intent.
    pub intent: Option<String>,
    /// Top result addresses, when its result page was fetched.
    pub results: Option<Vec<String>>,
    /// Whether the addresses are sites only (Google's hidden links).
    pub site_only: bool,
}

/// The newest numbers of each keyword of project `$1`, preferring engine `$2`.
pub(crate) const NUMBERS: &str = "
    WITH numbers AS (
        SELECT m.keyword_id,
            arg_max(m.volume, m.rank) FILTER (WHERE m.volume IS NOT NULL) AS volume,
            arg_max(m.monthly, m.rank) FILTER (WHERE m.volume IS NOT NULL) AS monthly,
            arg_max(m.cpc_usd, m.rank) FILTER (WHERE m.cpc_usd IS NOT NULL) AS cpc_usd,
            arg_max(m.competition, m.rank) FILTER (WHERE m.competition IS NOT NULL) AS competition,
            arg_max(m.difficulty, m.rank) FILTER (WHERE m.difficulty IS NOT NULL) AS difficulty
        FROM (
            SELECT metrics.*,
                (metrics.engine = $2)::INTEGER::VARCHAR || metrics.fetched_at || metrics.source AS rank
            FROM metrics JOIN keywords k ON k.id = metrics.keyword_id AND k.project_id = $1
        ) m
        GROUP BY m.keyword_id
    ),
    latest AS (SELECT max(id) AS id FROM clusterings WHERE project_id = $1),
    tags AS (
        SELECT keyword_id, string_agg(tag, chr(31) ORDER BY tag) AS tags FROM keyword_tags
        WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = $1) GROUP BY keyword_id
    ),
    sources AS (
        SELECT keyword_id, string_agg(source, chr(31) ORDER BY source) AS sources
        FROM keyword_sources
        WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = $1) GROUP BY keyword_id
    ),
    fetched AS (SELECT DISTINCT keyword_id FROM serps WHERE engine = $2)";

impl KwStore {
    /// Adds keywords to a project. Variants with one key are merged (the first text wins);
    /// keywords the project has keep their text and status and gain the new source.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn add_keywords(
        &mut self,
        project_id: i64,
        keywords: &[NewKeyword],
        found_at: &str,
    ) -> Result<Added, KwStoreError> {
        self.primary_engine(project_id)?;
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM incoming_keywords", [])?;
        {
            let mut appender = transaction.appender("incoming_keywords")?;
            for (ordinal, keyword) in keywords.iter().enumerate() {
                appender.append_row(params![
                    u64::try_from(ordinal).unwrap_or(u64::MAX),
                    keyword.keyword,
                    keyword.key,
                    json(&keyword.stems)?,
                    keyword.source,
                    keyword.seed
                ])?;
            }
            appender.flush()?;
        }
        let distinct: u64 = transaction.query_row(
            "SELECT count(DISTINCT key)::UBIGINT FROM incoming_keywords",
            [],
            |row| row.get(0),
        )?;
        let added = transaction.execute(
            "INSERT INTO keywords (project_id, keyword, key, stems, added_at)
             SELECT ?, arg_min(keyword, ordinal), key, arg_min(stems, ordinal), ?
             FROM incoming_keywords
             WHERE key NOT IN (SELECT key FROM keywords WHERE project_id = ?)
             GROUP BY key ORDER BY min(ordinal)",
            params![project_id, found_at, project_id],
        )?;
        transaction.execute(
            "INSERT INTO keyword_sources (keyword_id, source, seed, found_at)
             SELECT k.id, i.source, arg_min(i.seed, i.ordinal), ?
             FROM incoming_keywords i JOIN keywords k ON k.project_id = ? AND k.key = i.key
             WHERE NOT EXISTS (
                 SELECT 1 FROM keyword_sources s WHERE s.keyword_id = k.id AND s.source = i.source
             )
             GROUP BY k.id, i.source",
            params![found_at, project_id],
        )?;
        transaction.execute("DELETE FROM incoming_keywords", [])?;
        transaction.commit()?;
        let added = u64::try_from(added).unwrap_or(u64::MAX);
        Ok(Added {
            received: u64::try_from(keywords.len()).unwrap_or(u64::MAX),
            added,
            known: distinct.saturating_sub(added),
        })
    }

    /// Keyword ids of a project by key.
    ///
    /// # Errors
    /// Database errors.
    pub fn key_index(&self, project_id: i64) -> Result<HashMap<String, i64>, KwStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT key, id FROM keywords WHERE project_id = ?")?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Keywords of a project without an intent, with their text.
    ///
    /// # Errors
    /// Database errors.
    pub fn keywords_without_intent(
        &self,
        project_id: i64,
    ) -> Result<Vec<(i64, String)>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT k.id, k.keyword FROM keywords k
             WHERE k.project_id = ? AND NOT EXISTS (SELECT 1 FROM intents i WHERE i.keyword_id = k.id)
             ORDER BY k.id",
        )?;
        let rows =
            statement.query_map(params![project_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Stores search numbers, replacing earlier numbers of the same keyword, source, engine
    /// and country. The last of several records for one of them wins.
    ///
    /// # Errors
    /// Database errors.
    pub fn put_metrics(&mut self, records: &[MetricRecord]) -> Result<(), KwStoreError> {
        let mut last = HashMap::new();
        for (index, record) in records.iter().enumerate() {
            last.insert(
                (
                    record.keyword_id,
                    record.source.as_str(),
                    record.engine.as_str(),
                    record.country.as_str(),
                ),
                index,
            );
        }
        let mut groups: HashMap<(&str, &str, &str), Vec<i64>> = HashMap::new();
        for (keyword_id, source, engine, country) in last.keys() {
            groups
                .entry((source, engine, country))
                .or_default()
                .push(*keyword_id);
        }
        let transaction = self.connection.transaction()?;
        for ((source, engine, country), ids) in &groups {
            stage_ids(&transaction, ids)?;
            transaction.execute(
                "DELETE FROM metrics WHERE source = ? AND engine = ? AND country = ?
                     AND keyword_id IN (SELECT id FROM incoming_ids)",
                params![source, engine, country],
            )?;
        }
        transaction.execute("DELETE FROM incoming_ids", [])?;
        {
            let mut appender = transaction.appender("metrics")?;
            for (index, record) in records.iter().enumerate() {
                let key = (
                    record.keyword_id,
                    record.source.as_str(),
                    record.engine.as_str(),
                    record.country.as_str(),
                );
                if last.get(&key) != Some(&index) {
                    continue;
                }
                appender.append_row(params![
                    record.keyword_id,
                    record.source,
                    record.engine,
                    record.country,
                    record.volume,
                    record.volume_low,
                    record.volume_high,
                    record.cpc_usd,
                    record.competition,
                    record.difficulty,
                    json(&record.monthly)?,
                    record.fetched_at
                ])?;
            }
            appender.flush()?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Stores intents, replacing a keyword's earlier intent.
    ///
    /// # Errors
    /// Database errors.
    pub fn put_intents(&mut self, records: &[IntentRecord]) -> Result<(), KwStoreError> {
        let mut last = HashMap::new();
        for (index, record) in records.iter().enumerate() {
            last.insert(record.keyword_id, index);
        }
        let ids: Vec<i64> = last.keys().copied().collect();
        let transaction = self.connection.transaction()?;
        stage_ids(&transaction, &ids)?;
        transaction.execute(
            "DELETE FROM intents WHERE keyword_id IN (SELECT id FROM incoming_ids)",
            [],
        )?;
        transaction.execute("DELETE FROM incoming_ids", [])?;
        {
            let mut appender = transaction.appender("intents")?;
            for (index, record) in records.iter().enumerate() {
                if last.get(&record.keyword_id) != Some(&index) {
                    continue;
                }
                appender.append_row(params![
                    record.keyword_id,
                    record.primary,
                    record.secondary,
                    record.source
                ])?;
            }
            appender.flush()?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Sets the status of keywords of a project; returns how many changed.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_status(
        &mut self,
        project_id: i64,
        ids: &[i64],
        status: KeywordStatus,
    ) -> Result<u64, KwStoreError> {
        let transaction = self.connection.transaction()?;
        stage_ids(&transaction, ids)?;
        let changed = transaction.execute(
            "UPDATE keywords SET status = ?
             WHERE project_id = ? AND status <> ? AND id IN (SELECT id FROM incoming_ids)",
            params![status.as_str(), project_id, status.as_str()],
        )?;
        transaction.execute("DELETE FROM incoming_ids", [])?;
        transaction.commit()?;
        Ok(u64::try_from(changed).unwrap_or(u64::MAX))
    }

    /// Adds and removes tags of keywords of a project.
    ///
    /// # Errors
    /// Database errors.
    pub fn tag(
        &mut self,
        project_id: i64,
        ids: &[i64],
        add: &[String],
        remove: &[String],
    ) -> Result<(), KwStoreError> {
        let transaction = self.connection.transaction()?;
        stage_ids(&transaction, ids)?;
        for tag in remove {
            transaction.execute(
                "DELETE FROM keyword_tags WHERE tag = ? AND keyword_id IN (
                     SELECT id FROM keywords
                     WHERE project_id = ? AND id IN (SELECT id FROM incoming_ids)
                 )",
                params![tag, project_id],
            )?;
        }
        for tag in add {
            let tag: String = tag.chars().filter(|&c| c != SEPARATOR).collect();
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            transaction.execute(
                "INSERT INTO keyword_tags (keyword_id, tag)
                 SELECT k.id, ? FROM keywords k
                 WHERE k.project_id = ? AND k.id IN (SELECT id FROM incoming_ids)
                     AND NOT EXISTS (
                         SELECT 1 FROM keyword_tags t WHERE t.keyword_id = k.id AND t.tag = ?
                     )",
                params![tag, project_id, tag],
            )?;
        }
        transaction.execute("DELETE FROM incoming_ids", [])?;
        transaction.commit()?;
        Ok(())
    }

    /// Every tag of a project with how many keywords carry it.
    ///
    /// # Errors
    /// Database errors.
    pub fn tags(&self, project_id: i64) -> Result<Vec<(String, u64)>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT t.tag, count(*)::UBIGINT FROM keyword_tags t
             JOIN keywords k ON k.id = t.keyword_id AND k.project_id = ?
             GROUP BY t.tag ORDER BY t.tag",
        )?;
        let rows =
            statement.query_map(params![project_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Sources of a project's keywords with how many keywords each found.
    ///
    /// # Errors
    /// Database errors.
    pub fn sources(&self, project_id: i64) -> Result<Vec<(String, u64)>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT s.source, count(*)::UBIGINT FROM keyword_sources s
             JOIN keywords k ON k.id = s.keyword_id AND k.project_id = ?
             GROUP BY s.source ORDER BY s.source",
        )?;
        let rows =
            statement.query_map(params![project_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A page of a project's keywords.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn keywords(
        &self,
        project_id: i64,
        query: &KeywordQuery,
    ) -> Result<KeywordPage, KwStoreError> {
        let engine = self.primary_engine(project_id)?;
        let mut values = vec![Value::BigInt(project_id), Value::Text(engine)];
        let conditions = conditions(query, &mut values);
        let mut bind = |value: Value| {
            values.push(value);
            format!("${}", values.len())
        };
        let limit = bind(Value::UBigInt(query.limit.clamp(1, MAX_PAGE)));
        let offset = bind(Value::UBigInt(query.offset));
        let direction = if query.descending { "DESC" } else { "ASC" };
        let sql = format!(
            "{NUMBERS}
             SELECT k.id, k.keyword, k.status, t.tags, s.sources, n.volume, n.cpc_usd,
                    n.competition, n.difficulty, n.monthly, i.primary_intent, mem.number,
                    coalesce(cl.pillar_id = k.id, false), f.keyword_id IS NOT NULL,
                    count(*) OVER ()::UBIGINT
             FROM keywords k
             LEFT JOIN numbers n ON n.keyword_id = k.id
             LEFT JOIN intents i ON i.keyword_id = k.id
             LEFT JOIN members mem ON mem.clustering_id = (SELECT id FROM latest)
                 AND mem.keyword_id = k.id
             LEFT JOIN clusters cl ON cl.clustering_id = mem.clustering_id AND cl.number = mem.number
             LEFT JOIN tags t ON t.keyword_id = k.id
             LEFT JOIN sources s ON s.keyword_id = k.id
             LEFT JOIN fetched f ON f.keyword_id = k.id
             WHERE {}
             ORDER BY {} {direction} NULLS LAST, k.id
             LIMIT {limit} OFFSET {offset}",
            conditions.join(" AND "),
            query.sort.column(),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok((
                KeywordRow {
                    id: row.get(0)?,
                    keyword: row.get(1)?,
                    status: KeywordStatus::Active,
                    tags: split(row.get(3)?),
                    sources: split(row.get(4)?),
                    volume: row.get(5)?,
                    cpc_usd: row.get(6)?,
                    competition: row.get(7)?,
                    difficulty: row.get(8)?,
                    monthly: Vec::new(),
                    intent: row.get(10)?,
                    cluster: row.get(11)?,
                    pillar: row.get(12)?,
                    serp: row.get(13)?,
                },
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, u64>(14)?,
            ))
        })?;
        let mut page = KeywordPage {
            total: 0,
            rows: Vec::new(),
        };
        for row in rows {
            let (mut keyword, status, monthly, total) = row?;
            keyword.status = KeywordStatus::parse(&status)?;
            if let Some(monthly) = monthly {
                keyword.monthly = parse(&monthly)?;
            }
            page.total = total;
            page.rows.push(keyword);
        }
        if page.rows.is_empty() && query.offset > 0 {
            page.total = self.count_matching(project_id, query)?;
        }
        Ok(page)
    }

    /// Matching keywords when a page past the end was asked.
    fn count_matching(&self, project_id: i64, query: &KeywordQuery) -> Result<u64, KwStoreError> {
        let first = KeywordQuery {
            offset: 0,
            limit: 1,
            ..query.clone()
        };
        Ok(self.keywords(project_id, &first)?.total)
    }

    /// What clustering needs of every active keyword, with result pages of one engine and
    /// device, in order of addition.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn cluster_inputs(
        &self,
        project_id: i64,
        engine: &str,
        device: &str,
    ) -> Result<Vec<ClusterInput>, KwStoreError> {
        let primary = self.primary_engine(project_id)?;
        let mut statement = self.connection.prepare(&format!(
            "{NUMBERS}
             SELECT k.id, k.keyword, n.volume, n.cpc_usd, n.difficulty, i.primary_intent,
                    sr.urls, coalesce(sr.site_only, false)
             FROM keywords k
             LEFT JOIN numbers n ON n.keyword_id = k.id
             LEFT JOIN intents i ON i.keyword_id = k.id
             LEFT JOIN serps sr ON sr.keyword_id = k.id AND sr.engine = $3 AND sr.device = $4
             WHERE k.project_id = $1 AND k.status = 'active'
             ORDER BY k.id"
        ))?;
        let rows = statement.query_map(params![project_id, primary, engine, device], |row| {
            Ok((
                ClusterInput {
                    id: row.get(0)?,
                    keyword: row.get(1)?,
                    volume: row.get(2)?,
                    cpc_usd: row.get(3)?,
                    difficulty: row.get(4)?,
                    intent: row.get(5)?,
                    results: None,
                    site_only: row.get(7)?,
                },
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        let mut inputs = Vec::new();
        for row in rows {
            let (mut input, urls) = row?;
            if let Some(urls) = urls {
                input.results = Some(parse(&urls)?);
            }
            inputs.push(input);
        }
        Ok(inputs)
    }
}

/// SQL conditions of a keyword query; their values are appended to `values`.
fn conditions(query: &KeywordQuery, values: &mut Vec<Value>) -> Vec<String> {
    let mut conditions = vec!["k.project_id = $1".to_owned()];
    let mut bind = |value: Value| {
        values.push(value);
        format!("${}", values.len())
    };
    if let Some(search) = query.search.as_deref().filter(|text| !text.is_empty()) {
        conditions.push(format!(
            "contains(k.key, {})",
            bind(Value::Text(search.to_owned()))
        ));
    }
    if let Some(status) = query.status {
        conditions.push(format!(
            "k.status = {}",
            bind(Value::Text(status.as_str().to_owned()))
        ));
    }
    if let Some(tag) = &query.tag {
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM keyword_tags kt WHERE kt.keyword_id = k.id AND kt.tag = {})",
            bind(Value::Text(tag.clone()))
        ));
    }
    if let Some(source) = &query.source {
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM keyword_sources ks WHERE ks.keyword_id = k.id AND ks.source = {})",
            bind(Value::Text(source.clone()))
        ));
    }
    if let Some(intent) = &query.intent {
        conditions.push(format!(
            "i.primary_intent = {}",
            bind(Value::Text(intent.clone()))
        ));
    }
    if let Some(min) = query.min_volume {
        conditions.push(format!("n.volume >= {}", bind(Value::UBigInt(min))));
    }
    if let Some(max) = query.max_volume {
        conditions.push(format!("n.volume <= {}", bind(Value::UBigInt(max))));
    }
    if let Some(cluster) = query.cluster {
        conditions.push(format!("mem.number = {}", bind(Value::UInt(cluster))));
    }
    conditions
}

fn split(joined: Option<String>) -> Vec<String> {
    joined
        .map(|text| text.split(SEPARATOR).map(str::to_owned).collect())
        .unwrap_or_default()
}
