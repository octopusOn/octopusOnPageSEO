//! Rank tracking: where a site ranks for its keywords, over time.

use std::collections::{BTreeMap, HashMap};

pub use octopuson_serp_score::rank::{CompetitorShare, Schedule, VisibilityPoint};

use octopuson_core::Timestamp;
use octopuson_serp::provider::{ProviderError, SearchEvent};
use octopuson_serp::{Device, Engine, Location, SerpQuery, Target, keyword_key};
use octopuson_serp_score::rank::{
    Check, ResultPage, competitor_shares as shares_over, seconds_of, trend, visibility,
};
use octopuson_serp_store::{KeywordRow, ProjectRow, RankingRow, RunKind, RunStatus, TrackerRow};
use serde::{Deserialize, Serialize};

use crate::search::now;
use crate::{SearchRequest, SerpAppError, SerpWorkspace};

/// Where keywords are tracked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerSpec {
    /// Engine.
    pub engine: Engine,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Device.
    #[serde(default)]
    pub device: Device,
    /// Depth.
    pub depth: u16,
    /// City or region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

/// A new project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSpec {
    /// Name.
    pub name: String,
    /// The tracked site.
    pub target: Target,
    /// How often it is checked.
    #[serde(default)]
    pub schedule: Schedule,
    /// Where.
    pub trackers: Vec<TrackerSpec>,
    /// Providers to use instead of the engines' chains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<Vec<String>>,
}

/// A project with what belongs to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// Id.
    pub id: i64,
    /// Name.
    pub name: String,
    /// The tracked site.
    pub target: Target,
    /// Schedule.
    pub schedule: Schedule,
    /// Providers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub providers: Option<Vec<String>>,
    /// Trackers.
    pub trackers: Vec<TrackerRow>,
    /// Number of keywords.
    pub keywords: usize,
    /// Competitors.
    pub competitors: Vec<String>,
    /// When it was created.
    pub created_at: String,
    /// When it was last checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<String>,
}

/// Expected cost of checking a project.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackEstimate {
    /// Searches (keywords × trackers).
    pub searches: u32,
    /// Requests, with the first ready provider of each chain.
    pub requests: u32,
    /// US dollars.
    pub usd: f64,
}

/// Progress of a rank check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TrackEvent {
    /// A keyword is being searched.
    Keyword {
        /// Searches done.
        done: u32,
        /// Searches planned.
        total: u32,
        /// The keyword.
        keyword: String,
        /// Engine.
        engine: Engine,
    },
    /// What the provider reported.
    Search {
        /// The event.
        event: SearchEvent,
    },
}

/// How a rank check went.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackSummary {
    /// Project.
    pub project_id: i64,
    /// Run.
    pub run_id: i64,
    /// Searches that returned results.
    pub checked: u32,
    /// Searches that failed.
    pub failed: u32,
    /// How it ended.
    pub status: RunStatus,
    /// US dollars spent.
    pub cost_usd: f64,
}

/// Position of one keyword on one tracker, with its history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankRow {
    /// Keyword id.
    pub keyword_id: i64,
    /// Keyword.
    pub keyword: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Tracker id.
    pub tracker_id: i64,
    /// Latest position.
    pub position: Option<u16>,
    /// Position of the check before.
    pub previous: Option<u16>,
    /// Places gained since then (positive is better); `None` without two positions.
    pub change: Option<i32>,
    /// Best position ever.
    pub best: Option<u16>,
    /// URL that ranks.
    pub url: Option<String>,
    /// More than one URL of the site ranks (cannibalisation).
    pub cannibalised: bool,
    /// When it was last checked.
    pub checked_at: Option<String>,
    /// Daily positions (last check of each day), oldest first.
    pub history: Vec<(String, Option<u16>)>,
    /// Monthly searches, if known.
    pub volume: Option<u64>,
    /// Why the last check failed.
    pub error: Option<String>,
}

/// The rank report of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankReport {
    /// The project.
    pub project: Project,
    /// One row per keyword and tracker.
    pub rows: Vec<RankRow>,
    /// Visibility by day.
    pub visibility: Vec<VisibilityPoint>,
    /// The project's site and its competitors (listed, else the most frequent domains).
    pub competitors: Vec<CompetitorShare>,
}

