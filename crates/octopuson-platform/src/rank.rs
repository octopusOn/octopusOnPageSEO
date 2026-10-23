//! Rank tracking: the keywords a project tracks, where (engine, country, device, city), how
//! often, every run and every check with what the result page showed, and the checks still
//! waiting in the provider's queue.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError, list_json, parse_list};

/// Most keywords a project tracks.
pub const MAX_TRACKED: usize = 5_000;

/// Most places (engine, country, device, city) a project is tracked in.
pub const MAX_TRACKERS: usize = 10;

/// Longest tracked keyword.
const MAX_KEYWORD: usize = 200;

/// Most tags of a keyword.
const MAX_TAGS: usize = 10;

/// Longest tag.
const MAX_TAG: usize = 40;

/// Engines a tracker may use.
const ENGINES: &[&str] = &["google", "bing", "yandex"];

/// Devices a tracker may use.
const DEVICES: &[&str] = &["desktop", "mobile"];

/// Schedules of a project.
const SCHEDULES: &[&str] = &["daily", "weekly", "manual"];

/// How an element type the site holds is noted among a check's element types: the list keeps
/// its shape on every copy, and copies that do not know the mark show it as one more element.
const OWN: &str = "own:";

/// How a project's keywords are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankSettings {
    /// `daily`, `weekly` or `manual`.
    pub schedule: String,
    /// Results looked at, 10–100.
    pub depth: u32,
    /// Ask Google for AI Overviews that load after the page.
    pub ai_overview: bool,
    /// When the last run started, RFC 3339.
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Default for RankSettings {
    /// Checked only when asked until the user picks a schedule: nothing is paid on its own.
    fn default() -> Self {
        Self {
            schedule: "manual".to_owned(),
            depth: 20,
            ai_overview: true,
            last_run_at: None,
        }
    }
}

/// Where a project's keywords are checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerSpec {
    /// `google`, `bing` or `yandex`.
    pub engine: String,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// `desktop` or `mobile`.
    pub device: String,
    /// A city or region, as the provider names it.
    #[serde(default)]
    pub location: Option<String>,
}

/// A stored tracker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerRow {
    /// Tracker id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// Engine.
    pub engine: String,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Device.
    pub device: String,
    /// City or region.
    pub location: Option<String>,
    /// RFC 3339.
    pub created_at: String,
}

/// A keyword a project tracks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedKeyword {
    /// Keyword id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// The keyword.
    pub keyword: String,
    /// The user's tags.
    pub tags: Vec<String>,
    /// Monthly searches in the project's market, when known.
    pub volume: Option<u64>,
    /// RFC 3339.
    pub added_at: String,
}

/// One run of a project's checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankRunRow {
    /// Run id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// Started by the schedule rather than by hand.
    pub scheduled: bool,
    /// `running`, `done`, `partial`, `failed`, `cancelled` or `interrupted`.
    pub status: String,
    /// Checks planned.
    pub total: u32,
    /// Checks that read a result page.
    pub checked: u32,
    /// Checks that failed.
    pub failed: u32,
    /// US dollars paid.
    pub cost_usd: f64,
    /// Why it stopped early.
    pub error: Option<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub finished_at: Option<String>,
    /// Checks still waiting in the provider's queue (on the copy that posted them).
    #[serde(default)]
    pub waiting: u32,
}

