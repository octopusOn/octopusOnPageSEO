//! Alerts: what changed for the worse (or much for the better) in a project, found by rules
//! after the program's work, kept once (an alert's key comes from what it is about, so two
//! copies finding the same change keep one record), read in the program, sent by e-mail on the
//! web and shown as a system notification on the desktop.
//!
//! Sending is per copy: `mailed_at` and `notified_at` do not travel; whether an alert was read
//! does.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlatformStore, PlatformStoreError};

/// How alerts are e-mailed: not at all, as soon as they are found, or once a day.
pub const ALERT_EMAILS: &[&str] = &["off", "instant", "daily"];

/// Languages alert e-mails are written in.
pub const ALERT_LANGUAGES: &[&str] = &["tr", "en", "de", "es"];

/// Recipients of a project's alert e-mails at most.
pub const MAX_ALERT_RECIPIENTS: usize = 5;

/// What a project is alerted about, with each rule's threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)] // one switch per rule
pub struct AlertRules {
    /// Places a tracked keyword must fall to be reported; 0 turns it off.
    pub rank_drop: u32,
    /// A keyword leaving the first ten results.
    pub top10_exit: bool,
    /// Rank tracking visibility falling by this many percent; 0 turns it off.
    pub visibility_drop: f64,
    /// Search Console clicks falling by this many percent against the period before.
    pub gsc_drop: f64,
    /// Audit health falling by this many points.
    pub health_drop: f64,
    /// High-priority audit checks finding pages they did not find before.
    pub audit_issues: bool,
    /// AI mention rate changing by this many points (either way).
    pub ai_change: f64,
    /// Local share of voice falling by this many points.
    pub local_drop: f64,
    /// New reviews of one or two stars.
    pub reviews: bool,
    /// Competitors growing their keywords much faster (between snapshots).
    pub rivals: bool,
    /// Won and lost linking sites, checked weekly (paid, about $0.05 a week).
    pub links: bool,
}

impl Default for AlertRules {
    fn default() -> Self {
        Self {
            rank_drop: 5,
            top10_exit: true,
            visibility_drop: 10.0,
            gsc_drop: 20.0,
            health_drop: 5.0,
            audit_issues: true,
            ai_change: 10.0,
            local_drop: 10.0,
            reviews: true,
            rivals: true,
            links: false,
        }
    }
}

/// A project's alerting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertSettings {
    /// Alerts are looked for at all.
    pub enabled: bool,
    /// What is alerted about.
    pub rules: AlertRules,
    /// `off`, `instant` or `daily` (sent from the web).
    pub email: String,
    /// Who gets the e-mails.
    pub recipients: Vec<String>,
    /// Language of the e-mails; the project's when empty.
    pub lang: String,
    /// When the last daily e-mail went out, RFC 3339.
    #[serde(default)]
    pub last_digest_at: Option<String>,
    /// When links were last checked, RFC 3339.
    #[serde(default)]
    pub links_last_run: Option<String>,
}

impl Default for AlertSettings {
    /// On, inside the program only: nothing is sent or paid until chosen.
    fn default() -> Self {
        Self {
            enabled: true,
            rules: AlertRules::default(),
            email: "off".to_owned(),
            recipients: Vec::new(),
            lang: String::new(),
            last_digest_at: None,
            links_last_run: None,
        }
    }
}

/// An alert to keep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewAlert {
    /// What it is about (`rank|google-tr|2026-10-23T09:00`): kept once.
    pub key: String,
    /// The rule (`rank_drop`, `gsc_drop` …).
    pub kind: String,
    /// `bad`, `good` or `info`.
    pub severity: String,
    /// The module page that shows it.
    pub page: String,
    /// Its tab and what it shows.
    pub params: Value,
    /// Numbers and names its text uses.
    pub detail: Value,
    /// RFC 3339.
    pub created_at: String,
}

/// A kept alert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRow {
    /// Alert id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// What it says.
    #[serde(flatten)]
    pub alert: NewAlert,
    /// When it was read, RFC 3339.
    pub read_at: Option<String>,
}

fn invalid(message: &str) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.to_owned())
}

fn json_of(text: &str) -> Result<Value, PlatformStoreError> {
    serde_json::from_str(text)
        .map_err(|error| PlatformStoreError::Corrupt(format!("alerts: {error}")))
}

const ALERT_COLUMNS: &str =
    "id, project_id, key, kind, severity, page, params, detail, created_at, read_at";

type AlertColumns = (
    i64,
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);