impl SerpWorkspace {
    fn project_of(&self, row: ProjectRow) -> Result<Project, SerpAppError> {
        let store = self.store()?;
        Ok(Project {
            trackers: store.trackers(row.id)?,
            keywords: store.keywords(row.id)?.len(),
            competitors: store.competitors(row.id)?,
            id: row.id,
            name: row.name,
            target: row.target,
            schedule: Schedule::parse(&row.schedule),
            providers: row.providers,
            created_at: row.created_at,
            last_run_at: row.last_run_at,
        })
    }

    /// Creates a project.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidQuery`] for an empty name, no tracker or an invalid tracker.
    pub fn create_project(&self, spec: &ProjectSpec) -> Result<Project, SerpAppError> {
        if spec.name.trim().is_empty() {
            return Err(SerpAppError::InvalidQuery(
                "the project needs a name".to_owned(),
            ));
        }
        if spec.trackers.is_empty() {
            return Err(SerpAppError::InvalidQuery(
                "track at least one engine and locale".to_owned(),
            ));
        }
        for tracker in &spec.trackers {
            let query = SerpQuery {
                engine: tracker.engine,
                keyword: "x".to_owned(),
                country: tracker.country.clone(),
                language: tracker.language.clone(),
                location: tracker.location.clone(),
                device: tracker.device,
                depth: tracker.depth,
            };
            query
                .validate()
                .map_err(|error| SerpAppError::InvalidQuery(error.to_string()))?;
        }
        let id = {
            let store = self.store()?;
            let id = store.create_project(&ProjectRow {
                id: 0,
                name: spec.name.trim().to_owned(),
                target: spec.target.clone(),
                schedule: spec.schedule.as_str().to_owned(),
                providers: spec.providers.clone(),
                created_at: now(),
                last_run_at: None,
            })?;
            for tracker in &spec.trackers {
                store.add_tracker(&TrackerRow {
                    id: 0,
                    project_id: id,
                    engine: tracker.engine,
                    country: tracker.country.clone(),
                    language: tracker.language.clone(),
                    device: tracker.device,
                    depth: tracker.depth,
                    location: tracker.location.clone(),
                })?;
            }
            id
        };
        self.project(id)
    }

    /// Every project.
    ///
    /// # Errors
    /// Store errors.
    pub fn projects(&self) -> Result<Vec<Project>, SerpAppError> {
        let rows = self.store()?.projects()?;
        rows.into_iter().map(|row| self.project_of(row)).collect()
    }

    /// A project.
    ///
    /// # Errors
    /// Store errors, an unknown id.
    pub fn project(&self, project_id: i64) -> Result<Project, SerpAppError> {
        let row = self.store()?.project(project_id)?;
        self.project_of(row)
    }

    /// Changes a project's name, target, schedule and providers.
    ///
    /// # Errors
    /// Store errors, an unknown id.
    pub fn update_project(&self, project: &Project) -> Result<(), SerpAppError> {
        Ok(self.store()?.update_project(&ProjectRow {
            id: project.id,
            name: project.name.clone(),
            target: project.target.clone(),
            schedule: project.schedule.as_str().to_owned(),
            providers: project.providers.clone(),
            created_at: project.created_at.clone(),
            last_run_at: project.last_run_at.clone(),
        })?)
    }

    /// Deletes a project and its rank history.
    ///
    /// # Errors
    /// Store errors, an unknown id.
    pub fn delete_project(&self, project_id: i64) -> Result<(), SerpAppError> {
        Ok(self.store()?.delete_project(project_id)?)
    }

    /// Adds keywords (one per entry; duplicates are skipped). Returns how many were added.
    ///
    /// # Errors
    /// Store errors.
    pub fn add_keywords(
        &self,
        project_id: i64,
        keywords: &[String],
        tags: &[String],
    ) -> Result<usize, SerpAppError> {
        let project = self.project(project_id)?;
        let language = project
            .trackers
            .first()
            .map_or_else(|| "tr".to_owned(), |tracker| tracker.language.clone());
        Ok(self
            .store()?
            .add_keywords(project_id, keywords, tags, &language, &now())?)
    }

