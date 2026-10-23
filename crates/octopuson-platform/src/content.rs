//! Content briefs: what to write for a keyword, made from the pages ranking for it, with what
//! they were made from (to score pages against later), the brief an assistant wrote from it,
//! and the pages and drafts scored against it.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlatformStore, PlatformStoreError};

/// Longest keyword of a brief.
const MAX_KEYWORD: usize = 200;

/// Longest draft scored against a brief, characters.
pub const MAX_DRAFT_CHARS: usize = 200_000;

/// A brief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentBriefRow {
    /// Brief id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// The keyword it is for.
    pub keyword: String,
    /// Country of the result page read (ISO code).
    pub country: String,
    /// Its language.
    pub language: String,
    /// Read on a phone.
    pub mobile: bool,
    /// `running`, `done`, `failed` or `interrupted`.
    pub status: String,
    /// The brief, once made.
    pub brief: Option<Value>,
    /// The brief an assistant wrote from it (Markdown).
    pub ai_text: Option<String>,
    /// The assistant and model that wrote it (`claude:claude-opus-5-5`).
    pub ai_model: Option<String>,
    /// Paid for it so far, dollars.
    pub cost_usd: f64,
    /// Why it failed.
    pub error: Option<String>,
    /// Made at, RFC 3339.
    pub created_at: String,
    /// Last changed at, RFC 3339.
    pub updated_at: String,
}

/// A page or draft scored against a brief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentScoreRow {
    /// Score id.
    pub id: i64,
    /// The brief.
    pub brief_id: i64,
    /// Its project.
    pub project_id: i64,
    /// `url` (a published page) or `draft` (text pasted in).
    pub kind: String,
    /// The page (published pages).
    pub url: Option<String>,
    /// The draft's title.
    pub title: Option<String>,
    /// The draft's meta description.
    pub description: Option<String>,
    /// The draft (Markdown).
    pub body: Option<String>,
    /// 0–100.
    pub score: f64,
    /// What the score found.
    pub result: Value,
    /// Improvements an assistant suggested (Markdown).
    pub suggestions: Option<String>,
    /// Scored at, RFC 3339.
    pub scored_at: String,
}

const BRIEF_COLUMNS: &str = "id, project_id, keyword, country, language, mobile, status, brief, ai_text, ai_model, cost_usd, error, created_at, updated_at";

const SCORE_COLUMNS: &str = "id, brief_id, project_id, kind, url, title, description, body, score, result, suggestions, scored_at";

fn json_of(text: Option<String>) -> Result<Option<Value>, PlatformStoreError> {
    text.map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|error| PlatformStoreError::Corrupt(format!("content: {error}")))
}

type BriefColumns = (
    (i64, i64, String, String, String, bool, String),
    (
        Option<String>,
        Option<String>,
        Option<String>,
        f64,
        Option<String>,
        String,
        String,
    ),
);

fn brief_columns(row: &Row<'_>) -> duckdb::Result<BriefColumns> {
    Ok((
        (
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
        ),
        (
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
            row.get(11)?,
            row.get(12)?,
            row.get(13)?,
        ),
    ))
}

fn brief_row(columns: BriefColumns) -> Result<ContentBriefRow, PlatformStoreError> {
    let (
        (id, project_id, keyword, country, language, mobile, status),
        (brief, ai_text, ai_model, cost_usd, error, created_at, updated_at),
    ) = columns;
    Ok(ContentBriefRow {
        id,
        project_id,
        keyword,
        country,
        language,
        mobile,
        status,
        brief: json_of(brief)?,
        ai_text,
        ai_model,
        cost_usd,
        error,
        created_at,
        updated_at,
    })
}

type ScoreColumns = (
    (i64, i64, i64, String, Option<String>, Option<String>),
    (
        Option<String>,
        Option<String>,
        f64,
        String,
        Option<String>,
        String,
    ),
);

fn score_columns(row: &Row<'_>) -> duckdb::Result<ScoreColumns> {
    Ok((
        (
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ),
        (
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
            row.get(11)?,
        ),
    ))
}

