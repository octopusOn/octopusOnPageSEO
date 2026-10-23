//! Reports: the agency's branding (white label), each project's report definitions (which
//! parts, which language, how many days), their read-only share links, and their deliveries by
//! e-mail. Reports keep no numbers of their own: they are drawn from the modules' data when
//! shown.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError};

/// The parts a report can show, in the order they are offered.
pub const REPORT_WIDGETS: &[&str] = &[
    "summary",
    "rankings",
    "competitors",
    "search_console",
    "site_audit",
    "ai_visibility",
    "backlinks",
    "comparison",
    "comparison_previous",
    "local",
    "organic",
    "link_history",
    "gap",
    "keyword_lists",
    "opportunities",
];

/// Languages a report is written in.
pub const REPORT_LANGUAGES: &[&str] = &["tr", "en", "de", "es"];

/// Days a report covers.
pub const REPORT_DAYS: &[u32] = &[7, 28, 90];

/// How a report can look: white, deep sea (the program's own), the agency's colour, elegant.
pub const REPORT_DESIGNS: &[&str] = &["clean", "ocean", "accent", "elegant"];

/// The design of a report that has none.
pub const DEFAULT_DESIGN: &str = "clean";

/// How often a report is e-mailed: by hand, weekly, monthly, or after each new snapshot.
pub const REPORT_SCHEDULES: &[&str] = &["manual", "weekly", "monthly", "snapshot"];

/// Recipients of a report at most.
pub const MAX_RECIPIENTS: usize = 5;

/// Longest logo kept, as a `data:` address.
pub const MAX_LOGO: usize = 400_000;

/// Image types a logo may be.
const LOGO_TYPES: &[&str] = &[
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
    "data:image/svg+xml;base64,",
];

/// The agency's look on every report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branding {
    /// Company name in the header.
    #[serde(default)]
    pub company: Option<String>,
    /// Logo, a `data:image/…;base64,` address.
    #[serde(default)]
    pub logo: Option<String>,
    /// Accent colour, `#rrggbb`.
    #[serde(default)]
    pub accent: Option<String>,
    /// The agency's website.
    #[serde(default)]
    pub website: Option<String>,
    /// A line at the foot of every page.
    #[serde(default)]
    pub footer: Option<String>,
    /// Leaves out "Prepared with OctopusOn SEO".
    #[serde(default)]
    pub hide_octopuson: bool,
}

/// What a report shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSpec {
    /// Its name.
    pub name: String,
    /// `tr`, `en`, `de` or `es`.
    pub language: String,
    /// Days it covers: 7, 28 or 90.
    pub days: u32,
    /// Its parts, in order.
    pub widgets: Vec<String>,
    /// `manual`, `weekly` or `monthly`.
    pub schedule: String,
    /// Who gets it by e-mail.
    #[serde(default)]
    pub recipients: Vec<String>,
    /// How it looks: one of [`REPORT_DESIGNS`].
    #[serde(default = "default_design")]
    pub design: String,
    /// Its own colours over the design's.
    #[serde(default)]
    pub colors: ReportColors,
}

fn default_design() -> String {
    DEFAULT_DESIGN.to_owned()
}

/// A report's own colours, `#rrggbb`, each over its design's (and the branding's accent).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportColors {
    /// Section numbers, highlights, charts, bars and the contents' numbers.
    #[serde(default)]
    pub accent: Option<String>,
    /// The second colour of gradients and chart lines.
    #[serde(default)]
    pub accent2: Option<String>,
    /// The cover's background; its ink is chosen to read on it.
    #[serde(default)]
    pub cover: Option<String>,
    /// Section, table and chart titles.
    #[serde(default)]
    pub heading: Option<String>,
    /// The background of table heads; their ink is chosen to read on it.
    #[serde(default)]
    pub table: Option<String>,
}

impl ReportColors {
    /// Whether no colour is its own.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// A colour like `#1e6fd9`, lowercase; `None` stays `None`.
fn clean_color(value: Option<&String>, field: &str) -> Result<Option<String>, PlatformStoreError> {
    let Some(value) = value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(invalid(&format!("colors.{field}: a colour like #1e6fd9")));
    }
    Ok(Some(value.to_ascii_lowercase()))
}

