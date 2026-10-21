//! Projects: the site every module works on, its market, competitors and brand names.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError, list_json, parse_list};

/// Most competitors a project compares with.
pub const MAX_COMPETITORS: usize = 10;

/// Most brand names a project watches for in AI answers.
pub const MAX_BRANDS: usize = 20;

/// Longest project name.
const MAX_NAME: usize = 120;

/// Longest brand name.
const MAX_BRAND: usize = 80;

/// What the user enters for a project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSpec {
    /// Display name; the domain when empty.
    #[serde(default)]
    pub name: String,
    /// The site's domain (`example.com`); an address is reduced to its domain.
    pub domain: String,
    /// ISO 3166 country code (`tr`).
    pub country: String,
    /// ISO 639 language code (`tr`).
    pub language: String,
    /// Competitors' domains.
    #[serde(default)]
    pub competitors: Vec<String>,
    /// The brand's names and spellings, for AI answers.
    #[serde(default)]
    pub brands: Vec<String>,
}

/// A stored project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRow {
    /// Project id.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// The site's domain.
    pub domain: String,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Competitors' domains.
    pub competitors: Vec<String>,
    /// Brand names.
    pub brands: Vec<String>,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
}

/// A site's domain from what a user typed: host only, lowercase, without `www.`, a port or a
/// path. Empty when nothing is left.
pub fn normalize_domain(input: &str) -> String {
    let text = input.trim().to_lowercase();
    let rest = text
        .split_once("://")
        .map_or(text.as_str(), |(_, rest)| rest);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    host.strip_prefix("www.")
        .unwrap_or(host)
        .trim_end_matches('.')
        .to_owned()
}

fn valid_domain(domain: &str) -> bool {
    domain.len() <= 253
        && domain.contains('.')
        && !domain.starts_with(['.', '-'])
        && domain
            .split('.')
            .all(|label| !label.is_empty() && label.len() <= 63)
        && domain
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '.')
}

fn code(text: &str, lengths: std::ops::RangeInclusive<usize>) -> Option<String> {
    let code = text.trim().to_ascii_lowercase();
    (lengths.contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase())).then_some(code)
}

impl ProjectSpec {
    /// The spec with every field cleaned: names trimmed, domains reduced and deduplicated, the
    /// own domain left out of the competitors.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] naming the first field that cannot be used.
    pub fn normalized(&self) -> Result<Self, PlatformStoreError> {
        let invalid = |message: &str| PlatformStoreError::Invalid(message.to_owned());
        let domain = normalize_domain(&self.domain);
        if !valid_domain(&domain) {
            return Err(invalid("domain: enter a domain such as example.com"));
        }
        let name = match self.name.trim() {
            "" => domain.clone(),
            name if name.chars().count() > MAX_NAME => {
                return Err(invalid("name: at most 120 characters"));
            }
            name => name.to_owned(),
        };
        let country = code(&self.country, 2..=2)
            .ok_or_else(|| invalid("country: a two-letter country code such as tr"))?;
        let language = code(&self.language, 2..=3)
            .ok_or_else(|| invalid("language: a language code such as tr"))?;
        let mut competitors: Vec<String> = Vec::new();
        for competitor in &self.competitors {
            let competitor = normalize_domain(competitor);
            if competitor.is_empty() || competitor == domain || competitors.contains(&competitor) {
                continue;
            }
            if !valid_domain(&competitor) {
                return Err(PlatformStoreError::Invalid(format!(
                    "competitors: `{competitor}` is not a domain"
                )));
            }
            competitors.push(competitor);
        }
        if competitors.len() > MAX_COMPETITORS {
            return Err(invalid("competitors: at most 10"));
        }
        let mut brands: Vec<String> = Vec::new();
        for brand in &self.brands {
            let brand = brand.trim();
            if brand.is_empty()
                || brands
                    .iter()
                    .any(|known| known.to_lowercase() == brand.to_lowercase())
            {
                continue;
            }
            if brand.chars().count() > MAX_BRAND {
                return Err(invalid("brands: at most 80 characters each"));
            }
            brands.push(brand.to_owned());
        }
        if brands.len() > MAX_BRANDS {
            return Err(invalid("brands: at most 20"));
        }
        Ok(Self {
            name,
            domain,
            country,
            language,
            competitors,
            brands,
        })
    }
}