fn score_row(columns: ScoreColumns) -> Result<ContentScoreRow, PlatformStoreError> {
    let (
        (id, brief_id, project_id, kind, url, title),
        (description, body, score, result, suggestions, scored_at),
    ) = columns;
    Ok(ContentScoreRow {
        id,
        brief_id,
        project_id,
        kind,
        url,
        title,
        description,
        body,
        score,
        result: json_of(Some(result))?.unwrap_or(Value::Null),
        suggestions,
        scored_at,
    })
}

/// A page of the project worth checking against the pages ranking for its keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageCandidate {
    /// The page.
    pub url: String,
    /// The keyword it ranks for, or gets most clicks from.
    pub keyword: String,
    /// `rank` (a tracked keyword's latest check) or `gsc` (Search Console).
    pub source: String,
    /// Its position for the keyword.
    pub position: Option<f64>,
    /// Clicks from the keyword in Search Console's last period.
    pub clicks: Option<f64>,
    /// Monthly searches of the keyword, when known.
    pub volume: Option<u64>,
}

/// What a page or draft scored, to save.
#[derive(Debug, Clone, Copy)]
pub struct ScoreToSave<'a> {
    /// `url` or `draft`.
    pub kind: &'a str,
    /// The page (published pages).
    pub url: Option<&'a str>,
    /// The draft's title.
    pub title: Option<&'a str>,
    /// The draft's meta description.
    pub description: Option<&'a str>,
    /// The draft.
    pub body: Option<&'a str>,
    /// 0–100.
    pub score: f64,
    /// What the score found.
    pub result: &'a Value,
}