/// A saved report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRow {
    /// Report id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// What it shows.
    #[serde(flatten)]
    pub spec: ReportSpec,
    /// The token of its read-only link, when shared.
    pub share_token: Option<String>,
    /// Times the link was opened.
    pub share_views: u64,
    /// When it was last e-mailed, RFC 3339.
    pub last_sent_at: Option<String>,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
}

/// One delivery by e-mail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryRow {
    /// Delivery id.
    pub id: i64,
    /// Its report.
    pub report_id: i64,
    /// Its project.
    pub project_id: i64,
    /// Sent by the schedule.
    pub scheduled: bool,
    /// `sent` or `failed`.
    pub status: String,
    /// Recipients it went to.
    pub recipients: u32,
    /// Why it failed.
    pub error: Option<String>,
    /// RFC 3339.
    pub sent_at: String,
}

fn invalid(message: &str) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.to_owned())
}

fn clean_text(
    value: Option<&String>,
    longest: usize,
    field: &str,
) -> Result<Option<String>, PlatformStoreError> {
    let Some(text) = value
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
    else {
        return Ok(None);
    };
    if text.chars().count() > longest {
        return Err(invalid(&format!("{field}: at most {longest} characters")));
    }
    Ok(Some(text.to_owned()))
}

fn clean_branding(branding: &Branding) -> Result<Branding, PlatformStoreError> {
    let logo = branding
        .logo
        .as_ref()
        .map(|logo| logo.trim().to_owned())
        .filter(|logo| !logo.is_empty());
    if let Some(logo) = &logo {
        let known = LOGO_TYPES.iter().any(|kind| logo.starts_with(kind));
        let body_ok = logo.split_once(',').is_some_and(|(_, body)| {
            body.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        });
        if !known || !body_ok {
            return Err(invalid("logo: a PNG, JPEG, WebP or SVG image"));
        }
        if logo.len() > MAX_LOGO {
            return Err(invalid("logo: at most about 300 KB"));
        }
    }
    let accent = clean_text(branding.accent.as_ref(), 7, "accent")?;
    if accent.as_ref().is_some_and(|accent| {
        accent.len() != 7
            || !accent.starts_with('#')
            || !accent[1..].bytes().all(|b| b.is_ascii_hexdigit())
    }) {
        return Err(invalid("accent: a colour like #1e6fd9"));
    }
    let website = clean_text(branding.website.as_ref(), 200, "website")?;
    if website
        .as_ref()
        .is_some_and(|site| !(site.starts_with("https://") || site.starts_with("http://")))
    {
        return Err(invalid("website: an address starting with https://"));
    }
    Ok(Branding {
        company: clean_text(branding.company.as_ref(), 120, "company")?,
        logo,
        accent: accent.map(|accent| accent.to_ascii_lowercase()),
        website,
        footer: clean_text(branding.footer.as_ref(), 300, "footer")?,
        hide_octopuson: branding.hide_octopuson,
    })
}

/// Whether an e-mail address looks deliverable: one `@`, a dotted domain, nothing that could
/// split it into several addresses or headers.
pub(crate) fn valid_email(address: &str) -> bool {
    let Some((local, domain)) = address.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && address.len() <= 254
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !address.chars().any(|c| {
            c.is_control()
                || c.is_whitespace()
                || matches!(
                    c,
                    '<' | '>' | ',' | ';' | '"' | '(' | ')' | '[' | ']' | '\\'
                )
        })
}