/// An organic result kept with a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    /// Organic position.
    pub position: u16,
    /// Host, without `www.`.
    pub domain: String,
    /// The page.
    pub url: String,
    /// Its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// One check of a keyword on a tracker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankCheckRow {
    /// The run.
    pub run_id: i64,
    /// The project.
    pub project_id: i64,
    /// The keyword.
    pub keyword_id: i64,
    /// The tracker.
    pub tracker_id: i64,
    /// RFC 3339.
    pub checked_at: String,
    /// Best organic position of the site.
    pub position: Option<u16>,
    /// Its page there.
    pub url: Option<String>,
    /// Every page of the site among the results, by position.
    pub pages: Vec<(u16, String)>,
    /// Element types on the page.
    pub features: Vec<String>,
    /// An AI Overview was shown.
    pub ai_overview: bool,
    /// It cited the site.
    pub ai_cited: bool,
    /// The site held the featured snippet.
    pub snippet: bool,
    /// Element types that showed the site.
    #[serde(default)]
    pub owned: Vec<String>,
    /// The first organic results.
    pub results: Vec<CheckResult>,
    /// Why the check failed.
    pub error: Option<String>,
}

/// A check waiting in the provider's standard queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankTaskRow {
    /// Row id (0 for a new one).
    pub id: i64,
    /// The run.
    pub run_id: i64,
    /// The project.
    pub project_id: i64,
    /// The keyword.
    pub keyword_id: i64,
    /// The tracker.
    pub tracker_id: i64,
    /// `google`, `bing` or `yandex`.
    pub engine: String,
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

/// A project's schedule with what it would check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankPlan {
    /// The project.
    pub project_id: i64,
    /// Its settings.
    pub settings: RankSettings,
    /// Keywords tracked.
    pub keywords: u64,
    /// Trackers.
    pub trackers: u64,
}

fn invalid(message: impl Into<String>) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.into())
}

fn json_of<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_owned())
}

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, PlatformStoreError> {
    serde_json::from_str(text).map_err(|error| PlatformStoreError::Corrupt(error.to_string()))
}

/// A keyword with its spacing cleaned.
fn clean_keyword(keyword: &str) -> Result<Option<String>, PlatformStoreError> {
    let keyword = keyword.split_whitespace().collect::<Vec<_>>().join(" ");
    if keyword.chars().count() > MAX_KEYWORD {
        return Err(invalid("keywords: at most 200 characters each"));
    }
    Ok((!keyword.is_empty()).then_some(keyword))
}

fn clean_tags(tags: &[String]) -> Result<Vec<String>, PlatformStoreError> {
    let mut clean: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() || clean.iter().any(|known| known == tag) {
            continue;
        }
        if tag.chars().count() > MAX_TAG {
            return Err(invalid("tags: at most 40 characters each"));
        }
        clean.push(tag.to_owned());
    }
    if clean.len() > MAX_TAGS {
        return Err(invalid("tags: at most 10"));
    }
    Ok(clean)
}

fn code(
    text: &str,
    lengths: std::ops::RangeInclusive<usize>,
    field: &str,
) -> Result<String, PlatformStoreError> {
    let code = text.trim().to_ascii_lowercase();
    if lengths.contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase()) {
        Ok(code)
    } else {
        Err(invalid(format!("{field}: a code such as tr")))
    }
}

const TRACKER_COLUMNS: &str =
    "id, project_id, engine, country, language, device, location, created_at";

fn tracker(row: &Row<'_>) -> duckdb::Result<TrackerRow> {
    Ok(TrackerRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        engine: row.get(2)?,
        country: row.get(3)?,
        language: row.get(4)?,
        device: row.get(5)?,
        location: row.get(6)?,
        created_at: row.get(7)?,
    })
}

const RUN_COLUMNS: &str = "id, project_id, scheduled, status, total, checked, failed, cost_usd, error, started_at, finished_at,
    (SELECT count(*) FROM rank_tasks q WHERE q.run_id = rank_runs.id)::UINTEGER";

fn run(row: &Row<'_>) -> duckdb::Result<RankRunRow> {
    Ok(RankRunRow {
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
        waiting: row.get(11)?,
    })
}

const TASK_COLUMNS: &str = "id, run_id, project_id, keyword_id, tracker_id, engine, task_id, via, posted_at, tries, next_try_at";