impl PlatformStore {
    /// Starts a brief for `keyword` at `now`; a brief of the project left running by a program
    /// that stopped becomes `interrupted`.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an empty or long keyword, or database errors.
    pub fn begin_content_brief(
        &self,
        project_id: i64,
        keyword: &str,
        market: (&str, &str),
        mobile: bool,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        let keyword = keyword.trim();
        if keyword.is_empty() || keyword.chars().count() > MAX_KEYWORD {
            return Err(PlatformStoreError::Invalid(format!(
                "a brief's keyword has 1 to {MAX_KEYWORD} characters"
            )));
        }
        self.connection.execute(
            "UPDATE content_briefs SET status = 'interrupted', updated_at = ?
             WHERE project_id = ? AND status = 'running'",
            params![now, project_id],
        )?;
        Ok(self.connection.query_row(
            "INSERT INTO content_briefs (project_id, keyword, country, language, mobile, status,
             created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'running', ?, ?) RETURNING id",
            params![project_id, keyword, market.0, market.1, mobile, now, now],
            |row| row.get(0),
        )?)
    }

    /// Ends a brief with what it found and what it was made from.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn finish_content_brief(
        &self,
        id: i64,
        brief: &Value,
        source: &Value,
        cost_usd: f64,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        let changed = self.connection.execute(
            "UPDATE content_briefs SET status = 'done', brief = ?, source = ?,
             cost_usd = cost_usd + ?, error = NULL, updated_at = ? WHERE id = ?",
            params![brief.to_string(), source.to_string(), cost_usd, now, id],
        )?;
        if changed == 0 {
            return Err(PlatformStoreError::NotFound(format!("content brief {id}")));
        }
        Ok(())
    }

    /// Ends a brief that could not be made.
    ///
    /// # Errors
    /// Database errors.
    pub fn fail_content_brief(
        &self,
        id: i64,
        error: &str,
        cost_usd: f64,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE content_briefs SET status = 'failed', error = ?, cost_usd = cost_usd + ?,
             updated_at = ? WHERE id = ?",
            params![error, cost_usd, now, id],
        )?;
        Ok(())
    }

    /// Keeps what an assistant wrote from a brief, adding what it cost.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn note_brief_writing(
        &self,
        id: i64,
        text: &str,
        model: &str,
        cost_usd: f64,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        let changed = self.connection.execute(
            "UPDATE content_briefs SET ai_text = ?, ai_model = ?, cost_usd = cost_usd + ?,
             updated_at = ? WHERE id = ?",
            params![text, model, cost_usd, now, id],
        )?;
        if changed == 0 {
            return Err(PlatformStoreError::NotFound(format!("content brief {id}")));
        }
        Ok(())
    }

    /// Adds what writing from a brief cost (improvements of a page scored against it).
    ///
    /// # Errors
    /// Database errors.
    pub fn add_content_cost(
        &self,
        id: i64,
        cost_usd: f64,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE content_briefs SET cost_usd = cost_usd + ?, updated_at = ? WHERE id = ?",
            params![cost_usd, now, id],
        )?;
        Ok(())
    }

    /// A project's briefs, newest first.
    ///
    /// # Errors
    /// Database errors, or stored briefs that are not JSON.
    pub fn content_briefs(
        &self,
        project_id: i64,
    ) -> Result<Vec<ContentBriefRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {BRIEF_COLUMNS} FROM content_briefs WHERE project_id = ?
             ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = statement.query_map(params![project_id], brief_columns)?;
        rows.map(|row| brief_row(row?)).collect()
    }

    /// A brief.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn content_brief(&self, id: i64) -> Result<ContentBriefRow, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {BRIEF_COLUMNS} FROM content_briefs WHERE id = ?"
        ))?;
        let mut rows = statement.query_map(params![id], brief_columns)?;
        match rows.next() {
            Some(row) => brief_row(row?),
            None => Err(PlatformStoreError::NotFound(format!("content brief {id}"))),
        }
    }

    /// What a brief was made from (its result page and ranking pages), once made.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn content_brief_source(&self, id: i64) -> Result<Option<Value>, PlatformStoreError> {
        let source: Option<Option<String>> = self
            .connection
            .query_row(
                "SELECT source FROM content_briefs WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                duckdb::Error::QueryReturnedNoRows => Ok(None),
                error => Err(error),
            })?;
        match source {
            Some(source) => json_of(source),
            None => Err(PlatformStoreError::NotFound(format!("content brief {id}"))),
        }
    }

    /// Removes a brief with the pages and drafts scored against it.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn delete_content_brief(&self, id: i64) -> Result<(), PlatformStoreError> {
        self.connection
            .execute("DELETE FROM content_scores WHERE brief_id = ?", params![id])?;
        let removed = self
            .connection
            .execute("DELETE FROM content_briefs WHERE id = ?", params![id])?;
        if removed == 0 {
            return Err(PlatformStoreError::NotFound(format!("content brief {id}")));
        }
        Ok(())
    }

    /// Keeps what a page or draft scored against a brief: a page scored again, or the draft,
    /// replaces what it scored before. Returns the score's id.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown kind or a long draft, or database errors.
    pub fn save_content_score(
        &self,
        brief_id: i64,
        project_id: i64,
        score: ScoreToSave<'_>,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        if !matches!(score.kind, "url" | "draft") {
            return Err(PlatformStoreError::Invalid(format!(
                "unknown score kind {}",
                score.kind
            )));
        }
        if score
            .body
            .is_some_and(|body| body.chars().count() > MAX_DRAFT_CHARS)
        {
            return Err(PlatformStoreError::Invalid(format!(
                "a draft has at most {MAX_DRAFT_CHARS} characters"
            )));
        }
        let url = if score.kind == "url" { score.url } else { None };
        let existing: Option<i64> = self
            .connection
            .query_row(
                "SELECT id FROM content_scores WHERE brief_id = ? AND kind = ?
                 AND coalesce(url, '') = coalesce(?, '')",
                params![brief_id, score.kind, url],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                duckdb::Error::QueryReturnedNoRows => Ok(None),
                error => Err(error),
            })?;
        let result = score.result.to_string();
        if let Some(id) = existing {
            self.connection.execute(
                "UPDATE content_scores SET title = ?, description = ?, body = ?, score = ?,
                 result = ?, suggestions = NULL, scored_at = ? WHERE id = ?",
                params![
                    score.title,
                    score.description,
                    score.body,
                    score.score,
                    result,
                    now,
                    id
                ],
            )?;
            return Ok(id);
        }
        Ok(self.connection.query_row(
            "INSERT INTO content_scores (brief_id, project_id, kind, url, title, description,
             body, score, result, scored_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                brief_id,
                project_id,
                score.kind,
                url,
                score.title,
                score.description,
                score.body,
                score.score,
                result,
                now
            ],
            |row| row.get(0),
        )?)
    }

    /// Keeps the improvements an assistant suggested for a scored page or draft.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn note_score_suggestions(&self, id: i64, text: &str) -> Result<(), PlatformStoreError> {
        let changed = self.connection.execute(
            "UPDATE content_scores SET suggestions = ? WHERE id = ?",
            params![text, id],
        )?;
        if changed == 0 {
            return Err(PlatformStoreError::NotFound(format!("content score {id}")));
        }
        Ok(())
    }

    /// The pages and drafts scored against a brief, the draft first, then the best pages.
    ///
    /// # Errors
    /// Database errors, or stored results that are not JSON.
    pub fn content_scores(
        &self,
        brief_id: i64,
    ) -> Result<Vec<ContentScoreRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SCORE_COLUMNS} FROM content_scores WHERE brief_id = ?
             ORDER BY kind = 'draft' DESC, score DESC, id"
        ))?;
        let rows = statement.query_map(params![brief_id], score_columns)?;
        rows.map(|row| score_row(row?)).collect()
    }

    /// A scored page or draft.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn content_score(&self, id: i64) -> Result<ContentScoreRow, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SCORE_COLUMNS} FROM content_scores WHERE id = ?"
        ))?;
        let mut rows = statement.query_map(params![id], score_columns)?;
        match rows.next() {
            Some(row) => score_row(row?),
            None => Err(PlatformStoreError::NotFound(format!("content score {id}"))),
        }
    }

    /// The latest score of each page of the project scored against one of its briefs, best
    /// known per URL, with the brief's keyword.
    ///
    /// # Errors
    /// Database errors.
    pub fn content_page_scores(
        &self,
        project_id: i64,
    ) -> Result<Vec<(String, String, f64, String)>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT s.url, b.keyword, s.score, s.scored_at FROM content_scores s
             JOIN content_briefs b ON b.id = s.brief_id
             WHERE s.project_id = ? AND s.kind = 'url' AND s.url IS NOT NULL
             QUALIFY row_number() OVER (PARTITION BY s.url ORDER BY s.scored_at DESC, s.id DESC) = 1
             ORDER BY s.score, s.url",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Pages worth checking: the pages ranking for the project's tracked keywords (each
    /// keyword's latest check), then the pages Search Console shows with the query bringing
    /// each the most clicks. One entry per page, tracked keywords first.
    ///
    /// # Errors
    /// Database errors.
    pub fn content_candidates(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<PageCandidate>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "WITH ranked AS (
                SELECT c.url, k.keyword, 'rank' AS source, c.position::DOUBLE AS position,
                       NULL::DOUBLE AS clicks, k.volume, 0 AS tier,
                       coalesce(k.volume, 0)::DOUBLE AS weight
                FROM rank_checks c JOIN tracked_keywords k ON k.id = c.keyword_id
                WHERE c.project_id = ? AND c.url IS NOT NULL
                QUALIFY row_number() OVER (PARTITION BY c.keyword_id ORDER BY c.checked_at DESC) = 1
            ), searched AS (
                SELECT page AS url, query AS keyword, 'gsc' AS source, position, clicks,
                       NULL::UBIGINT AS volume, 1 AS tier, clicks AS weight
                FROM gsc_query_pages WHERE project_id = ?
                QUALIFY row_number() OVER (PARTITION BY page ORDER BY clicks DESC, impressions DESC, query) = 1
            )
            SELECT url, keyword, source, position, clicks, volume FROM (
                SELECT * FROM ranked UNION ALL SELECT * FROM searched
            )
            QUALIFY row_number() OVER (PARTITION BY url ORDER BY tier, weight DESC, keyword) = 1
            ORDER BY tier, weight DESC, url LIMIT ?",
        )?;
        let rows = statement.query_map(params![project_id, project_id, limit], |row| {
            Ok(PageCandidate {
                url: row.get(0)?,
                keyword: row.get(1)?,
                source: row.get(2)?,
                position: row.get(3)?,
                clicks: row.get(4)?,
                volume: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Removes everything a project's content work holds.
    pub(crate) fn delete_content_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in ["content_scores", "content_briefs"] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