fn clean_spec(spec: &ReportSpec) -> Result<ReportSpec, PlatformStoreError> {
    let name = spec.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 120 {
        return Err(invalid("name: 1 to 120 characters"));
    }
    if !REPORT_LANGUAGES.contains(&spec.language.as_str()) {
        return Err(invalid("language: tr, en, de or es"));
    }
    if !REPORT_DAYS.contains(&spec.days) {
        return Err(invalid("days: 7, 28 or 90"));
    }
    if !REPORT_SCHEDULES.contains(&spec.schedule.as_str()) {
        return Err(invalid("schedule: manual, weekly, monthly or snapshot"));
    }
    let mut widgets: Vec<String> = Vec::new();
    for widget in &spec.widgets {
        if !REPORT_WIDGETS.contains(&widget.as_str()) {
            return Err(invalid(&format!("widgets: unknown part `{widget}`")));
        }
        if !widgets.contains(widget) {
            widgets.push(widget.clone());
        }
    }
    if widgets.is_empty() {
        return Err(invalid("widgets: choose at least one part"));
    }
    let mut recipients: Vec<String> = Vec::new();
    for recipient in &spec.recipients {
        let recipient = recipient.trim().to_lowercase();
        if recipient.is_empty() {
            continue;
        }
        if !valid_email(&recipient) {
            return Err(invalid(&format!(
                "recipients: `{recipient}` is not an e-mail address"
            )));
        }
        if !recipients.contains(&recipient) {
            recipients.push(recipient);
        }
    }
    if recipients.len() > MAX_RECIPIENTS {
        return Err(invalid("recipients: at most 5"));
    }
    if !REPORT_DESIGNS.contains(&spec.design.as_str()) {
        return Err(invalid("design: clean, ocean, accent or elegant"));
    }
    let colors = &spec.colors;
    Ok(ReportSpec {
        name,
        language: spec.language.clone(),
        days: spec.days,
        widgets,
        schedule: spec.schedule.clone(),
        recipients,
        design: spec.design.clone(),
        colors: ReportColors {
            accent: clean_color(colors.accent.as_ref(), "accent")?,
            accent2: clean_color(colors.accent2.as_ref(), "accent2")?,
            cover: clean_color(colors.cover.as_ref(), "cover")?,
            heading: clean_color(colors.heading.as_ref(), "heading")?,
            table: clean_color(colors.table.as_ref(), "table")?,
        },
    })
}

/// Colours as stored: JSON, or none when all are the design's.
fn colors_of(colors: &ReportColors) -> Option<String> {
    (!colors.is_empty()).then(|| json_of(colors))
}

fn json_of<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_owned())
}

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, PlatformStoreError> {
    serde_json::from_str(text).map_err(|error| PlatformStoreError::Corrupt(error.to_string()))
}

const REPORT_COLUMNS: &str = "id, project_id, name, language, days, widgets, schedule, recipients, share_token, share_views, last_sent_at, created_at, updated_at, design, colors";

/// A report row with its lists still as JSON.
type RawReport = (ReportRow, String, String);

fn raw_report(row: &Row<'_>) -> duckdb::Result<RawReport> {
    Ok((
        ReportRow {
            id: row.get(0)?,
            project_id: row.get(1)?,
            spec: ReportSpec {
                name: row.get(2)?,
                language: row.get(3)?,
                days: row.get(4)?,
                widgets: Vec::new(),
                schedule: row.get(6)?,
                recipients: Vec::new(),
                // A design this program does not know (or none) reads as the default.
                design: row
                    .get::<_, Option<String>>(13)?
                    .filter(|design| REPORT_DESIGNS.contains(&design.as_str()))
                    .unwrap_or_else(default_design),
                // Colours that cannot be read keep the design's.
                colors: row
                    .get::<_, Option<String>>(14)?
                    .and_then(|text| serde_json::from_str(&text).ok())
                    .unwrap_or_default(),
            },
            share_token: row.get(8)?,
            share_views: row.get::<_, i64>(9)?.try_into().unwrap_or_default(),
            last_sent_at: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        },
        row.get(5)?,
        row.get(7)?,
    ))
}

fn finish(raw: RawReport) -> Result<ReportRow, PlatformStoreError> {
    let (mut report, widgets, recipients) = raw;
    report.spec.widgets = parse(&widgets)?;
    report.spec.recipients = parse(&recipients)?;
    Ok(report)
}

