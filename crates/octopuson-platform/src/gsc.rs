//! Search Console and Google Analytics: which properties a project reads, how often, its syncs,
//! and the numbers the latest sync brought. Google keeps the history itself, so a sync replaces
//! the project's numbers instead of adding to them.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError};

/// How often a project's numbers are read again.
pub const GSC_SCHEDULES: &[&str] = &["daily", "weekly", "manual"];

/// Days a sync may cover (each compared with as many days before).
pub const GSC_DAYS: &[u32] = &[7, 28, 90, 180];

/// Longest property name kept.
const MAX_PROPERTY: usize = 300;

/// Which properties a project reads and how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GscSettings {
    /// The Search Console property (`sc-domain:example.com` or `https://www.example.com/`).
    #[serde(default)]
    pub property: Option<String>,
    /// The Google Analytics 4 property (`properties/123`), if any.
    #[serde(default)]
    pub ga4_property: Option<String>,
    /// Its display name.
    #[serde(default)]
    pub ga4_name: Option<String>,
    /// Days of a sync.
    pub days: u32,
    /// `daily`, `weekly` or `manual`.
    pub schedule: String,
    /// When the last sync started, RFC 3339.
    #[serde(default)]
    pub last_sync_at: Option<String>,
}

impl Default for GscSettings {
    fn default() -> Self {
        Self {
            property: None,
            ga4_property: None,
            ga4_name: None,
            days: 28,
            schedule: "weekly".to_owned(),
            last_sync_at: None,
        }
    }
}

/// One sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GscSyncRow {
    /// Sync id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// Started by the schedule.
    pub scheduled: bool,
    /// `running`, `done`, `partial`, `failed`, `cancelled` or `interrupted`.
    pub status: String,
    /// First day of the period, `YYYY-MM-DD`.
    pub period_start: Option<String>,
    /// Last day.
    pub period_end: Option<String>,
    /// Rows read.
    pub rows: u64,
    /// The Analytics property's currency.
    pub currency: Option<String>,
    /// Google sampled or thresholded some Analytics numbers, or rows were left out.
    pub limited: bool,
    /// Why it failed or what was missing.
    pub error: Option<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub finished_at: Option<String>,
}

/// How a sync ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GscSyncEnd<'a> {
    /// `done`, `partial`, `failed` or `cancelled`.
    pub status: &'a str,
    /// The period read.
    pub period: Option<(&'a str, &'a str)>,
    /// Rows read.
    pub rows: u64,
    /// The Analytics property's currency.
    pub currency: Option<&'a str>,
    /// Numbers were sampled, thresholded or cut.
    pub limited: bool,
    /// Why it failed or what was missing.
    pub error: Option<&'a str>,
}

/// Search numbers of one key: a day, a query, a page, a country or a device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRow {
    /// The key.
    pub key: String,
    /// Clicks.
    pub clicks: f64,
    /// Impressions.
    pub impressions: f64,
    /// Average position.
    pub position: f64,
}

/// Search numbers of a query on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryPage {
    /// The query.
    pub query: String,
    /// The page.
    pub page: String,
    /// Clicks.
    pub clicks: f64,
    /// Impressions.
    pub impressions: f64,
    /// Average position.
    pub position: f64,
}

/// Organic traffic of one key: a day or a landing page's path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrafficRow {
    /// The key.
    pub key: String,
    /// Sessions.
    pub sessions: f64,
    /// Engaged sessions.
    pub engaged_sessions: f64,
    /// Key events.
    pub key_events: f64,
    /// Revenue.
    pub revenue: f64,
}

/// What a search row is split by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    /// Days of both periods.
    Date,
    /// Queries.
    Query,
    /// Pages.
    Page,
    /// Countries.
    Country,
    /// Devices.
    Device,
}

impl SearchKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Query => "query",
            Self::Page => "page",
            Self::Country => "country",
            Self::Device => "device",
        }
    }
}