fn alert_columns(row: &Row<'_>) -> duckdb::Result<AlertColumns> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

fn alert_row(columns: AlertColumns) -> Result<AlertRow, PlatformStoreError> {
    let (id, project_id, key, kind, severity, page, params, detail, created_at, read_at) = columns;
    Ok(AlertRow {
        id,
        project_id,
        alert: NewAlert {
            key,
            kind,
            severity,
            page,
            params: json_of(&params)?,
            detail: json_of(&detail)?,
            created_at,
        },
        read_at,
    })
}

fn clean_settings(settings: &AlertSettings) -> Result<AlertSettings, PlatformStoreError> {
    if !ALERT_EMAILS.contains(&settings.email.as_str()) {
        return Err(invalid("email: off, instant or daily"));
    }
    if !settings.lang.is_empty() && !ALERT_LANGUAGES.contains(&settings.lang.as_str()) {
        return Err(invalid("lang: tr, en, de or es"));
    }
    let rules = &settings.rules;
    let percent = |value: f64| (0.0..=100.0).contains(&value);
    if rules.rank_drop > 100
        || ![
            rules.visibility_drop,
            rules.gsc_drop,
            rules.health_drop,
            rules.ai_change,
            rules.local_drop,
        ]
        .into_iter()
        .all(percent)
    {
        return Err(invalid("rules: thresholds from 0 to 100"));
    }
    let mut recipients: Vec<String> = Vec::new();
    for recipient in &settings.recipients {
        let recipient = recipient.trim().to_lowercase();
        if recipient.is_empty() {
            continue;
        }
        if !crate::reports::valid_email(&recipient) {
            return Err(invalid(&format!(
                "recipients: `{recipient}` is not an e-mail address"
            )));
        }
        if !recipients.contains(&recipient) {
            recipients.push(recipient);
        }
    }
    if recipients.len() > MAX_ALERT_RECIPIENTS {
        return Err(invalid("recipients: at most 5"));
    }
    if settings.email != "off" && recipients.is_empty() {
        return Err(invalid("recipients: add who gets the alert e-mails"));
    }
    Ok(AlertSettings {
        recipients,
        ..settings.clone()
    })
}

impl PlatformStore {
    /// A project's alerting; the defaults until saved.
    ///
    /// # Errors
    /// Database errors, or stored rules that are not JSON.
    pub fn alert_settings(&self, project_id: i64) -> Result<AlertSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT enabled, rules, email, recipients, lang, last_digest_at, links_last_run
             FROM alert_settings WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, bool>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        let Some(row) = rows.next().transpose()? else {
            return Ok(AlertSettings::default());
        };
        let (enabled, rules, email, recipients, lang, last_digest_at, links_last_run) = row;
        Ok(AlertSettings {
            enabled,
            rules: serde_json::from_value(json_of(&rules)?).unwrap_or_default(),
            email,
            recipients: crate::parse_list(&recipients)?,
            lang,
            last_digest_at,
            links_last_run,
        })
    }

    /// Saves a project's alerting; when e-mails and links last went out stay.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown e-mail choice or language, a threshold
    /// out of range or a recipient that is not an address; database errors.
    pub fn save_alert_settings(
        &self,
        project_id: i64,
        settings: &AlertSettings,
    ) -> Result<AlertSettings, PlatformStoreError> {
        self.project(project_id)?;
        let clean = clean_settings(settings)?;
        let rules = serde_json::to_string(&clean.rules)
            .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))?;
        self.connection.execute(
            "INSERT INTO alert_settings (project_id, enabled, rules, email, recipients, lang)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET enabled = excluded.enabled,
             rules = excluded.rules, email = excluded.email, recipients = excluded.recipients,
             lang = excluded.lang",
            params![
                project_id,
                clean.enabled,
                rules,
                clean.email,
                crate::list_json(&clean.recipients),
                clean.lang
            ],
        )?;
        self.alert_settings(project_id)
    }

    /// Every project with its alerting and when its alerts were last looked for (RFC 3339);
    /// projects never saved have the defaults.
    ///
    /// # Errors
    /// Database errors.
    pub fn alert_plans(
        &self,
    ) -> Result<Vec<(i64, AlertSettings, Option<String>)>, PlatformStoreError> {
        let ids: Vec<(i64, Option<String>)> = {
            let mut statement = self.connection.prepare(
                "SELECT p.id, s.evaluated_at FROM projects p
                 LEFT JOIN alert_settings s ON s.project_id = p.id ORDER BY p.id",
            )?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.into_iter()
            .map(|(id, evaluated)| Ok((id, self.alert_settings(id)?, evaluated)))
            .collect()
    }

    fn alert_mark(
        &self,
        project_id: i64,
        column: &str,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        let defaults = AlertSettings::default();
        self.connection.execute(
            &format!(
                "INSERT INTO alert_settings (project_id, enabled, rules, email, recipients, lang, {column})
                 VALUES (?, ?, ?, ?, '[]', '', ?)
                 ON CONFLICT (project_id) DO UPDATE SET {column} = excluded.{column}"
            ),
            params![
                project_id,
                defaults.enabled,
                serde_json::to_string(&defaults.rules).unwrap_or_default(),
                defaults.email,
                now
            ],
        )?;
        Ok(())
    }

    /// Notes when a project's alerts were looked for (kept on this copy only).
    ///
    /// # Errors
    /// Database errors.
    pub fn note_alert_evaluation(
        &self,
        project_id: i64,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.alert_mark(project_id, "evaluated_at", now)
    }

    /// Notes when a project's daily alert e-mail went out.
    ///
    /// # Errors
    /// Database errors.
    pub fn note_alert_digest(&self, project_id: i64, now: &str) -> Result<(), PlatformStoreError> {
        self.alert_mark(project_id, "last_digest_at", now)
    }

    /// Notes when a project's links were last checked for alerts.
    ///
    /// # Errors
    /// Database errors.
    pub fn note_link_watch(&self, project_id: i64, now: &str) -> Result<(), PlatformStoreError> {
        self.alert_mark(project_id, "links_last_run", now)
    }

    /// Keeps an alert unless one with its key is kept; whether it was new.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_alert(&self, project_id: i64, alert: &NewAlert) -> Result<bool, PlatformStoreError> {
        let known: i64 = self.connection.query_row(
            "SELECT count(*) FROM alerts WHERE project_id = ? AND key = ?",
            params![project_id, alert.key],
            |row| row.get(0),
        )?;
        if known > 0 {
            return Ok(false);
        }
        self.connection.execute(
            "INSERT INTO alerts (project_id, key, kind, severity, page, params, detail, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                project_id,
                alert.key,
                alert.kind,
                alert.severity,
                alert.page,
                alert.params.to_string(),
                alert.detail.to_string(),
                alert.created_at
            ],
        )?;
        Ok(true)
    }

    /// Alerts, newest first: of one project or all, only unread ones when asked.
    ///
    /// # Errors
    /// Database errors, or stored alerts that are not JSON.
    pub fn alerts(
        &self,
        project_id: Option<i64>,
        unread_only: bool,
        limit: u32,
    ) -> Result<Vec<AlertRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {ALERT_COLUMNS} FROM alerts
             WHERE (?::BIGINT IS NULL OR project_id = ?) AND (NOT ? OR read_at IS NULL)
             ORDER BY created_at DESC, id DESC LIMIT ?"
        ))?;
        let rows = statement.query_map(
            params![project_id, project_id, unread_only, limit],
            alert_columns,
        )?;
        rows.map(|row| alert_row(row?)).collect()
    }

    /// Unread alerts by project.
    ///
    /// # Errors
    /// Database errors.
    pub fn unread_alerts(&self) -> Result<Vec<(i64, u64)>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT project_id, count(*)::UBIGINT FROM alerts WHERE read_at IS NULL
             GROUP BY project_id ORDER BY project_id",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Marks alerts read: the ones named, or every unread one (of a project, or all).
    ///
    /// # Errors
    /// Database errors.
    pub fn mark_alerts_read(
        &self,
        project_id: Option<i64>,
        ids: Option<&[i64]>,
        now: &str,
    ) -> Result<usize, PlatformStoreError> {
        Ok(match ids {
            Some(ids) => {
                let mut changed = 0;
                for id in ids {
                    changed += self.connection.execute(
                        "UPDATE alerts SET read_at = ? WHERE id = ? AND read_at IS NULL
                         AND (?::BIGINT IS NULL OR project_id = ?)",
                        params![now, id, project_id, project_id],
                    )?;
                }
                changed
            }
            None => self.connection.execute(
                "UPDATE alerts SET read_at = ? WHERE read_at IS NULL
                 AND (?::BIGINT IS NULL OR project_id = ?)",
                params![now, project_id, project_id],
            )?,
        })
    }

    /// Alerts found since `since` (RFC 3339) not yet sent by `channel` (`mailed` or
    /// `notified`) from this copy: of one project, or all.
    ///
    /// # Errors
    /// Database errors, or stored alerts that are not JSON.
    pub fn alerts_unsent(
        &self,
        project_id: Option<i64>,
        channel: &str,
        since: &str,
    ) -> Result<Vec<AlertRow>, PlatformStoreError> {
        let column = if channel == "mailed" {
            "mailed_at"
        } else {
            "notified_at"
        };
        let mut statement = self.connection.prepare(&format!(
            "SELECT {ALERT_COLUMNS} FROM alerts
             WHERE {column} IS NULL AND created_at >= ? AND (?::BIGINT IS NULL OR project_id = ?)
             ORDER BY created_at, id"
        ))?;
        let rows = statement.query_map(params![since, project_id, project_id], alert_columns)?;
        rows.map(|row| alert_row(row?)).collect()
    }

    /// Notes alerts as sent by `channel` (`mailed` or `notified`) from this copy.
    ///
    /// # Errors
    /// Database errors.
    pub fn mark_alerts_sent(
        &self,
        ids: &[i64],
        channel: &str,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        let column = if channel == "mailed" {
            "mailed_at"
        } else {
            "notified_at"
        };
        for id in ids {
            self.connection.execute(
                &format!("UPDATE alerts SET {column} = ? WHERE id = ?"),
                params![now, id],
            )?;
        }
        Ok(())
    }

    /// Removes a project's alerts and alerting.
    pub(crate) fn delete_alert_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in ["alerts", "alert_settings"] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
