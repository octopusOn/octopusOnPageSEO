//! Local SEO: the business a project tracks on Google Maps, the keywords and grid it is checked
//! with, every grid run with what each point showed, its reviews, its profile's rating day by
//! day, and the tasks still waiting in the provider's queue.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::rank::RunEnd;
use crate::{PlatformStore, PlatformStoreError, list_json, parse_list};

/// Most keywords a project's grid is checked with.
pub const MAX_LOCAL_KEYWORDS: usize = 20;

/// Longest keyword.
const MAX_KEYWORD: usize = 200;

/// Points a side of a grid may have.
const GRIDS: &[u32] = &[3, 5, 7, 9];

/// Schedules of a project's grid.
const SCHEDULES: &[&str] = &["weekly", "monthly", "manual"];

/// How a project's business is checked on Google Maps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalSettings {
    /// The business, as the provider described it (title, ids, coordinates …); none chosen yet
    /// when `None`.
    pub business: Option<Value>,
    /// Keywords the grid is checked with.
    pub keywords: Vec<String>,
    /// Points a side: 3, 5, 7 or 9.
    pub grid: u32,
    /// From the business to the grid's edge, kilometres.
    pub radius_km: f64,
    /// `weekly`, `monthly` or `manual`.
    pub schedule: String,
    /// When the last run started, RFC 3339.
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Default for LocalSettings {
    /// Checked only when asked until a schedule is picked: nothing is paid on its own.
    fn default() -> Self {
        Self {
            business: None,
            keywords: Vec::new(),
            grid: 5,
            radius_km: 2.0,
            schedule: "manual".to_owned(),
            last_run_at: None,
        }
    }
}

/// One grid run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalRunRow {
    /// Run id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// Started by the schedule rather than by hand.
    pub scheduled: bool,
    /// `running`, `done`, `partial`, `failed`, `cancelled` or `interrupted`.
    pub status: String,
    /// Points planned (keywords × points).
    pub total: u32,
    /// Points read.
    pub checked: u32,
    /// Points that failed.
    pub failed: u32,
    /// US dollars paid.
    pub cost_usd: f64,
    /// Why it stopped early.
    pub error: Option<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub finished_at: Option<String>,
    /// Points a side.
    pub grid: u32,
    /// From the centre to the edge, kilometres.
    pub radius_km: f64,
    /// The centre's latitude.
    pub lat: f64,
    /// The centre's longitude.
    pub lng: f64,
    /// Points still waiting in the provider's queue (on the copy that posted them).
    #[serde(default)]
    pub waiting: u32,
}

/// What one point of a run showed for one keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalPointRow {
    /// The run.
    pub run_id: i64,
    /// The project.
    pub project_id: i64,
    /// The keyword.
    pub keyword: String,
    /// Row, from the north.
    pub row: u32,
    /// Column, from the west.
    pub col: u32,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// The business's place on the Maps page; `None` when it was not among the results.
    pub position: Option<u16>,
    /// The first three businesses (JSON list).
    pub top: Value,
    /// Why the point failed.
    pub error: Option<String>,
    /// RFC 3339.
    pub checked_at: String,
}

/// A task waiting in the provider's standard queue: a grid point or a page of reviews.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalTaskRow {
    /// Row id (0 for a new one).
    pub id: i64,
    /// The project.
    pub project_id: i64,
    /// `grid` or `reviews`.
    pub kind: String,
    /// The grid run, for a point.
    pub run_id: Option<i64>,
    /// The keyword, for a point.
    pub keyword: Option<String>,
    /// Row of a point.
    pub row: Option<u32>,
    /// Column of a point.
    pub col: Option<u32>,
    /// Latitude of a point.
    pub lat: Option<f64>,
    /// Longitude of a point.
    pub lng: Option<f64>,
    /// The provider's id of the task.
    pub task_id: String,
    /// How it was paid (`own` or `credit`): its answer is read the same way.
    pub via: String,
    /// RFC 3339.
    pub posted_at: String,
    /// Times its answer was asked for.
    pub tries: u32,
    /// When to ask next, seconds since the Unix epoch.
    pub next_try_at: i64,
}

/// A review of the business.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalReviewRow {
    /// Google's id of the review.
    pub review_id: String,
    /// 1–5 stars.
    pub rating: Option<u8>,
    /// What the reviewer wrote.
    pub text: Option<String>,
    /// The reviewer's name.
    pub author: Option<String>,
    /// The reviewer's profile.
    pub author_url: Option<String>,
    /// The review on Google.
    pub url: Option<String>,
    /// RFC 3339.
    pub published_at: Option<String>,
    /// The business's answer.
    pub owner_answer: Option<String>,
    /// RFC 3339.
    pub answered_at: Option<String>,
    /// Photos with it.
    pub photos: u32,
    /// The reviewer is a Local Guide.
    pub local_guide: bool,
    /// When it was first read here, RFC 3339.
    pub first_seen: String,
}