/// What a traffic row is split by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficKind {
    /// Days of both periods.
    Date,
    /// Landing pages.
    Landing,
}

impl TrafficKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Landing => "landing",
        }
    }
}

/// The period of a row: the sync's days, or as many days before them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// Days are their own period.
    Days,
    /// The sync's days.
    Now,
    /// The days before them.
    Before,
}

impl Period {
    const fn name(self) -> &'static str {
        match self {
            Self::Days => "",
            Self::Now => "now",
            Self::Before => "before",
        }
    }
}

/// Everything one sync brought.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GscData {
    /// Search rows with what they are split by and their period.
    pub search: Vec<(SearchKind, Period, SearchRow)>,
    /// Queries on pages, the sync's days.
    pub query_pages: Vec<QueryPage>,
    /// Analytics rows with what they are split by and their period.
    pub traffic: Vec<(TrafficKind, Period, TrafficRow)>,
}

impl GscData {
    /// Rows in all.
    pub fn rows(&self) -> u64 {
        (self.search.len() + self.query_pages.len() + self.traffic.len()) as u64
    }
}

fn invalid(message: &str) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.to_owned())
}

fn clean(value: Option<&String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn clean_settings(settings: &GscSettings) -> Result<GscSettings, PlatformStoreError> {
    if !GSC_SCHEDULES.contains(&settings.schedule.as_str()) {
        return Err(invalid("schedule: daily, weekly or manual"));
    }
    if !GSC_DAYS.contains(&settings.days) {
        return Err(invalid("days: 7, 28, 90 or 180"));
    }
    let property = clean(settings.property.as_ref());
    if property.as_ref().is_some_and(|property| {
        property.chars().count() > MAX_PROPERTY
            || !(property.starts_with("sc-domain:")
                || property.starts_with("https://")
                || property.starts_with("http://"))
    }) {
        return Err(invalid(
            "property: a Search Console property (sc-domain:example.com or https://www.example.com/)",
        ));
    }
    let ga4_property = clean(settings.ga4_property.as_ref());
    if ga4_property.as_ref().is_some_and(|id| {
        let digits = id.strip_prefix("properties/").unwrap_or(id);
        digits.is_empty() || digits.len() > 20 || !digits.bytes().all(|b| b.is_ascii_digit())
    }) {
        return Err(invalid(
            "ga4_property: a GA4 property id (properties/123456)",
        ));
    }
    Ok(GscSettings {
        ga4_name: clean(settings.ga4_name.as_ref())
            .filter(|_| ga4_property.is_some())
            .map(|name| name.chars().take(MAX_PROPERTY).collect()),
        ga4_property: ga4_property.map(|id| {
            if id.starts_with("properties/") {
                id
            } else {
                format!("properties/{id}")
            }
        }),
        property,
        days: settings.days,
        schedule: settings.schedule.clone(),
        last_sync_at: None,
    })
}

const SYNC_COLUMNS: &str = "id, project_id, scheduled, status, period_start, period_end, rows, currency, limited, error, started_at, finished_at";

fn sync(row: &Row<'_>) -> duckdb::Result<GscSyncRow> {
    Ok(GscSyncRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        scheduled: row.get(2)?,
        status: row.get(3)?,
        period_start: row.get(4)?,
        period_end: row.get(5)?,
        rows: row.get::<_, i64>(6)?.try_into().unwrap_or_default(),
        currency: row.get(7)?,
        limited: row.get(8)?,
        error: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
    })
}

