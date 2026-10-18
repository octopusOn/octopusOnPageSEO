//! Rank tracking projects: a site, where it is tracked, its keywords and the positions found.

use duckdb::{OptionalExt, params};
use octopuson_serp::{Device, Engine, Location, Target, keyword_key};
use serde::{Deserialize, Serialize};

use crate::{SerpStore, SerpStoreError};

/// A project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRow {
    /// Id.
    pub id: i64,
    /// Name.
    pub name: String,
    /// The tracked site.
    pub target: Target,
    /// `daily`, `weekly` or `manual`.
    pub schedule: String,
    /// Providers to use instead of the engines' chains.
    pub providers: Option<Vec<String>>,
    /// When it was created.
    pub created_at: String,
    /// When it was last checked.
    pub last_run_at: Option<String>,
}

/// Where a project is tracked: an engine, a locale and a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerRow {
    /// Id (ignored when adding).
    pub id: i64,
    /// Project.
    pub project_id: i64,
    /// Engine.
    pub engine: Engine,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Device.
    pub device: Device,
    /// Depth.
    pub depth: u16,
    /// City or region.
    pub location: Option<Location>,
}

/// A tracked keyword.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeywordRow {
    /// Id.
    pub id: i64,
    /// Project.
    pub project_id: i64,
    /// Keyword.
    pub keyword: String,
    /// Tags.
    pub tags: Vec<String>,
    /// When it was added.
    pub created_at: String,
}

/// A position found by a rank check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankingRow {
    /// Run.
    pub run_id: i64,
    /// Project.
    pub project_id: i64,
    /// Keyword.
    pub keyword_id: i64,
    /// Tracker.
    pub tracker_id: i64,
    /// Snapshot the position was read from.
    pub snapshot_id: Option<i64>,
    /// When it was checked.
    pub checked_at: String,
    /// Best position, `None` when the site does not rank within the depth.
    pub position: Option<u16>,
    /// URL at that position.
    pub url: Option<String>,
    /// Every position the site holds.
    pub positions: Vec<u16>,
    /// Why the check failed.
    pub error: Option<String>,
}

/// Columns of a project row: id, name, target, schedule, providers, created, last run.
type ProjectFields = (
    i64,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
);

fn json<T: Serialize + ?Sized>(value: &T) -> Result<String, SerpStoreError> {
    serde_json::to_string(value).map_err(|error| SerpStoreError::Corrupt(error.to_string()))
}

fn parse<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, SerpStoreError> {
    serde_json::from_str(text).map_err(|error| SerpStoreError::Corrupt(error.to_string()))
}

impl SerpStore {
    /// Creates a project. Returns its id.
    ///
    /// # Errors
    /// Database errors.
    pub fn create_project(&self, project: &ProjectRow) -> Result<i64, SerpStoreError> {
        Ok(self.connection.query_row(
            "INSERT INTO projects (name, target, schedule, providers, created_at)
             VALUES (?, ?, ?, ?, ?) RETURNING id",
            params![
                project.name,
                json(&project.target)?,
                project.schedule,
                project.providers.as_ref().map(json).transpose()?,
                project.created_at
            ],
            |row| row.get(0),
        )?)
    }

    /// Updates a project's name, target, schedule and providers.
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn update_project(&self, project: &ProjectRow) -> Result<(), SerpStoreError> {
        let updated = self.connection.execute(
            "UPDATE projects SET name = ?, target = ?, schedule = ?, providers = ? WHERE id = ?",
            params![
                project.name,
                json(&project.target)?,
                project.schedule,
                project.providers.as_ref().map(json).transpose()?,
                project.id
            ],
        )?;
        if updated == 0 {
            return Err(SerpStoreError::NotFound(format!("project {}", project.id)));
        }
        Ok(())
    }