    /// Keywords of a project.
    ///
    /// # Errors
    /// Store errors.
    pub fn keywords(&self, project_id: i64) -> Result<Vec<KeywordRow>, SerpAppError> {
        Ok(self.store()?.keywords(project_id)?)
    }

    /// Removes a keyword and its history.
    ///
    /// # Errors
    /// Store errors.
    pub fn remove_keyword(&self, project_id: i64, keyword_id: i64) -> Result<(), SerpAppError> {
        Ok(self.store()?.remove_keyword(project_id, keyword_id)?)
    }

    /// Replaces a project's competitors (registrable domains).
    ///
    /// # Errors
    /// Store errors.
    pub fn set_competitors(&self, project_id: i64, domains: &[String]) -> Result<(), SerpAppError> {
        let domains: Vec<String> = domains
            .iter()
            .filter(|domain| !domain.trim().is_empty())
            .map(|domain| octopuson_serp::registrable_domain(&domain.trim().to_ascii_lowercase()))
            .collect();
        Ok(self.store()?.set_competitors(project_id, &domains)?)
    }

    fn requests_of(project: &Project, keyword: &str, tracker: &TrackerRow) -> SearchRequest {
        SearchRequest {
            query: SerpQuery {
                engine: tracker.engine,
                keyword: keyword.to_owned(),
                country: tracker.country.clone(),
                language: tracker.language.clone(),
                location: tracker.location.clone(),
                device: tracker.device,
                depth: tracker.depth,
            },
            providers: project.providers.clone(),
            target: Some(project.target.clone()),
        }
    }

    /// Expected cost of checking a project once.
    ///
    /// # Errors
    /// Store and settings errors.
    pub fn estimate_track(&self, project_id: i64) -> Result<TrackEstimate, SerpAppError> {
        let project = self.project(project_id)?;
        let keywords = u32::try_from(project.keywords).unwrap_or(u32::MAX);
        let mut estimate = TrackEstimate {
            searches: 0,
            requests: 0,
            usd: 0.0,
        };
        for tracker in &project.trackers {
            let request = Self::requests_of(&project, "x", tracker);
            let first = self
                .estimate(&request)?
                .into_iter()
                .find(|estimate| estimate.ready)
                .map(|estimate| estimate.estimate)
                .unwrap_or_default();
            estimate.searches += keywords;
            estimate.requests += first.requests * keywords;
            estimate.usd += first.usd * f64::from(keywords);
        }
        Ok(estimate)
    }