const COLUMNS: &str =
    "id, name, domain, country, language, competitors, brands, created_at, updated_at";

fn project(row: &Row<'_>) -> duckdb::Result<(ProjectRow, String, String)> {
    Ok((
        ProjectRow {
            id: row.get(0)?,
            name: row.get(1)?,
            domain: row.get(2)?,
            country: row.get(3)?,
            language: row.get(4)?,
            competitors: Vec::new(),
            brands: Vec::new(),
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        },
        row.get(5)?,
        row.get(6)?,
    ))
}

fn finish(
    (mut row, competitors, brands): (ProjectRow, String, String),
) -> Result<ProjectRow, PlatformStoreError> {
    row.competitors = parse_list(&competitors)?;
    row.brands = parse_list(&brands)?;
    Ok(row)
}

impl PlatformStore {
    /// Every project, by name.
    ///
    /// # Errors
    /// Database errors.
    pub fn projects(&self) -> Result<Vec<ProjectRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM projects ORDER BY lower(name), id"
        ))?;
        let rows = statement.query_map([], project)?;
        rows.map(|row| finish(row?)).collect()
    }

    /// One project.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn project(&self, id: i64) -> Result<ProjectRow, PlatformStoreError> {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT {COLUMNS} FROM projects WHERE id = ?"))?;
        let mut rows = statement.query_map(params![id], project)?;
        match rows.next() {
            Some(row) => finish(row?),
            None => Err(PlatformStoreError::NotFound(format!("project {id}"))),
        }
    }

    /// Saves a new project at `now` (RFC 3339).
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unusable spec, or database errors.
    pub fn create_project(
        &self,
        spec: &ProjectSpec,
        now: &str,
    ) -> Result<ProjectRow, PlatformStoreError> {
        let spec = spec.normalized()?;
        let id: i64 = self.connection.query_row(
            "INSERT INTO projects (name, domain, country, language, competitors, brands, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                spec.name,
                spec.domain,
                spec.country,
                spec.language,
                list_json(&spec.competitors),
                list_json(&spec.brands),
                now,
                now
            ],
            |row| row.get(0),
        )?;
        self.project(id)
    }

    /// Replaces a project's fields at `now` (RFC 3339).
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`], [`PlatformStoreError::Invalid`] or database errors.
    pub fn update_project(
        &self,
        id: i64,
        spec: &ProjectSpec,
        now: &str,
    ) -> Result<ProjectRow, PlatformStoreError> {
        let spec = spec.normalized()?;
        let changed = self.connection.execute(
            "UPDATE projects SET name = ?, domain = ?, country = ?, language = ?, competitors = ?,
             brands = ?, updated_at = ? WHERE id = ?",
            params![
                spec.name,
                spec.domain,
                spec.country,
                spec.language,
                list_json(&spec.competitors),
                list_json(&spec.brands),
                now,
                id
            ],
        )?;
        if changed == 0 {
            return Err(PlatformStoreError::NotFound(format!("project {id}")));
        }
        self.project(id)
    }

    /// Removes a project with its cached answers, keyword lists, rank tracking, audit records
    /// (the caller deletes the audits' crawl folders) and AI visibility; spending stays for the
    /// usage history.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn delete_project(&self, id: i64) -> Result<(), PlatformStoreError> {
        self.connection
            .execute("DELETE FROM cache WHERE project_id = ?", params![id])?;
        self.connection.execute(
            "DELETE FROM list_keywords WHERE list_id IN (SELECT id FROM keyword_lists WHERE project_id = ?)",
            params![id],
        )?;
        self.connection.execute(
            "DELETE FROM keyword_lists WHERE project_id = ?",
            params![id],
        )?;
        self.delete_rank_data(id)?;
        self.delete_audit_data(id)?;
        self.delete_ai_data(id)?;
        self.delete_gsc_data(id)?;
        self.delete_report_data(id)?;
        let removed = self
            .connection
            .execute("DELETE FROM projects WHERE id = ?", params![id])?;
        if removed == 0 {
            return Err(PlatformStoreError::NotFound(format!("project {id}")));
        }
        Ok(())
    }
}