    /// Records that a project was checked.
    ///
    /// # Errors
    /// Database errors.
    pub fn mark_project_run(&self, project_id: i64, at: &str) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "UPDATE projects SET last_run_at = ? WHERE id = ?",
            params![at, project_id],
        )?;
        Ok(())
    }

    /// Deletes a project with its trackers, keywords, competitors and rankings (snapshots
    /// stay).
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn delete_project(&mut self, project_id: i64) -> Result<(), SerpStoreError> {
        let transaction = self.connection.transaction()?;
        for table in ["rankings", "keywords", "trackers", "competitors"] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        let deleted =
            transaction.execute("DELETE FROM projects WHERE id = ?", params![project_id])?;
        transaction.commit()?;
        if deleted == 0 {
            return Err(SerpStoreError::NotFound(format!("project {project_id}")));
        }
        Ok(())
    }

    fn project_from(row: &duckdb::Row<'_>) -> duckdb::Result<ProjectFields> {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
        ))
    }

    /// Every project, oldest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn projects(&self) -> Result<Vec<ProjectRow>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, target, schedule, providers, created_at, last_run_at FROM projects ORDER BY id",
        )?;
        let rows = statement.query_map([], Self::project_from)?;
        rows.map(|row| {
            let (id, name, target, schedule, providers, created_at, last_run_at) = row?;
            Ok(ProjectRow {
                id,
                name,
                target: parse(&target)?,
                schedule,
                providers: providers.as_deref().map(parse).transpose()?,
                created_at,
                last_run_at,
            })
        })
        .collect()
    }

    /// A project.
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn project(&self, project_id: i64) -> Result<ProjectRow, SerpStoreError> {
        self.projects()?
            .into_iter()
            .find(|project| project.id == project_id)
            .ok_or_else(|| SerpStoreError::NotFound(format!("project {project_id}")))
    }

    /// Adds a tracker. Returns its id.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_tracker(&self, tracker: &TrackerRow) -> Result<i64, SerpStoreError> {
        Ok(self.connection.query_row(
            "INSERT INTO trackers (project_id, engine, country, language, device, depth, location)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                tracker.project_id,
                tracker.engine.as_str(),
                tracker.country,
                tracker.language,
                tracker.device.as_str(),
                tracker.depth,
                tracker.location.as_ref().map(json).transpose()?
            ],
            |row| row.get(0),
        )?)
    }

    /// Removes a tracker and its rankings.
    ///
    /// # Errors
    /// Database errors.
    pub fn remove_tracker(&self, project_id: i64, tracker_id: i64) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "DELETE FROM rankings WHERE project_id = ? AND tracker_id = ?",
            params![project_id, tracker_id],
        )?;
        self.connection.execute(
            "DELETE FROM trackers WHERE project_id = ? AND id = ?",
            params![project_id, tracker_id],
        )?;
        Ok(())
    }

    /// Trackers of a project.
    ///
    /// # Errors
    /// Database errors.
    pub fn trackers(&self, project_id: i64) -> Result<Vec<TrackerRow>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, engine, country, language, device, depth, location
             FROM trackers WHERE project_id = ? ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, u16>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        rows.map(|row| {
            let (id, project_id, engine, country, language, device, depth, location) = row?;
            Ok(TrackerRow {
                id,
                project_id,
                engine: engine.parse().map_err(|error: octopuson_serp::SerpError| {
                    SerpStoreError::Corrupt(error.to_string())
                })?,
                country,
                language,
                device: device.parse().map_err(|error: octopuson_serp::SerpError| {
                    SerpStoreError::Corrupt(error.to_string())
                })?,
                depth,
                location: location.as_deref().map(parse).transpose()?,
            })
        })
        .collect()
    }

    /// Adds keywords not yet tracked (compared by [`keyword_key`] in `language`). Returns how
    /// many were added.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_keywords(
        &self,
        project_id: i64,
        keywords: &[String],
        tags: &[String],
        language: &str,
        created_at: &str,
    ) -> Result<usize, SerpStoreError> {
        let tags = (!tags.is_empty()).then(|| json(tags)).transpose()?;
        let mut added = 0;
        for keyword in keywords {
            let keyword = keyword.trim();
            if keyword.is_empty() {
                continue;
            }
            added += self.connection.execute(
                "INSERT INTO keywords (project_id, keyword, keyword_key, tags, created_at)
                 VALUES (?, ?, ?, ?, ?) ON CONFLICT DO NOTHING",
                params![
                    project_id,
                    keyword,
                    keyword_key(keyword, language),
                    tags,
                    created_at
                ],
            )?;
        }
        Ok(added)
    }

    /// Removes a keyword and its rankings.
    ///
    /// # Errors
    /// Database errors.
    pub fn remove_keyword(&self, project_id: i64, keyword_id: i64) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "DELETE FROM rankings WHERE project_id = ? AND keyword_id = ?",
            params![project_id, keyword_id],
        )?;
        self.connection.execute(
            "DELETE FROM keywords WHERE project_id = ? AND id = ?",
            params![project_id, keyword_id],
        )?;
        Ok(())
    }

    /// Keywords of a project, in the order added.
    ///
    /// # Errors
    /// Database errors.
    pub fn keywords(&self, project_id: i64) -> Result<Vec<KeywordRow>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, keyword, tags, created_at FROM keywords WHERE project_id = ? ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, project_id, keyword, tags, created_at) = row?;
            Ok(KeywordRow {
                id,
                project_id,
                keyword,
                tags: tags.as_deref().map(parse).transpose()?.unwrap_or_default(),
                created_at,
            })
        })
        .collect()
    }

    /// Replaces a project's competitors.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_competitors(
        &mut self,
        project_id: i64,
        domains: &[String],
    ) -> Result<(), SerpStoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM competitors WHERE project_id = ?",
            params![project_id],
        )?;
        for domain in domains {
            transaction.execute(
                "INSERT INTO competitors VALUES (?, ?) ON CONFLICT DO NOTHING",
                params![project_id, domain.trim().to_ascii_lowercase()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// A project's competitors.
    ///
    /// # Errors
    /// Database errors.
    pub fn competitors(&self, project_id: i64) -> Result<Vec<String>, SerpStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT domain FROM competitors WHERE project_id = ? ORDER BY domain")?;
        Ok(statement
            .query_map(params![project_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Records a rank check.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_ranking(&self, ranking: &RankingRow) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "INSERT INTO rankings VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                ranking.run_id,
                ranking.project_id,
                ranking.keyword_id,
                ranking.tracker_id,
                ranking.snapshot_id,
                ranking.checked_at,
                ranking.position,
                ranking.url,
                (!ranking.positions.is_empty())
                    .then(|| json(&ranking.positions))
                    .transpose()?,
                ranking.error
            ],
        )?;
        Ok(())
    }

    /// Projects whose rank checks used a snapshot.
    ///
    /// # Errors
    /// Database errors.
    pub fn snapshot_projects(&self, snapshot_id: i64) -> Result<Vec<i64>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT project_id FROM rankings WHERE snapshot_id = ? ORDER BY project_id",
        )?;
        let rows = statement.query_map(params![snapshot_id], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Sets the position a project's rank check read from a snapshot, after the snapshot was
    /// read again.
    ///
    /// # Errors
    /// Database errors.
    pub fn update_ranking(
        &self,
        snapshot_id: i64,
        project_id: i64,
        position: Option<u16>,
        url: Option<&str>,
        positions: &[u16],
    ) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "UPDATE rankings SET position = ?, url = ?, positions = ?
             WHERE snapshot_id = ? AND project_id = ?",
            params![
                position,
                url,
                (!positions.is_empty())
                    .then(|| json(&positions))
                    .transpose()?,
                snapshot_id,
                project_id,
            ],
        )?;
        Ok(())
    }

    /// Every rank check of a project, oldest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn rankings(&self, project_id: i64) -> Result<Vec<RankingRow>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT run_id, project_id, keyword_id, tracker_id, snapshot_id, checked_at, position, url,
                    positions, error
             FROM rankings WHERE project_id = ? ORDER BY checked_at, keyword_id, tracker_id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((
                RankingRow {
                    run_id: row.get(0)?,
                    project_id: row.get(1)?,
                    keyword_id: row.get(2)?,
                    tracker_id: row.get(3)?,
                    snapshot_id: row.get(4)?,
                    checked_at: row.get(5)?,
                    position: row.get(6)?,
                    url: row.get(7)?,
                    positions: Vec::new(),
                    error: row.get(9)?,
                },
                row.get::<_, Option<String>>(8)?,
            ))
        })?;
        rows.map(|row| {
            let (mut ranking, positions) = row?;
            ranking.positions = positions
                .as_deref()
                .map(parse)
                .transpose()?
                .unwrap_or_default();
            Ok(ranking)
        })
        .collect()
    }

    /// Caches a keyword's monthly searches.
    ///
    /// # Errors
    /// Database errors.
    pub fn put_volume(
        &self,
        key: &str,
        engine: Engine,
        country: &str,
        volume: Option<u64>,
        source: &str,
        fetched_at: &str,
    ) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO keyword_volumes VALUES (?, ?, ?, ?, ?, ?)",
            params![key, engine.as_str(), country, volume, source, fetched_at],
        )?;
        Ok(())
    }

    /// A cached volume: the value and when it was fetched.
    ///
    /// # Errors
    /// Database errors.
    pub fn volume(
        &self,
        key: &str,
        engine: Engine,
        country: &str,
    ) -> Result<Option<(Option<u64>, String)>, SerpStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT volume, fetched_at FROM keyword_volumes WHERE keyword_key = ? AND engine = ? AND country = ?",
                params![key, engine.as_str(), country],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    /// Organic results of a snapshot as (position, domain), for competitor shares.
    ///
    /// # Errors
    /// Database errors.
    pub fn snapshot_domains(&self, snapshot_id: i64) -> Result<Vec<(u16, String)>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT position, domain FROM results WHERE snapshot_id = ? ORDER BY position",
        )?;
        Ok(statement
            .query_map(params![snapshot_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    }
}