    /// Checks every keyword of a project on every tracker.
    ///
    /// A check page that the person does not clear stops the run, so a blocked engine is not
    /// asked again and again.
    ///
    /// # Errors
    /// Store and settings errors; failed searches are recorded per keyword.
    pub fn track(
        &self,
        project_id: i64,
        cancelled: &dyn Fn() -> bool,
        events: &dyn Fn(&TrackEvent),
    ) -> Result<TrackSummary, SerpAppError> {
        let project = self.project(project_id)?;
        let keywords = self.keywords(project_id)?;
        let total = u32::try_from(keywords.len() * project.trackers.len()).unwrap_or(u32::MAX);
        let estimate = self.estimate_track(project_id)?;
        let run = self.store()?.begin_run(
            RunKind::Rank,
            Some(project_id),
            total,
            estimate.usd,
            &now(),
        )?;
        let settings = self.settings()?;
        let transport = self.transport(&settings)?;
        let mut summary = TrackSummary {
            project_id,
            run_id: run,
            checked: 0,
            failed: 0,
            status: RunStatus::Done,
            cost_usd: 0.0,
        };
        let mut done = 0;
        'trackers: for tracker in &project.trackers {
            for keyword in &keywords {
                if cancelled() {
                    summary.status = RunStatus::Cancelled;
                    break 'trackers;
                }
                events(&TrackEvent::Keyword {
                    done,
                    total,
                    keyword: keyword.keyword.clone(),
                    engine: tracker.engine,
                });
                let request = Self::requests_of(&project, &keyword.keyword, tracker);
                let forward = |event: &SearchEvent| {
                    events(&TrackEvent::Search {
                        event: event.clone(),
                    });
                };
                let result = self.search_in(
                    &request,
                    &transport,
                    cancelled,
                    &forward,
                    Some((run, keyword.id)),
                );
                done += 1;
                let (ranking, checked) =
                    ranking_of(run, project_id, keyword.id, tracker.id, result)?;
                self.store()?.add_ranking(&ranking)?;
                match checked {
                    Checked::Found(cost) => {
                        summary.checked += 1;
                        summary.cost_usd += cost;
                    }
                    Checked::Failed { stop } => {
                        summary.failed += 1;
                        if stop {
                            summary.status = RunStatus::Partial;
                            break 'trackers;
                        }
                    }
                }
            }
        }
        if summary.status == RunStatus::Done && summary.failed > 0 {
            summary.status = if summary.checked == 0 {
                RunStatus::Failed
            } else {
                RunStatus::Partial
            };
        }
        let store = self.store()?;
        store.finish_run(run, summary.status, &now(), None)?;
        store.mark_project_run(project_id, &now())?;
        Ok(summary)
    }

    /// Projects whose schedule says a check is due.
    ///
    /// # Errors
    /// Store errors.
    pub fn due_projects(&self) -> Result<Vec<i64>, SerpAppError> {
        let now_secs = Timestamp::now().as_unix_micros() / 1_000_000;
        Ok(self
            .projects()?
            .into_iter()
            .filter(|project| {
                let Some(interval) = project.schedule.interval_secs() else {
                    return false;
                };
                project.keywords > 0
                    && project.last_run_at.as_deref().is_none_or(|last| {
                        seconds_of(last).is_none_or(|last| now_secs - last >= interval)
                    })
            })
            .map(|project| project.id)
            .collect())
    }

    /// The rank report of a project.
    ///
    /// # Errors
    /// Store errors, an unknown id.
    #[allow(clippy::cast_precision_loss)] // volumes
    pub fn rank_report(&self, project_id: i64) -> Result<RankReport, SerpAppError> {
        let project = self.project(project_id)?;
        let store = self.store()?;
        let keywords = store.keywords(project_id)?;
        let rankings = store.rankings(project_id)?;
        let mut by_pair: Checks<'_> = BTreeMap::new();
        for ranking in &rankings {
            by_pair
                .entry((ranking.keyword_id, ranking.tracker_id))
                .or_default()
                .push(ranking);
        }
        let mut volumes: HashMap<Pair, Option<u64>> = HashMap::new();
        let mut rows = Vec::new();
        for keyword in &keywords {
            for tracker in &project.trackers {
                let key = keyword_key(&keyword.keyword, &tracker.language);
                let volume = store
                    .volume(&key, tracker.engine, &tracker.country)?
                    .and_then(|(volume, _)| volume);
                volumes.insert((keyword.id, tracker.id), volume);
                let checks = by_pair
                    .get(&(keyword.id, tracker.id))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                rows.push(rank_row(keyword, tracker.id, checks, volume));
            }
        }
        let weight = |pair: &Pair| volumes.get(pair).copied().flatten().unwrap_or(1).max(1) as f64;
        let visibility = visibility_points(&rankings, &weight);
        let competitors = competitor_shares(&store, &project, &by_pair, &weight)?;
        Ok(RankReport {
            project,
            rows,
            visibility,
            competitors,
        })
    }
}

/// A keyword and a tracker.
type Pair = (i64, i64);
/// Checks by keyword and tracker, oldest first.
type Checks<'a> = BTreeMap<Pair, Vec<&'a RankingRow>>;

/// What one search of a rank check found.
enum Checked {
    /// The search returned results; what it cost.
    Found(f64),
    /// It failed; `stop` ends the run (an uncleared check page, a cancellation).
    Failed {
        /// Whether to stop.
        stop: bool,
    },
}