impl PlatformStore {
    /// The agency's branding; empty until saved.
    ///
    /// # Errors
    /// Database errors.
    pub fn report_branding(&self) -> Result<Branding, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT company, logo, accent, website, footer, hide_octopuson FROM report_branding WHERE id = 1",
        )?;
        let mut rows = statement.query_map([], |row| {
            Ok(Branding {
                company: row.get(0)?,
                logo: row.get(1)?,
                accent: row.get(2)?,
                website: row.get(3)?,
                footer: row.get(4)?,
                hide_octopuson: row.get(5)?,
            })
        })?;
        Ok(rows.next().transpose()?.unwrap_or_default())
    }

    /// Saves the agency's branding.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for a logo that is not an image, an accent that is not a
    /// colour, or overlong texts; database errors.
    pub fn save_report_branding(
        &self,
        branding: &Branding,
    ) -> Result<Branding, PlatformStoreError> {
        let clean = clean_branding(branding)?;
        self.connection.execute(
            "INSERT INTO report_branding (id, company, logo, accent, website, footer, hide_octopuson)
             VALUES (1, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET company = excluded.company, logo = excluded.logo,
             accent = excluded.accent, website = excluded.website, footer = excluded.footer,
             hide_octopuson = excluded.hide_octopuson",
            params![
                clean.company,
                clean.logo,
                clean.accent,
                clean.website,
                clean.footer,
                clean.hide_octopuson
            ],
        )?;
        self.report_branding()
    }

    /// A project's reports, newest first.
    ///
    /// # Errors
    /// Database errors, or stored lists that are not JSON.
    pub fn reports(&self, project_id: i64) -> Result<Vec<ReportRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {REPORT_COLUMNS} FROM reports WHERE project_id = ? ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = statement.query_map(params![project_id], raw_report)?;
        rows.map(|row| finish(row?)).collect()
    }

    /// One report of a project.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown report; database errors.
    pub fn report(&self, project_id: i64, report_id: i64) -> Result<ReportRow, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {REPORT_COLUMNS} FROM reports WHERE project_id = ? AND id = ?"
        ))?;
        let mut rows = statement.query_map(params![project_id, report_id], raw_report)?;
        match rows.next() {
            Some(row) => finish(row?),
            None => Err(PlatformStoreError::NotFound(format!("report {report_id}"))),
        }
    }

    /// Saves a new report (without `report_id`) or changes one.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project or report;
    /// [`PlatformStoreError::Invalid`] for an empty name, an unknown part, language, period or
    /// schedule, or recipients that are not e-mail addresses (at most five); database errors.
    pub fn save_report(
        &self,
        project_id: i64,
        report_id: Option<i64>,
        spec: &ReportSpec,
        now: &str,
    ) -> Result<ReportRow, PlatformStoreError> {
        self.project(project_id)?;
        let clean = clean_spec(spec)?;
        let id = match report_id {
            Some(id) => {
                self.report(project_id, id)?;
                self.connection.execute(
                    "UPDATE reports SET name = ?, language = ?, days = ?, widgets = ?, schedule = ?,
                     recipients = ?, design = ?, colors = ?, updated_at = ? WHERE project_id = ? AND id = ?",
                    params![
                        clean.name,
                        clean.language,
                        clean.days,
                        json_of(&clean.widgets),
                        clean.schedule,
                        json_of(&clean.recipients),
                        clean.design,
                        colors_of(&clean.colors),
                        now,
                        project_id,
                        id
                    ],
                )?;
                id
            }
            None => self.connection.query_row(
                "INSERT INTO reports (project_id, name, language, days, widgets, schedule, recipients,
                 design, colors, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
                params![
                    project_id,
                    clean.name,
                    clean.language,
                    clean.days,
                    json_of(&clean.widgets),
                    clean.schedule,
                    json_of(&clean.recipients),
                    clean.design,
                    colors_of(&clean.colors),
                    now,
                    now
                ],
                |row| row.get(0),
            )?,
        };
        self.report(project_id, id)
    }

    /// Deletes a report with its link and deliveries.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown report; database errors.
    pub fn delete_report(&self, project_id: i64, report_id: i64) -> Result<(), PlatformStoreError> {
        self.report(project_id, report_id)?;
        self.connection.execute(
            "DELETE FROM report_deliveries WHERE report_id = ?",
            params![report_id],
        )?;
        self.connection
            .execute("DELETE FROM reports WHERE id = ?", params![report_id])?;
        Ok(())
    }

    /// Gives a report a read-only link token, or with `None` takes it away.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown report; database errors.
    pub fn set_report_share(
        &self,
        project_id: i64,
        report_id: i64,
        token: Option<&str>,
    ) -> Result<ReportRow, PlatformStoreError> {
        self.report(project_id, report_id)?;
        self.connection.execute(
            "UPDATE reports SET share_token = ?, share_views = 0 WHERE id = ?",
            params![token, report_id],
        )?;
        self.report(project_id, report_id)
    }

    /// The report a link token opens, counting the view.
    ///
    /// # Errors
    /// Database errors.
    pub fn open_shared_report(&self, token: &str) -> Result<Option<ReportRow>, PlatformStoreError> {
        if token.is_empty() {
            return Ok(None);
        }
        let found = {
            let mut statement = self.connection.prepare(&format!(
                "SELECT {REPORT_COLUMNS} FROM reports WHERE share_token = ?"
            ))?;
            let mut rows = statement.query_map(params![token], raw_report)?;
            rows.next().transpose()?
        };
        let Some(raw) = found else {
            return Ok(None);
        };
        self.connection.execute(
            "UPDATE reports SET share_views = share_views + 1 WHERE share_token = ?",
            params![token],
        )?;
        finish(raw).map(Some)
    }

    /// Every report e-mailed on a schedule.
    ///
    /// # Errors
    /// Database errors, or stored lists that are not JSON.
    pub fn report_plans(&self) -> Result<Vec<ReportRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {REPORT_COLUMNS} FROM reports r WHERE schedule <> 'manual' AND recipients <> '[]'
             AND EXISTS (SELECT 1 FROM projects p WHERE p.id = r.project_id) ORDER BY id"
        ))?;
        let rows = statement.query_map([], raw_report)?;
        rows.map(|row| finish(row?)).collect()
    }

    /// Records a delivery and, when it went out, when the report was last sent.
    ///
    /// # Errors
    /// Database errors.
    pub fn add_report_delivery(
        &self,
        report: &ReportRow,
        scheduled: bool,
        error: Option<&str>,
        now: &str,
    ) -> Result<DeliveryRow, PlatformStoreError> {
        let recipients = u32::try_from(report.spec.recipients.len()).unwrap_or(u32::MAX);
        let status = if error.is_some() { "failed" } else { "sent" };
        let id: i64 = self.connection.query_row(
            "INSERT INTO report_deliveries (report_id, project_id, scheduled, status, recipients, error, sent_at)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![report.id, report.project_id, scheduled, status, recipients, error, now],
            |row| row.get(0),
        )?;
        // A failed delivery also waits for the next turn of the schedule: it is not retried
        // every minute.
        self.connection.execute(
            "UPDATE reports SET last_sent_at = ? WHERE id = ?",
            params![now, report.id],
        )?;
        Ok(DeliveryRow {
            id,
            report_id: report.id,
            project_id: report.project_id,
            scheduled,
            status: status.to_owned(),
            recipients,
            error: error.map(str::to_owned),
            sent_at: now.to_owned(),
        })
    }

    /// A report's latest deliveries, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn report_deliveries(
        &self,
        report_id: i64,
        limit: u32,
    ) -> Result<Vec<DeliveryRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, report_id, project_id, scheduled, status, recipients, error, sent_at
             FROM report_deliveries WHERE report_id = ? ORDER BY sent_at DESC, id DESC LIMIT ?",
        )?;
        let rows = statement.query_map(params![report_id, limit], |row| {
            Ok(DeliveryRow {
                id: row.get(0)?,
                report_id: row.get(1)?,
                project_id: row.get(2)?,
                scheduled: row.get(3)?,
                status: row.get(4)?,
                recipients: row.get(5)?,
                error: row.get(6)?,
                sent_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Deletes a project's reports and deliveries.
    pub(crate) fn delete_report_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in ["report_deliveries", "reports"] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