fn task(row: &Row<'_>) -> duckdb::Result<RankTaskRow> {
    Ok(RankTaskRow {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        keyword_id: row.get(3)?,
        tracker_id: row.get(4)?,
        engine: row.get(5)?,
        task_id: row.get(6)?,
        via: row.get(7)?,
        posted_at: row.get(8)?,
        tries: row.get(9)?,
        next_try_at: row.get(10)?,
    })
}

/// A check's element types with those the site holds marked.
fn features_json(check: &RankCheckRow) -> String {
    let mut features = check.features.clone();
    features.extend(check.owned.iter().map(|kind| format!("{OWN}{kind}")));
    list_json(&features)
}

/// Element types and those the site holds, from the stored list.
fn split_features(list: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut features = Vec::new();
    let mut owned = Vec::new();
    for entry in list {
        match entry.strip_prefix(OWN) {
            Some(kind) => owned.push(kind.to_owned()),
            None => features.push(entry),
        }
    }
    (features, owned)
}

/// How a run ended, for [`PlatformStore::finish_rank_run`].
#[derive(Debug, Clone, PartialEq)]
pub struct RunEnd<'a> {
    /// `done`, `partial`, `failed` or `cancelled`.
    pub status: &'a str,
    /// Checks that read a result page.
    pub checked: u32,
    /// Checks that failed.
    pub failed: u32,
    /// US dollars paid.
    pub cost_usd: f64,
    /// Why it stopped early.
    pub error: Option<&'a str>,
}