fn ranking_of(
    run: i64,
    project_id: i64,
    keyword_id: i64,
    tracker_id: i64,
    result: Result<crate::SearchOutcome, SerpAppError>,
) -> Result<(RankingRow, Checked), SerpAppError> {
    let mut ranking = RankingRow {
        run_id: run,
        project_id,
        keyword_id,
        tracker_id,
        snapshot_id: None,
        checked_at: now(),
        position: None,
        url: None,
        positions: Vec::new(),
        error: None,
    };
    let checked = match result {
        Ok(outcome) => {
            ranking.snapshot_id = Some(outcome.snapshot_id);
            if let Some(rank) = outcome.rank {
                ranking.position = Some(rank.position);
                ranking.url = Some(rank.url);
                ranking.positions = rank.positions;
            }
            Checked::Found(outcome.cost_usd)
        }
        Err(SerpAppError::Search { error, .. }) => {
            ranking.error = Some(error.to_string());
            Checked::Failed {
                stop: matches!(error, ProviderError::Blocked(_) | ProviderError::Cancelled),
            }
        }
        Err(other) => return Err(other),
    };
    Ok((ranking, checked))
}

/// A stored ranking as the shared measures read it.
fn check_of(ranking: &RankingRow) -> Check<'_> {
    Check {
        checked_at: &ranking.checked_at,
        position: ranking.position,
        url: ranking.url.as_deref(),
        pages: ranking.positions.len(),
        error: ranking.error.as_deref(),
    }
}

fn rank_row(
    keyword: &KeywordRow,
    tracker_id: i64,
    checks: &[&RankingRow],
    volume: Option<u64>,
) -> RankRow {
    let checks: Vec<Check<'_>> = checks.iter().map(|check| check_of(check)).collect();
    let trend = trend(&checks);
    RankRow {
        keyword_id: keyword.id,
        keyword: keyword.keyword.clone(),
        tags: keyword.tags.clone(),
        tracker_id,
        position: trend.position,
        previous: trend.previous,
        change: trend.change,
        best: trend.best,
        url: trend.url,
        cannibalised: trend.cannibalised,
        checked_at: trend.checked_at,
        history: trend.history,
        volume,
        error: trend.error,
    }
}

/// Visibility by day: the last successful check of each keyword and tracker that day.
fn visibility_points(
    rankings: &[RankingRow],
    weight: &dyn Fn(&Pair) -> f64,
) -> Vec<VisibilityPoint> {
    visibility(
        rankings
            .iter()
            .map(|ranking| ((ranking.keyword_id, ranking.tracker_id), check_of(ranking))),
        weight,
    )
}

/// Shares of the project's site and its competitors over the latest snapshot of each keyword
/// and tracker; without listed competitors, the five most visible other domains.
fn competitor_shares(
    store: &octopuson_serp_store::SerpStore,
    project: &Project,
    by_pair: &Checks<'_>,
    weight: &dyn Fn(&Pair) -> f64,
) -> Result<Vec<CompetitorShare>, SerpAppError> {
    let mut pages = Vec::new();
    for (pair, checks) in by_pair {
        let Some(snapshot) = checks.iter().rev().find_map(|check| check.snapshot_id) else {
            continue;
        };
        pages.push(ResultPage {
            weight: weight(pair),
            domains: store.snapshot_domains(snapshot)?,
        });
    }
    let own = project
        .target
        .value
        .split('/')
        .next()
        .map(octopuson_serp::registrable_domain)
        .unwrap_or_default();
    Ok(shares_over(&pages, &own, &project.competitors))
}

#[cfg(test)]
mod tests {
    use octopuson_serp_score::rank::ctr;

    use super::*;

    #[test]
    fn reads_timestamps_and_click_shares() {
        let day = seconds_of("2026-10-18T00:00:00Z").unwrap();
        assert_eq!(seconds_of("2026-10-19T00:00:01Z"), Some(day + 86_401));
        assert_eq!(seconds_of("nonsense"), None);
        assert!(ctr(1) > ctr(2));
        assert_eq!(ctr(15), 0.01);
        assert_eq!(ctr(40), 0.0);
        assert_eq!(Schedule::parse("weekly"), Schedule::Weekly);
    }
}
