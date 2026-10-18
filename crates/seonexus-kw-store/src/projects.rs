//! Projects: one market (country, language, engines), an optional site and its competitors.

use duckdb::params;
use serde::{Deserialize, Serialize};

use crate::{KwStore, KwStoreError, json, parse};

/// What a project is about, as entered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSpec {
    /// Display name.
    pub name: String,
    /// Country code (ISO 3166-1 alpha-2, lower case).
    pub country: String,
    /// Language code (ISO 639-1).
    pub language: String,
    /// Engines, the first one primary (`google`, `yandex`, `bing`).
    pub engines: Vec<String>,
    /// The site the keywords are for (domain or address).
    pub site: Option<String>,
    /// Competitor domains.
    pub competitors: Vec<String>,
}

/// A stored project with its counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRow {
    /// Project id.
    pub id: i64,
    /// What it is about.
    #[serde(flatten)]
    pub spec: ProjectSpec,
    /// RFC 3339.
    pub created_at: String,
    /// Active keywords.
    pub keywords: u64,
    /// Excluded keywords.
    pub excluded: u64,
    /// Clusters of the newest clustering.
    pub clusters: Option<u32>,
}

const SELECT: &str = "
    SELECT p.id, p.name, p.country, p.language, p.engines, p.site, p.competitors, p.created_at,
           (SELECT count(*) FROM keywords k WHERE k.project_id = p.id AND k.status = 'active')::UBIGINT,
           (SELECT count(*) FROM keywords k WHERE k.project_id = p.id AND k.status = 'excluded')::UBIGINT,
           (SELECT c.clusters FROM clusterings c WHERE c.project_id = p.id ORDER BY c.id DESC LIMIT 1)
    FROM projects p";

/// Everything a project owns, children first.
const DELETE_CONTENTS: &[&str] = &[
    "DELETE FROM members WHERE clustering_id IN (SELECT id FROM clusterings WHERE project_id = ?)",
    "DELETE FROM clusters WHERE clustering_id IN (SELECT id FROM clusterings WHERE project_id = ?)",
    "DELETE FROM clusterings WHERE project_id = ?",
    "DELETE FROM pins WHERE project_id = ?",
    "DELETE FROM site_pages WHERE project_id = ?",
    "DELETE FROM serps WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = ?)",
    "DELETE FROM intents WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = ?)",
    "DELETE FROM metrics WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = ?)",
    "DELETE FROM keyword_tags WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = ?)",
    "DELETE FROM keyword_sources WHERE keyword_id IN (SELECT id FROM keywords WHERE project_id = ?)",
    "DELETE FROM keywords WHERE project_id = ?",
];

type Raw = (
    i64,
    ProjectSpec,
    String,
    String,
    String,
    u64,
    u64,
    Option<u32>,
);

impl KwStore {
    /// Creates a project.
    ///
    /// # Errors
    /// Database errors.
    pub fn create_project(
        &self,
        spec: &ProjectSpec,
        created_at: &str,
    ) -> Result<i64, KwStoreError> {
        Ok(self.connection.query_row(
            "INSERT INTO projects (name, country, language, engines, site, competitors, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                spec.name,
                spec.country,
                spec.language,
                json(&spec.engines)?,
                spec.site,
                json(&spec.competitors)?,
                created_at
            ],
            |row| row.get(0),
        )?)
    }

    /// Changes a project.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn update_project(&self, id: i64, spec: &ProjectSpec) -> Result<(), KwStoreError> {
        let changed = self.connection.execute(
            "UPDATE projects SET name = ?, country = ?, language = ?, engines = ?, site = ?,
                 competitors = ?
             WHERE id = ?",
            params![
                spec.name,
                spec.country,
                spec.language,
                json(&spec.engines)?,
                spec.site,
                json(&spec.competitors)?,
                id
            ],
        )?;
        if changed == 0 {
            return Err(KwStoreError::NotFound(format!("project {id}")));
        }
        Ok(())
    }

    /// Every project, newest first.
    ///
    /// # Errors
    /// Database errors, invalid stored lists.
    pub fn projects(&self) -> Result<Vec<ProjectRow>, KwStoreError> {
        let mut statement = self
            .connection
            .prepare(&format!("{SELECT} ORDER BY p.id DESC"))?;
        let rows = statement.query_map([], read)?;
        rows.map(|row| finish(row?)).collect()
    }

    /// The primary engine of a project.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub(crate) fn primary_engine(&self, id: i64) -> Result<String, KwStoreError> {
        let engines: String = self
            .connection
            .query_row(
                "SELECT engines FROM projects WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| not_found(error, id))?;
        let engines: Vec<String> = parse(&engines)?;
        Ok(engines
            .into_iter()
            .next()
            .unwrap_or_else(|| "google".to_owned()))
    }

    /// One project.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn project(&self, id: i64) -> Result<ProjectRow, KwStoreError> {
        let mut statement = self
            .connection
            .prepare(&format!("{SELECT} WHERE p.id = ?"))?;
        let mut rows = statement.query_map(params![id], read)?;
        match rows.next() {
            Some(row) => finish(row?),
            None => Err(KwStoreError::NotFound(format!("project {id}"))),
        }
    }

    /// Settings of a project (clustering defaults and the like), `{}` when none were saved.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn project_settings(&self, id: i64) -> Result<serde_json::Value, KwStoreError> {
        let text: Option<String> = self
            .connection
            .query_row(
                "SELECT settings FROM projects WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| not_found(error, id))?;
        parse(text.as_deref().unwrap_or("{}"))
    }

    /// Replaces the settings of a project.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn set_project_settings(
        &self,
        id: i64,
        settings: &serde_json::Value,
    ) -> Result<(), KwStoreError> {
        let changed = self.connection.execute(
            "UPDATE projects SET settings = ? WHERE id = ?",
            params![json(settings)?, id],
        )?;
        if changed == 0 {
            return Err(KwStoreError::NotFound(format!("project {id}")));
        }
        Ok(())
    }

    /// Deletes a project with its keywords, numbers, result pages and clusterings. Runs and
    /// spending stay for the usage totals.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown project; database errors.
    pub fn delete_project(&mut self, id: i64) -> Result<(), KwStoreError> {
        let transaction = self.connection.transaction()?;
        for sql in DELETE_CONTENTS {
            transaction.execute(sql, params![id])?;
        }
        let deleted = transaction.execute("DELETE FROM projects WHERE id = ?", params![id])?;
        if deleted == 0 {
            return Err(KwStoreError::NotFound(format!("project {id}")));
        }
        transaction.commit()?;
        Ok(())
    }
}

fn not_found(error: duckdb::Error, id: i64) -> KwStoreError {
    match error {
        duckdb::Error::QueryReturnedNoRows => KwStoreError::NotFound(format!("project {id}")),
        other => other.into(),
    }
}

fn read(row: &duckdb::Row<'_>) -> duckdb::Result<Raw> {
    Ok((
        row.get(0)?,
        ProjectSpec {
            name: row.get(1)?,
            country: row.get(2)?,
            language: row.get(3)?,
            engines: Vec::new(),
            site: row.get(5)?,
            competitors: Vec::new(),
        },
        row.get(4)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
    ))
}

fn finish(raw: Raw) -> Result<ProjectRow, KwStoreError> {
    let (id, mut spec, engines, competitors, created_at, keywords, excluded, clusters) = raw;
    spec.engines = parse(&engines)?;
    spec.competitors = parse(&competitors)?;
    Ok(ProjectRow {
        id,
        spec,
        created_at,
        keywords,
        excluded,
        clusters,
    })
}