/// The business's rating on a day, and its profile when one was read that day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalProfileRow {
    /// `YYYY-MM-DD`.
    pub day: String,
    /// Average stars.
    pub rating: Option<f64>,
    /// Reviews counted in it.
    pub votes: Option<u64>,
    /// The profile as the provider described it.
    pub profile: Option<Value>,
    /// RFC 3339.
    pub taken_at: String,
}

fn invalid(message: impl Into<String>) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.into())
}

fn json_value(text: Option<String>) -> Result<Option<Value>, PlatformStoreError> {
    text.map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))
}

const RUN_COLUMNS: &str = "id, project_id, scheduled, status, total, checked, failed, cost_usd, error, started_at, finished_at,
    grid, radius_km, lat, lng, (SELECT count(*) FROM local_tasks q WHERE q.run_id = local_runs.id)::UINTEGER";

fn run(row: &Row<'_>) -> duckdb::Result<LocalRunRow> {
    Ok(LocalRunRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        scheduled: row.get(2)?,
        status: row.get(3)?,
        total: row.get(4)?,
        checked: row.get(5)?,
        failed: row.get(6)?,
        cost_usd: row.get(7)?,
        error: row.get(8)?,
        started_at: row.get(9)?,
        finished_at: row.get(10)?,
        grid: row.get(11)?,
        radius_km: row.get(12)?,
        lat: row.get(13)?,
        lng: row.get(14)?,
        waiting: row.get(15)?,
    })
}

const TASK_COLUMNS: &str = "id, project_id, kind, run_id, keyword, grid_row, grid_col, lat, lng, task_id, via, posted_at, tries, next_try_at";

fn task(row: &Row<'_>) -> duckdb::Result<LocalTaskRow> {
    Ok(LocalTaskRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        kind: row.get(2)?,
        run_id: row.get(3)?,
        keyword: row.get(4)?,
        row: row.get(5)?,
        col: row.get(6)?,
        lat: row.get(7)?,
        lng: row.get(8)?,
        task_id: row.get(9)?,
        via: row.get(10)?,
        posted_at: row.get(11)?,
        tries: row.get(12)?,
        next_try_at: row.get(13)?,
    })
}

const REVIEW_COLUMNS: &str = "review_id, rating, text, author, author_url, url, published_at, owner_answer, answered_at, photos, local_guide, first_seen";

fn review(row: &Row<'_>) -> duckdb::Result<LocalReviewRow> {
    Ok(LocalReviewRow {
        review_id: row.get(0)?,
        rating: row.get(1)?,
        text: row.get(2)?,
        author: row.get(3)?,
        author_url: row.get(4)?,
        url: row.get(5)?,
        published_at: row.get(6)?,
        owner_answer: row.get(7)?,
        answered_at: row.get(8)?,
        photos: row.get(9)?,
        local_guide: row.get(10)?,
        first_seen: row.get(11)?,
    })
}

/// Keywords with their spacing cleaned, without repeats.
fn clean_keywords(keywords: &[String]) -> Result<Vec<String>, PlatformStoreError> {
    let mut clean: Vec<String> = Vec::new();
    for keyword in keywords {
        let keyword = keyword.split_whitespace().collect::<Vec<_>>().join(" ");
        if keyword.is_empty()
            || clean
                .iter()
                .any(|known| known.to_lowercase() == keyword.to_lowercase())
        {
            continue;
        }
        if keyword.chars().count() > MAX_KEYWORD {
            return Err(invalid("keywords: at most 200 characters each"));
        }
        clean.push(keyword);
    }
    if clean.len() > MAX_LOCAL_KEYWORDS {
        return Err(invalid(format!(
            "keywords: at most {MAX_LOCAL_KEYWORDS} for the grid"
        )));
    }
    Ok(clean)
}