impl PlatformStore {
    /// How a project's keywords are checked; the defaults until saved.
    ///
    /// # Errors
    /// Database errors.
    pub fn rank_settings(&self, project_id: i64) -> Result<RankSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT schedule, depth, ai_overview, last_run_at FROM rank_settings WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok(RankSettings {
                schedule: row.get(0)?,
                depth: row.get(1)?,
                ai_overview: row.get(2)?,
                last_run_at: row.get(3)?,
            })
        })?;
        Ok(rows.next().transpose()?.unwrap_or_default())
    }

    /// Saves how a project's keywords are checked; the last run time stays.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown schedule or a depth outside 10–100, or database errors.
    pub fn save_rank_settings(
        &self,
        project_id: i64,
        settings: &RankSettings,
    ) -> Result<RankSettings, PlatformStoreError> {
        self.project(project_id)?;
        if !SCHEDULES.contains(&settings.schedule.as_str()) {
            return Err(invalid("schedule: daily, weekly or manual"));
        }
        if !(10..=100).contains(&settings.depth) {
            return Err(invalid("depth: 10–100 results"));
        }
        self.connection.execute(
            "INSERT INTO rank_settings (project_id, schedule, depth, ai_overview) VALUES (?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET schedule = excluded.schedule,
             depth = excluded.depth, ai_overview = excluded.ai_overview",
            params![
                project_id,
                settings.schedule,
                settings.depth,
                settings.ai_overview
            ],
        )?;
        self.rank_settings(project_id)
    }

    /// Records that a run of a project started at `at`, which moves its schedule on.
    ///
    /// # Errors
    /// Database errors.
    pub fn mark_rank_run(&self, project_id: i64, at: &str) -> Result<(), PlatformStoreError> {
        let defaults = RankSettings::default();
        self.connection.execute(
            "INSERT INTO rank_settings (project_id, schedule, depth, ai_overview, last_run_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET last_run_at = excluded.last_run_at",
            params![
                project_id,
                defaults.schedule,
                defaults.depth,
                defaults.ai_overview,
                at
            ],
        )?;
        Ok(())
    }

    /// Every project's settings with how many keywords and trackers it has.
    ///
    /// # Errors
    /// Database errors.
    pub fn rank_plans(&self) -> Result<Vec<RankPlan>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT p.id, s.schedule, s.depth, s.ai_overview, s.last_run_at,
                    (SELECT count(*) FROM tracked_keywords k WHERE k.project_id = p.id)::UBIGINT,
                    (SELECT count(*) FROM rank_trackers t WHERE t.project_id = p.id)::UBIGINT
             FROM projects p JOIN rank_settings s ON s.project_id = p.id ORDER BY p.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(RankPlan {
                project_id: row.get(0)?,
                settings: RankSettings {
                    schedule: row.get(1)?,
                    depth: row.get(2)?,
                    ai_overview: row.get(3)?,
                    last_run_at: row.get(4)?,
                },
                keywords: row.get(5)?,
                trackers: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The places a project is tracked in, oldest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn rank_trackers(&self, project_id: i64) -> Result<Vec<TrackerRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {TRACKER_COLUMNS} FROM rank_trackers WHERE project_id = ? ORDER BY created_at, id"
        ))?;
        let rows = statement.query_map(params![project_id], tracker)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Adds a place to track a project in at `now`.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown engine or device, a bad code, a place already tracked or one too many,
    /// or database errors.
    pub fn add_rank_tracker(
        &self,
        project_id: i64,
        spec: &TrackerSpec,
        now: &str,
    ) -> Result<TrackerRow, PlatformStoreError> {
        self.project(project_id)?;
        let engine = spec.engine.trim().to_ascii_lowercase();
        if !ENGINES.contains(&engine.as_str()) {
            return Err(invalid("engine: google, bing or yandex"));
        }
        let device = spec.device.trim().to_ascii_lowercase();
        if !DEVICES.contains(&device.as_str()) {
            return Err(invalid("device: desktop or mobile"));
        }
        let country = code(&spec.country, 2..=2, "country")?;
        let language = code(&spec.language, 2..=3, "language")?;
        let location = spec
            .location
            .as_deref()
            .map(str::trim)
            .filter(|location| !location.is_empty())
            .map(str::to_owned);
        let known = self.rank_trackers(project_id)?;
        if known.iter().any(|row| {
            row.engine == engine
                && row.country == country
                && row.language == language
                && row.device == device
                && row.location == location
        }) {
            return Err(invalid("this place is already tracked"));
        }
        if known.len() >= MAX_TRACKERS {
            return Err(invalid("trackers: at most 10 places per project"));
        }
        let id: i64 = self.connection.query_row(
            "INSERT INTO rank_trackers (project_id, engine, country, language, device, location, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![project_id, engine, country, language, device, location, now],
            |row| row.get(0),
        )?;
        self.rank_trackers(project_id)?
            .into_iter()
            .find(|row| row.id == id)
            .ok_or_else(|| PlatformStoreError::NotFound(format!("tracker {id}")))
    }

    /// Removes a tracker and its checks.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn remove_rank_tracker(
        &self,
        project_id: i64,
        tracker_id: i64,
    ) -> Result<(), PlatformStoreError> {
        for table in ["rank_checks", "rank_tasks"] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ? AND tracker_id = ?"),
                params![project_id, tracker_id],
            )?;
        }
        let removed = self.connection.execute(
            "DELETE FROM rank_trackers WHERE project_id = ? AND id = ?",
            params![project_id, tracker_id],
        )?;
        if removed == 0 {
            return Err(PlatformStoreError::NotFound(format!(
                "tracker {tracker_id}"
            )));
        }
        Ok(())
    }

    /// The keywords a project tracks, in the order they were added.
    ///
    /// # Errors
    /// Database errors.
    pub fn tracked_keywords(
        &self,
        project_id: i64,
    ) -> Result<Vec<TrackedKeyword>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, keyword, tags, volume, added_at FROM tracked_keywords
             WHERE project_id = ? ORDER BY added_at, id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((
                TrackedKeyword {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    keyword: row.get(2)?,
                    tags: Vec::new(),
                    volume: row.get(4)?,
                    added_at: row.get(5)?,
                },
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (mut keyword, tags) = row?;
            keyword.tags = parse_list(&tags)?;
            Ok(keyword)
        })
        .collect()
    }

    /// Tracks keywords (with their volume when known) at `now`, each with `tags`. A keyword
    /// already tracked gets the new tags and a volume it lacked. Returns how many are new.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for a long keyword or tag, or past [`MAX_TRACKED`] keywords, or database errors.
    pub fn track_keywords(
        &self,
        project_id: i64,
        keywords: &[(String, Option<u64>)],
        tags: &[String],
        now: &str,
    ) -> Result<usize, PlatformStoreError> {
        self.project(project_id)?;
        let tags = clean_tags(tags)?;
        let known = self.tracked_keywords(project_id)?;
        let mut clean: Vec<(String, Option<u64>)> = Vec::new();
        for (keyword, volume) in keywords {
            let Some(keyword) = clean_keyword(keyword)? else {
                continue;
            };
            match clean.iter_mut().find(|(seen, _)| *seen == keyword) {
                Some(entry) => entry.1 = entry.1.or(*volume),
                None => clean.push((keyword, *volume)),
            }
        }
        let new = clean
            .iter()
            .filter(|(keyword, _)| !known.iter().any(|row| &row.keyword == keyword))
            .count();
        if known.len() + new > MAX_TRACKED {
            return Err(invalid(format!(
                "keywords: a project tracks at most {MAX_TRACKED} keywords"
            )));
        }
        let transaction = self.connection.unchecked_transaction()?;
        for (keyword, volume) in &clean {
            match known.iter().find(|row| &row.keyword == keyword) {
                Some(row) => {
                    let mut merged = row.tags.clone();
                    merged.extend(tags.iter().filter(|tag| !row.tags.contains(tag)).cloned());
                    transaction.execute(
                        "UPDATE tracked_keywords SET tags = ?, volume = coalesce(volume, ?) WHERE id = ?",
                        params![list_json(&clean_tags(&merged)?), volume, row.id],
                    )?;
                }
                None => {
                    transaction.execute(
                        "INSERT INTO tracked_keywords (project_id, keyword, tags, volume, added_at)
                         VALUES (?, ?, ?, ?, ?)",
                        params![project_id, keyword, list_json(&tags), volume, now],
                    )?;
                }
            }
        }
        transaction.commit()?;
        Ok(new)
    }

    /// Stops tracking keywords and forgets their checks; returns how many are left.
    ///
    /// # Errors
    /// Database errors.
    pub fn untrack_keywords(
        &self,
        project_id: i64,
        keyword_ids: &[i64],
    ) -> Result<usize, PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for id in keyword_ids {
            for table in ["rank_checks", "rank_tasks"] {
                transaction.execute(
                    &format!("DELETE FROM {table} WHERE project_id = ? AND keyword_id = ?"),
                    params![project_id, id],
                )?;
            }
            transaction.execute(
                "DELETE FROM tracked_keywords WHERE project_id = ? AND id = ?",
                params![project_id, id],
            )?;
        }
        transaction.commit()?;
        Ok(self.tracked_keywords(project_id)?.len())
    }

    /// Replaces the tags of keywords.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for unusable tags, or database errors.
    pub fn set_keyword_tags(
        &self,
        project_id: i64,
        keyword_ids: &[i64],
        tags: &[String],
    ) -> Result<(), PlatformStoreError> {
        let tags = list_json(&clean_tags(tags)?);
        for id in keyword_ids {
            self.connection.execute(
                "UPDATE tracked_keywords SET tags = ? WHERE project_id = ? AND id = ?",
                params![tags, project_id, id],
            )?;
        }
        Ok(())
    }

    /// Sets the monthly searches of tracked keywords, by keyword.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_keyword_volumes(
        &self,
        project_id: i64,
        volumes: &[(String, Option<u64>)],
    ) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for (keyword, volume) in volumes {
            transaction.execute(
                "UPDATE tracked_keywords SET volume = ? WHERE project_id = ? AND keyword = ?",
                params![volume, project_id, keyword],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Starts a run of `total` checks at `now`; a run of the project left running by a program
    /// that stopped becomes `interrupted`, unless checks of it still wait in the queue.
    ///
    /// # Errors
    /// Database errors.
    pub fn begin_rank_run(
        &self,
        project_id: i64,
        scheduled: bool,
        total: u32,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        self.connection.execute(
            "UPDATE rank_runs SET status = 'interrupted', finished_at = ?
             WHERE project_id = ? AND status = 'running'
               AND id NOT IN (SELECT run_id FROM rank_tasks)",
            params![now, project_id],
        )?;
        Ok(self.connection.query_row(
            "INSERT INTO rank_runs (project_id, scheduled, status, total, started_at)
             VALUES (?, ?, 'running', ?, ?) RETURNING id",
            params![project_id, scheduled, total, now],
            |row| row.get(0),
        )?)
    }

    /// Records how a run ended at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_rank_run(
        &self,
        run_id: i64,
        end: &RunEnd<'_>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE rank_runs SET status = ?, checked = ?, failed = ?, cost_usd = ?, error = ?,
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

    /// Notes what a run whose checks wait in the queue has failed and paid so far; it keeps
    /// running until their answers are read.
    ///
    /// # Errors
    /// Database errors.
    pub fn queue_rank_run(
        &self,
        run_id: i64,
        failed: u32,
        cost_usd: f64,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE rank_runs SET failed = ?, cost_usd = ? WHERE id = ?",
            params![failed, cost_usd, run_id],
        )?;
        Ok(())
    }

    /// One run.
    ///
    /// # Errors
    /// Database errors.
    pub fn rank_run(&self, run_id: i64) -> Result<Option<RankRunRow>, PlatformStoreError> {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT {RUN_COLUMNS} FROM rank_runs WHERE id = ?"))?;
        let mut rows = statement.query_map(params![run_id], run)?;
        Ok(rows.next().transpose()?)
    }

    /// A run's checks that read a page and those that failed.
    ///
    /// # Errors
    /// Database errors.
    pub fn rank_run_tally(&self, run_id: i64) -> Result<(u32, u32), PlatformStoreError> {
        Ok(self.connection.query_row(
            "SELECT count(*) FILTER (WHERE error IS NULL)::UINTEGER,
                    count(*) FILTER (WHERE error IS NOT NULL)::UINTEGER
             FROM rank_checks WHERE run_id = ?",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    /// Keeps checks posted to the provider's queue.
    ///
    /// # Errors
    /// Database errors.
    pub fn queue_rank_tasks(&self, tasks: &[RankTaskRow]) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for task in tasks {
            transaction.execute(
                "INSERT INTO rank_tasks (run_id, project_id, keyword_id, tracker_id, engine, task_id,
                 via, posted_at, tries, next_try_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    task.run_id,
                    task.project_id,
                    task.keyword_id,
                    task.tracker_id,
                    task.engine,
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

    /// Queued checks whose answer may be asked for at `now` (seconds since the Unix epoch),
    /// those waiting longest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn due_rank_tasks(
        &self,
        now: i64,
        limit: u32,
    ) -> Result<Vec<RankTaskRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM rank_tasks WHERE next_try_at <= ?
             ORDER BY next_try_at, id LIMIT ?"
        ))?;
        let rows = statement.query_map(params![now, limit], task)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Asks for a queued check's answer again at `next_try_at`.
    ///
    /// # Errors
    /// Database errors.
    pub fn delay_rank_task(&self, id: i64, next_try_at: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE rank_tasks SET tries = tries + 1, next_try_at = ? WHERE id = ?",
            params![next_try_at, id],
        )?;
        Ok(())
    }

    /// Forgets a queued check once its answer is read (or given up on).
    ///
    /// # Errors
    /// Database errors.
    pub fn drop_rank_task(&self, id: i64) -> Result<(), PlatformStoreError> {
        self.connection
            .execute("DELETE FROM rank_tasks WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Checks of a project waiting in the queue.
    ///
    /// # Errors
    /// Database errors.
    pub fn waiting_rank_tasks(&self, project_id: i64) -> Result<u32, PlatformStoreError> {
        Ok(self.connection.query_row(
            "SELECT count(*)::UINTEGER FROM rank_tasks WHERE project_id = ?",
            params![project_id],
            |row| row.get(0),
        )?)
    }

    /// When the queue is next asked for an answer, seconds since the Unix epoch.
    ///
    /// # Errors
    /// Database errors.
    pub fn next_rank_task_at(&self) -> Result<Option<i64>, PlatformStoreError> {
        Ok(self
            .connection
            .query_row("SELECT min(next_try_at) FROM rank_tasks", [], |row| {
                row.get(0)
            })?)
    }

    /// A project's latest runs, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn rank_runs(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<RankRunRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM rank_runs WHERE project_id = ? ORDER BY started_at DESC, id DESC LIMIT ?"
        ))?;
        let rows = statement.query_map(params![project_id, limit], run)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Keeps a check.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_rank_check(&self, check: &RankCheckRow) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "INSERT INTO rank_checks (run_id, project_id, keyword_id, tracker_id, checked_at,
             position, url, pages, features, ai_overview, ai_cited, snippet, results, error)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                check.run_id,
                check.project_id,
                check.keyword_id,
                check.tracker_id,
                check.checked_at,
                check.position,
                check.url,
                json_of(&check.pages),
                features_json(check),
                check.ai_overview,
                check.ai_cited,
                check.snippet,
                json_of(&check.results),
                check.error
            ],
        )?;
        Ok(())
    }

    /// The checks of a project's tracker, oldest first: of one keyword, and since a time
    /// (RFC 3339), when given.
    ///
    /// # Errors
    /// Database errors, or stored lists that are not JSON.
    pub fn rank_checks(
        &self,
        project_id: i64,
        tracker_id: i64,
        keyword_id: Option<i64>,
        since: Option<&str>,
    ) -> Result<Vec<RankCheckRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT run_id, project_id, keyword_id, tracker_id, checked_at, position, url, pages,
                    features, ai_overview, ai_cited, snippet, results, error
             FROM rank_checks
             WHERE project_id = ? AND tracker_id = ? AND (?::BIGINT IS NULL OR keyword_id = ?)
               AND (?::VARCHAR IS NULL OR checked_at >= ?)
             ORDER BY checked_at, run_id",
        )?;
        let rows = statement.query_map(
            params![project_id, tracker_id, keyword_id, keyword_id, since, since],
            |row| {
                Ok((
                    RankCheckRow {
                        run_id: row.get(0)?,
                        project_id: row.get(1)?,
                        keyword_id: row.get(2)?,
                        tracker_id: row.get(3)?,
                        checked_at: row.get(4)?,
                        position: row.get(5)?,
                        url: row.get(6)?,
                        pages: Vec::new(),
                        features: Vec::new(),
                        ai_overview: row.get(9)?,
                        ai_cited: row.get(10)?,
                        snippet: row.get(11)?,
                        owned: Vec::new(),
                        results: Vec::new(),
                        error: row.get(13)?,
                    },
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(12)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (mut check, pages, features, results) = row?;
            check.pages = parse(&pages)?;
            (check.features, check.owned) = split_features(parse_list(&features)?);
            check.results = parse(&results)?;
            Ok(check)
        })
        .collect()
    }

    /// Removes everything a project's rank tracking holds.
    pub(crate) fn delete_rank_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in [
            "rank_tasks",
            "rank_checks",
            "rank_runs",
            "tracked_keywords",
            "rank_trackers",
            "rank_settings",
        ] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