impl PlatformStore {
    /// Which properties a project reads; the defaults until saved.
    ///
    /// # Errors
    /// Database errors.
    pub fn gsc_settings(&self, project_id: i64) -> Result<GscSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT property, ga4_property, ga4_name, days, schedule, last_sync_at
             FROM gsc_settings WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok(GscSettings {
                property: row.get(0)?,
                ga4_property: row.get(1)?,
                ga4_name: row.get(2)?,
                days: row.get(3)?,
                schedule: row.get(4)?,
                last_sync_at: row.get(5)?,
            })
        })?;
        Ok(rows.next().transpose()?.unwrap_or_default())
    }

    /// Saves which properties a project reads; the last sync time stays.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown schedule or period, or a property that is not one; database errors.
    pub fn save_gsc_settings(
        &self,
        project_id: i64,
        settings: &GscSettings,
    ) -> Result<GscSettings, PlatformStoreError> {
        self.project(project_id)?;
        let clean = clean_settings(settings)?;
        self.connection.execute(
            "INSERT INTO gsc_settings (project_id, property, ga4_property, ga4_name, days, schedule)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET property = excluded.property,
             ga4_property = excluded.ga4_property, ga4_name = excluded.ga4_name,
             days = excluded.days, schedule = excluded.schedule",
            params![
                project_id,
                clean.property,
                clean.ga4_property,
                clean.ga4_name,
                clean.days,
                clean.schedule
            ],
        )?;
        self.gsc_settings(project_id)
    }

    /// Every project's settings that names a Search Console property.
    ///
    /// # Errors
    /// Database errors.
    pub fn gsc_plans(&self) -> Result<Vec<(i64, GscSettings)>, PlatformStoreError> {
        let ids: Vec<i64> = {
            let mut statement = self.connection.prepare(
                "SELECT s.project_id FROM gsc_settings s JOIN projects p ON p.id = s.project_id
                 WHERE s.property IS NOT NULL ORDER BY s.project_id",
            )?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.into_iter()
            .map(|id| Ok((id, self.gsc_settings(id)?)))
            .collect()
    }

    /// Starts a sync at `now` and moves the schedule on; a sync left running by a program that
    /// stopped becomes `interrupted`.
    ///
    /// # Errors
    /// Database errors.
    pub fn begin_gsc_sync(
        &self,
        project_id: i64,
        scheduled: bool,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        let defaults = GscSettings::default();
        self.connection.execute(
            "INSERT INTO gsc_settings (project_id, days, schedule, last_sync_at) VALUES (?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET last_sync_at = excluded.last_sync_at",
            params![project_id, defaults.days, defaults.schedule, now],
        )?;
        self.connection.execute(
            "UPDATE gsc_syncs SET status = 'interrupted', finished_at = ?
             WHERE project_id = ? AND status = 'running'",
            params![now, project_id],
        )?;
        Ok(self.connection.query_row(
            "INSERT INTO gsc_syncs (project_id, scheduled, status, started_at)
             VALUES (?, ?, 'running', ?) RETURNING id",
            params![project_id, scheduled, now],
            |row| row.get(0),
        )?)
    }

    /// Replaces a project's numbers with what a sync brought, all at once.
    ///
    /// # Errors
    /// Database errors; nothing is replaced then.
    pub fn replace_gsc_data(
        &self,
        project_id: i64,
        data: &GscData,
    ) -> Result<(), PlatformStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for table in ["gsc_rows", "gsc_query_pages", "ga4_rows"] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        {
            let mut rows = transaction.appender("gsc_rows")?;
            for (kind, period, row) in &data.search {
                rows.append_row(params![
                    project_id,
                    kind.name(),
                    period.name(),
                    row.key,
                    row.clicks,
                    row.impressions,
                    row.position
                ])?;
            }
            rows.flush()?;
            let mut rows = transaction.appender("gsc_query_pages")?;
            for row in &data.query_pages {
                rows.append_row(params![
                    project_id,
                    row.query,
                    row.page,
                    row.clicks,
                    row.impressions,
                    row.position
                ])?;
            }
            rows.flush()?;
            let mut rows = transaction.appender("ga4_rows")?;
            for (kind, period, row) in &data.traffic {
                rows.append_row(params![
                    project_id,
                    kind.name(),
                    period.name(),
                    row.key,
                    row.sessions,
                    row.engaged_sessions,
                    row.key_events,
                    row.revenue
                ])?;
            }
            rows.flush()?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Records how a sync ended at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_gsc_sync(
        &self,
        sync_id: i64,
        end: &GscSyncEnd<'_>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE gsc_syncs SET status = ?, period_start = ?, period_end = ?, rows = ?,
             currency = ?, limited = ?, error = ?, finished_at = ? WHERE id = ?",
            params![
                end.status,
                end.period.map(|(start, _)| start),
                end.period.map(|(_, end)| end),
                i64::try_from(end.rows).unwrap_or(i64::MAX),
                end.currency,
                end.limited,
                end.error,
                now,
                sync_id
            ],
        )?;
        Ok(())
    }

    /// A project's latest syncs, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn gsc_syncs(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<GscSyncRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SYNC_COLUMNS} FROM gsc_syncs WHERE project_id = ? ORDER BY id DESC LIMIT ?"
        ))?;
        let rows = statement.query_map(params![project_id, limit], sync)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A project's search rows of one kind and period: days in order, the rest by clicks then
    /// impressions.
    ///
    /// # Errors
    /// Database errors.
    pub fn gsc_rows(
        &self,
        project_id: i64,
        kind: SearchKind,
        period: Period,
        limit: u32,
    ) -> Result<Vec<SearchRow>, PlatformStoreError> {
        let order = if kind == SearchKind::Date {
            "key"
        } else {
            "clicks DESC, impressions DESC, key"
        };
        let mut statement = self.connection.prepare(&format!(
            "SELECT key, clicks, impressions, position FROM gsc_rows
             WHERE project_id = ? AND kind = ? AND period = ? ORDER BY {order} LIMIT ?"
        ))?;
        let rows = statement.query_map(
            params![project_id, kind.name(), period.name(), limit],
            |row| {
                Ok(SearchRow {
                    key: row.get(0)?,
                    clicks: row.get(1)?,
                    impressions: row.get(2)?,
                    position: row.get(3)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A project's queries on pages: of one query when given, by clicks then impressions.
    ///
    /// # Errors
    /// Database errors.
    pub fn gsc_query_pages(
        &self,
        project_id: i64,
        query: Option<&str>,
        limit: u32,
    ) -> Result<Vec<QueryPage>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT query, page, clicks, impressions, position FROM gsc_query_pages
             WHERE project_id = ? AND (?::VARCHAR IS NULL OR query = ?)
             ORDER BY clicks DESC, impressions DESC, query, page LIMIT ?",
        )?;
        let rows = statement.query_map(params![project_id, query, query, limit], |row| {
            Ok(QueryPage {
                query: row.get(0)?,
                page: row.get(1)?,
                clicks: row.get(2)?,
                impressions: row.get(3)?,
                position: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A project's Analytics rows of one kind and period: days in order, pages by sessions.
    ///
    /// # Errors
    /// Database errors.
    pub fn ga4_rows(
        &self,
        project_id: i64,
        kind: TrafficKind,
        period: Period,
        limit: u32,
    ) -> Result<Vec<TrafficRow>, PlatformStoreError> {
        let order = if kind == TrafficKind::Date {
            "key"
        } else {
            "sessions DESC, key"
        };
        let mut statement = self.connection.prepare(&format!(
            "SELECT key, sessions, engaged_sessions, key_events, revenue FROM ga4_rows
             WHERE project_id = ? AND kind = ? AND period = ? ORDER BY {order} LIMIT ?"
        ))?;
        let rows = statement.query_map(
            params![project_id, kind.name(), period.name(), limit],
            |row| {
                Ok(TrafficRow {
                    key: row.get(0)?,
                    sessions: row.get(1)?,
                    engaged_sessions: row.get(2)?,
                    key_events: row.get(3)?,
                    revenue: row.get(4)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Deletes a project's Search Console and Analytics data.
    pub(crate) fn delete_gsc_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in [
            "gsc_rows",
            "gsc_query_pages",
            "ga4_rows",
            "gsc_syncs",
            "gsc_settings",
        ] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