impl PlatformStore {
    /// How a project's business is checked; the defaults until saved.
    ///
    /// # Errors
    /// Database errors, or a stored business or keyword list that is not JSON.
    pub fn local_settings(&self, project_id: i64) -> Result<LocalSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT business, keywords, grid, radius_km, schedule, last_run_at FROM local_settings
             WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        let Some(found) = rows.next().transpose()? else {
            return Ok(LocalSettings::default());
        };
        let (business, keywords, grid, radius_km, schedule, last_run_at) = found;
        Ok(LocalSettings {
            business: json_value(business)?,
            keywords: parse_list(&keywords)?,
            grid,
            radius_km,
            schedule,
            last_run_at,
        })
    }

    /// Saves how a project's business is checked; the last run time stays.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown grid size, radius or schedule or too many keywords, or database errors.
    pub fn save_local_settings(
        &self,
        project_id: i64,
        settings: &LocalSettings,
    ) -> Result<LocalSettings, PlatformStoreError> {
        self.project(project_id)?;
        if !GRIDS.contains(&settings.grid) {
            return Err(invalid("grid: 3, 5, 7 or 9 points a side"));
        }
        if !(0.2..=50.0).contains(&settings.radius_km) {
            return Err(invalid("radius: 0.2–50 km"));
        }
        if !SCHEDULES.contains(&settings.schedule.as_str()) {
            return Err(invalid("schedule: weekly, monthly or manual"));
        }
        let keywords = clean_keywords(&settings.keywords)?;
        let business = settings.business.as_ref().map(Value::to_string);
        self.connection.execute(
            "INSERT INTO local_settings (project_id, business, keywords, grid, radius_km, schedule)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET business = excluded.business,
             keywords = excluded.keywords, grid = excluded.grid, radius_km = excluded.radius_km,
             schedule = excluded.schedule",
            params![
                project_id,
                business,
                list_json(&keywords),
                settings.grid,
                settings.radius_km,
                settings.schedule
            ],
        )?;
        self.local_settings(project_id)
    }

    /// Records that a grid run of a project started at `at`, which moves its schedule on.
    ///
    /// # Errors
    /// Database errors.
    pub fn mark_local_run(&self, project_id: i64, at: &str) -> Result<(), PlatformStoreError> {
        let defaults = LocalSettings::default();
        self.connection.execute(
            "INSERT INTO local_settings (project_id, keywords, grid, radius_km, schedule, last_run_at)
             VALUES (?, '[]', ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET last_run_at = excluded.last_run_at",
            params![
                project_id,
                defaults.grid,
                defaults.radius_km,
                defaults.schedule,
                at
            ],
        )?;
        Ok(())
    }

    /// Every project's grid settings.
    ///
    /// # Errors
    /// Database errors.
    pub fn local_plans(&self) -> Result<Vec<(i64, LocalSettings)>, PlatformStoreError> {
        let ids: Vec<i64> = {
            let mut statement = self.connection.prepare(
                "SELECT s.project_id FROM local_settings s JOIN projects p ON p.id = s.project_id
                 ORDER BY s.project_id",
            )?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.into_iter()
            .map(|id| Ok((id, self.local_settings(id)?)))
            .collect()
    }

    /// Starts a grid run at `now`; a run of the project left running by a program that stopped
    /// becomes `interrupted`, unless points of it still wait in the queue.
    ///
    /// # Errors
    /// Database errors.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_local_run(
        &self,
        project_id: i64,
        scheduled: bool,
        total: u32,
        grid: u32,
        radius_km: f64,
        centre: (f64, f64),
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        self.connection.execute(
            "UPDATE local_runs SET status = 'interrupted', finished_at = ?
             WHERE project_id = ? AND status = 'running'
               AND id NOT IN (SELECT run_id FROM local_tasks WHERE run_id IS NOT NULL)",
            params![now, project_id],
        )?;
        Ok(self.connection.query_row(
            "INSERT INTO local_runs (project_id, scheduled, status, total, started_at, grid,
             radius_km, lat, lng) VALUES (?, ?, 'running', ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                project_id, scheduled, total, now, grid, radius_km, centre.0, centre.1
            ],
            |row| row.get(0),
        )?)
    }

    /// Notes what a run whose points wait in the queue has failed and paid so far.
    ///
    /// # Errors
    /// Database errors.
    pub fn queue_local_run(
        &self,
        run_id: i64,
        failed: u32,
        cost_usd: f64,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE local_runs SET failed = ?, cost_usd = ? WHERE id = ?",
            params![failed, cost_usd, run_id],
        )?;
        Ok(())
    }

    /// Records how a grid run ended at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_local_run(
        &self,
        run_id: i64,
        end: &RunEnd<'_>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE local_runs SET status = ?, checked = ?, failed = ?, cost_usd = ?, error = ?,
             finished_at = ? WHERE id = ?",
            params![
                end.status,
                end.checked,
                end.failed,
                end.cost_usd,
                end.error,
                now,
                run_id
            ],
        )?;
        Ok(())
    }

    /// A project's latest grid runs, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn local_runs(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<LocalRunRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM local_runs WHERE project_id = ?
             ORDER BY started_at DESC, id DESC LIMIT ?"
        ))?;
        let rows = statement.query_map(params![project_id, limit], run)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One grid run.
    ///
    /// # Errors
    /// Database errors.
    pub fn local_run(&self, run_id: i64) -> Result<Option<LocalRunRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM local_runs WHERE id = ?"
        ))?;
        let mut rows = statement.query_map(params![run_id], run)?;
        Ok(rows.next().transpose()?)
    }

    /// A run's points that were read and those that failed.
    ///
    /// # Errors
    /// Database errors.
    pub fn local_run_tally(&self, run_id: i64) -> Result<(u32, u32), PlatformStoreError> {
        Ok(self.connection.query_row(
            "SELECT count(*) FILTER (WHERE error IS NULL)::UINTEGER,
                    count(*) FILTER (WHERE error IS NOT NULL)::UINTEGER
             FROM local_points WHERE run_id = ?",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    /// Keeps what points showed.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_local_points(&self, points: &[LocalPointRow]) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for point in points {
            transaction.execute(
                "INSERT INTO local_points (run_id, project_id, keyword, grid_row, grid_col, lat,
                 lng, position, top, error, checked_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    point.run_id,
                    point.project_id,
                    point.keyword,
                    point.row,
                    point.col,
                    point.lat,
                    point.lng,
                    point.position,
                    point.top.to_string(),
                    point.error,
                    point.checked_at
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// What a run's points showed, keyword by keyword, row by row.
    ///
    /// # Errors
    /// Database errors, or stored lists that are not JSON.
    pub fn local_points(&self, run_id: i64) -> Result<Vec<LocalPointRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT run_id, project_id, keyword, grid_row, grid_col, lat, lng, position, top, error,
                    checked_at
             FROM local_points WHERE run_id = ? ORDER BY keyword, grid_row, grid_col",
        )?;
        let rows = statement.query_map(params![run_id], |row| {
            Ok((
                LocalPointRow {
                    run_id: row.get(0)?,
                    project_id: row.get(1)?,
                    keyword: row.get(2)?,
                    row: row.get(3)?,
                    col: row.get(4)?,
                    lat: row.get(5)?,
                    lng: row.get(6)?,
                    position: row.get(7)?,
                    top: Value::Null,
                    error: row.get(9)?,
                    checked_at: row.get(10)?,
                },
                row.get::<_, String>(8)?,
            ))
        })?;
        rows.map(|row| {
            let (mut point, top) = row?;
            point.top = serde_json::from_str(&top)
                .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))?;
            Ok(point)
        })
        .collect()
    }

    /// Keeps tasks posted to the provider's queue.
    ///
    /// # Errors
    /// Database errors.
    pub fn queue_local_tasks(&self, tasks: &[LocalTaskRow]) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for task in tasks {
            transaction.execute(
                "INSERT INTO local_tasks (project_id, kind, run_id, keyword, grid_row, grid_col,
                 lat, lng, task_id, via, posted_at, tries, next_try_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    task.project_id,
                    task.kind,
                    task.run_id,
                    task.keyword,
                    task.row,
                    task.col,
                    task.lat,
                    task.lng,
                    task.task_id,
                    task.via,
                    task.posted_at,
                    task.tries,
                    task.next_try_at
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Queued tasks whose answer may be asked for at `now`, those waiting longest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn due_local_tasks(
        &self,
        now: i64,
        limit: u32,
    ) -> Result<Vec<LocalTaskRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM local_tasks WHERE next_try_at <= ?
             ORDER BY next_try_at, id LIMIT ?"
        ))?;
        let rows = statement.query_map(params![now, limit], task)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Asks for a queued task's answer again at `next_try_at`.
    ///
    /// # Errors
    /// Database errors.
    pub fn delay_local_task(&self, id: i64, next_try_at: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE local_tasks SET tries = tries + 1, next_try_at = ? WHERE id = ?",
            params![next_try_at, id],
        )?;
        Ok(())
    }

    /// Forgets a queued task once its answer is read (or given up on).
    ///
    /// # Errors
    /// Database errors.
    pub fn drop_local_task(&self, id: i64) -> Result<(), PlatformStoreError> {
        self.connection
            .execute("DELETE FROM local_tasks WHERE id = ?", params![id])?;
        Ok(())
    }

    /// A project's tasks of a kind waiting in the queue.
    ///
    /// # Errors
    /// Database errors.
    pub fn waiting_local_tasks(
        &self,
        project_id: i64,
        kind: &str,
    ) -> Result<u32, PlatformStoreError> {
        Ok(self.connection.query_row(
            "SELECT count(*)::UINTEGER FROM local_tasks WHERE project_id = ? AND kind = ?",
            params![project_id, kind],
            |row| row.get(0),
        )?)
    }

    /// When the queue is next asked for an answer, seconds since the Unix epoch.
    ///
    /// # Errors
    /// Database errors.
    pub fn next_local_task_at(&self) -> Result<Option<i64>, PlatformStoreError> {
        Ok(self
            .connection
            .query_row("SELECT min(next_try_at) FROM local_tasks", [], |row| {
                row.get(0)
            })?)
    }

    /// Keeps reviews read at `now`: new ones are added, known ones take the newer answer and
    /// text. Returns how many are new.
    ///
    /// # Errors
    /// Database errors.
    pub fn save_local_reviews(
        &self,
        project_id: i64,
        reviews: &[LocalReviewRow],
        now: &str,
    ) -> Result<usize, PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut new = 0;
        for review in reviews {
            let known: i64 = transaction.query_row(
                "SELECT count(*) FROM local_reviews WHERE project_id = ? AND review_id = ?",
                params![project_id, review.review_id],
                |row| row.get(0),
            )?;
            if known > 0 {
                transaction.execute(
                    "UPDATE local_reviews SET rating = ?, text = ?, owner_answer = ?, answered_at = ?,
                     photos = ? WHERE project_id = ? AND review_id = ?",
                    params![
                        review.rating,
                        review.text,
                        review.owner_answer,
                        review.answered_at,
                        review.photos,
                        project_id,
                        review.review_id
                    ],
                )?;
                continue;
            }
            new += 1;
            transaction.execute(
                "INSERT INTO local_reviews (project_id, review_id, rating, text, author, author_url,
                 url, published_at, owner_answer, answered_at, photos, local_guide, first_seen)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    project_id,
                    review.review_id,
                    review.rating,
                    review.text,
                    review.author,
                    review.author_url,
                    review.url,
                    review.published_at,
                    review.owner_answer,
                    review.answered_at,
                    review.photos,
                    review.local_guide,
                    now
                ],
            )?;
        }
        transaction.commit()?;
        Ok(new)
    }

    /// A project's reviews, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn local_reviews(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<LocalReviewRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {REVIEW_COLUMNS} FROM local_reviews WHERE project_id = ?
             ORDER BY coalesce(published_at, first_seen) DESC, review_id LIMIT ?"
        ))?;
        let rows = statement.query_map(params![project_id, limit], review)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Notes the business's rating on `day` (`YYYY-MM-DD`), with its profile when one was read;
    /// a profile read before on that day stays when none is given.
    ///
    /// # Errors
    /// Database errors.
    pub fn note_local_profile(
        &self,
        project_id: i64,
        day: &str,
        rating: Option<f64>,
        votes: Option<u64>,
        profile: Option<&Value>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        let profile = profile.map(Value::to_string);
        let updated = self.connection.execute(
            "UPDATE local_profiles SET rating = coalesce(?, rating), votes = coalesce(?, votes),
             profile = coalesce(?, profile), taken_at = ? WHERE project_id = ? AND day = ?",
            params![rating, votes, profile, now, project_id, day],
        )?;
        if updated == 0 {
            self.connection.execute(
                "INSERT INTO local_profiles (project_id, day, rating, votes, profile, taken_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![project_id, day, rating, votes, profile, now],
            )?;
        }
        Ok(())
    }

    /// The business's rating day by day, oldest first.
    ///
    /// # Errors
    /// Database errors, or stored profiles that are not JSON.
    pub fn local_profiles(
        &self,
        project_id: i64,
    ) -> Result<Vec<LocalProfileRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT day, rating, votes, profile, taken_at FROM local_profiles WHERE project_id = ?
             ORDER BY day",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?,
                row.get::<_, Option<u64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (day, rating, votes, profile, taken_at) = row?;
            Ok(LocalProfileRow {
                day,
                rating,
                votes,
                profile: json_value(profile)?,
                taken_at,
            })
        })
        .collect()
    }

    /// Removes everything a project's local SEO holds.
    pub(crate) fn delete_local_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in [
            "local_tasks",
            "local_points",
            "local_runs",
            "local_reviews",
            "local_profiles",
            "local_settings",
        ] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
