//! Per-site settings: the site's own host and which requests an import stores.
//!
//! Filters apply while importing (like Screaming Frog's include/exclude and parameter
//! removal), so they save disk space and time; requests already stored are not changed.

use std::borrow::Cow;

use regex::RegexSet;
use seonexus_botverify::BotCategory;
use seonexus_logparse::LogRecord;
use seonexus_store::{BotTag, Catalog, SiteStore, StoreError};
use serde::{Deserialize, Serialize};

use crate::workspace::Workspace;

/// Longest accepted pattern list entry, to keep regular expressions cheap.
const MAX_PATTERN_LEN: usize = 500;

/// Which requests are stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficScope {
    /// Every request (browsers included).
    #[default]
    All,
    /// Requests from identified bots only (much smaller imports).
    Bots,
}

/// Settings of one site.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteSettings {
    /// The site's own host (e.g. `www.acme.com`); referers from it and its subdomains count
    /// as internal. Empty: the site name when it is a host name.
    pub host: Option<String>,
    /// Which requests are stored.
    pub traffic: TrafficScope,
    /// With [`TrafficScope::Bots`]: only these bot categories (`search`, `ai-training`, …);
    /// empty means every bot.
    pub bot_categories: Vec<String>,
    /// Store only URLs matching at least one of these regular expressions (empty: all).
    pub include_urls: Vec<String>,
    /// Never store URLs matching one of these regular expressions.
    pub exclude_urls: Vec<String>,
    /// Query parameters removed from URLs before storing (e.g. `sid`, `jsessionid`).
    pub strip_parameters: Vec<String>,
    /// Also remove well-known tracking parameters (`utm_*`, `gclid`, `fbclid`, …).
    pub strip_tracking_parameters: bool,
}

/// Invalid settings.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// The host is not a host name.
    #[error("`{0}` is not a host name (e.g. www.example.com)")]
    InvalidHost(String),
    /// Unknown bot category.
    #[error("unknown bot category `{0}`")]
    UnknownCategory(String),
    /// A URL pattern is not a valid regular expression.
    #[error("invalid URL pattern `{pattern}`: {reason}")]
    InvalidPattern {
        /// The pattern.
        pattern: String,
        /// Why it was rejected.
        reason: String,
    },
    /// A parameter name is empty or contains `=`, `&` or `?`.
    #[error("invalid parameter name `{0}`")]
    InvalidParameter(String),
    /// Stored settings could not be read.
    #[error("stored settings are invalid: {0}")]
    Corrupt(String),
    /// Storage failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl SiteSettings {
    /// Trims and de-duplicates entries and checks every value.
    ///
    /// # Errors
    /// See [`SettingsError`].
    pub fn normalized(mut self) -> Result<Self, SettingsError> {
        self.host = match self.host.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(host) => Some(
                normalize_host(host).ok_or_else(|| SettingsError::InvalidHost(host.to_owned()))?,
            ),
        };
        for category in &mut self.bot_categories {
            *category = category.to_ascii_lowercase();
        }
        clean(&mut self.bot_categories);
        if let Some(unknown) = self
            .bot_categories
            .iter()
            .find(|category| BotCategory::from_name(category).is_none())
        {
            return Err(SettingsError::UnknownCategory(unknown.clone()));
        }
        clean(&mut self.include_urls);
        clean(&mut self.exclude_urls);
        for pattern in self.include_urls.iter().chain(&self.exclude_urls) {
            compile(std::slice::from_ref(pattern))?;
        }
        clean(&mut self.strip_parameters);
        if let Some(bad) = self
            .strip_parameters
            .iter()
            .find(|name| name.contains(['=', '&', '?', ' ']))
        {
            return Err(SettingsError::InvalidParameter(bad.clone()));
        }
        Ok(self)
    }

    /// Whether imports store every request unchanged.
    pub fn filters_nothing(&self) -> bool {
        self.traffic == TrafficScope::All
            && self.include_urls.is_empty()
            && self.exclude_urls.is_empty()
            && self.strip_parameters.is_empty()
            && !self.strip_tracking_parameters
    }

    pub(crate) fn from_json(json: &str) -> Result<Self, SettingsError> {
        serde_json::from_str(json).map_err(|e| SettingsError::Corrupt(e.to_string()))
    }
}

/// Settings of `site`.
///
/// # Errors
/// [`SettingsError::Store`] (e.g. unknown site) or [`SettingsError::Corrupt`].
pub fn site_settings(workspace: &Workspace, site: &str) -> Result<SiteSettings, SettingsError> {
    let site = Catalog::open(workspace.data_dir())?.site_by_name(site)?;
    SiteSettings::from_json(&site.settings)
}

/// Validates and stores the settings of `site`; returns them normalized.
///
/// # Errors
/// See [`SettingsError`]; [`StoreError::SiteLocked`] while the site is being imported.
pub fn set_site_settings(
    workspace: &Workspace,
    site: &str,
    settings: SiteSettings,
) -> Result<SiteSettings, SettingsError> {
    let settings = settings.normalized()?;
    let catalog = Catalog::open(workspace.data_dir())?;
    let site = catalog.site_by_name(site)?;
    let json =
        serde_json::to_string(&settings).map_err(|e| SettingsError::Corrupt(e.to_string()))?;
    catalog.set_site_settings(site.id, &json)?;
    drop(catalog);
    // Internal-referer detection reads the host from the site's own database.
    if SiteStore::exists(workspace.data_dir(), site.id) {
        let store = SiteStore::open(workspace.data_dir(), site.id, workspace.settings())?;
        store.set_site_host(effective_host(&site.name, &settings).as_deref())?;
    }
    Ok(settings)
}

/// The configured host, or the site name when that is a host name.
pub(crate) fn effective_host(site_name: &str, settings: &SiteSettings) -> Option<String> {
    settings.host.clone().or_else(|| normalize_host(site_name))
}

/// `www.Acme.com.` → `acme.com`; `None` unless it is a dotted host name.
fn normalize_host(name: &str) -> Option<String> {
    let host = name.trim().trim_end_matches('.').to_ascii_lowercase();
    let host = host
        .strip_prefix("https://")
        .or_else(|| host.strip_prefix("http://"))
        .unwrap_or(&host);
    let host = host.split(['/', ':']).next().unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    let valid = host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    valid.then(|| host.to_owned())
}

fn clean(list: &mut Vec<String>) {
    for item in list.iter_mut() {
        *item = item.trim().to_owned();
    }
    list.retain(|item| !item.is_empty());
    let mut seen = std::collections::HashSet::new();
    list.retain(|item| seen.insert(item.clone()));
}

fn compile(patterns: &[String]) -> Result<Option<RegexSet>, SettingsError> {
    if patterns.is_empty() {
        return Ok(None);
    }
    if let Some(long) = patterns.iter().find(|p| p.len() > MAX_PATTERN_LEN) {
        return Err(SettingsError::InvalidPattern {
            pattern: long.clone(),
            reason: format!("longer than {MAX_PATTERN_LEN} characters"),
        });
    }
    regex::RegexSetBuilder::new(patterns)
        .size_limit(1 << 22)
        .build()
        .map(Some)
        .map_err(|error| SettingsError::InvalidPattern {
            pattern: patterns.join(" | "),
            reason: error.to_string(),
        })
}

/// Import-time form of [`SiteSettings`]; shared by the parsing threads.
#[derive(Debug, Default)]
pub(crate) struct ImportFilter {
    bots_only: bool,
    categories: Vec<String>,
    include: Option<RegexSet>,
    exclude: Option<RegexSet>,
    strip: Vec<String>,
    strip_tracking: bool,
}

/// What happens to a parsed request.
pub(crate) enum Decision<'a> {
    /// Store the record as parsed.
    Keep,
    /// Store this rewritten record (parameters removed).
    Rewrite(Box<LogRecord<'a>>),
    /// Do not store.
    Drop,
}

impl ImportFilter {
    pub(crate) fn new(settings: &SiteSettings) -> Result<Self, SettingsError> {
        Ok(Self {
            bots_only: settings.traffic == TrafficScope::Bots,
            categories: settings.bot_categories.clone(),
            include: compile(&settings.include_urls)?,
            exclude: compile(&settings.exclude_urls)?,
            strip: settings
                .strip_parameters
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect(),
            strip_tracking: settings.strip_tracking_parameters,
        })
    }

    /// Decides about one record classified as `bot`.
    pub(crate) fn decide<'a>(&self, record: &LogRecord<'a>, bot: Option<BotTag>) -> Decision<'a> {
        if self.bots_only {
            match bot {
                None => return Decision::Drop,
                Some(tag)
                    if !self.categories.is_empty()
                        && !self.categories.iter().any(|c| c == tag.category) =>
                {
                    return Decision::Drop;
                }
                Some(_) => {}
            }
        }
        let rewritten = if self.strip.is_empty() && !self.strip_tracking {
            None
        } else {
            seonexus_url::strip_parameters(&record.target, |name| {
                (self.strip_tracking && seonexus_url::is_tracking_parameter(name))
                    || self.strip.iter().any(|s| s.eq_ignore_ascii_case(name))
            })
        };
        let target = rewritten.as_deref().unwrap_or(&record.target);
        if self
            .include
            .as_ref()
            .is_some_and(|set| !set.is_match(target))
            || self
                .exclude
                .as_ref()
                .is_some_and(|set| set.is_match(target))
        {
            return Decision::Drop;
        }
        match rewritten {
            None => Decision::Keep,
            Some(target) => Decision::Rewrite(Box::new(LogRecord {
                target: Cow::Owned(target),
                ..record.clone()
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(target: &str) -> LogRecord<'_> {
        LogRecord {
            timestamp: seonexus_core::Timestamp::from_unix_micros(0),
            client_ip: "10.0.0.1".parse().unwrap(),
            method: "GET".into(),
            target: target.into(),
            protocol: "HTTP/1.1".into(),
            status: 200,
            bytes: None,
            referer: None,
            user_agent: None,
            duration_micros: None,
            host: None,
            router: None,
            upstream: None,
            request_seq: None,
        }
    }

    const GOOGLE: BotTag = BotTag {
        id: "googlebot-smartphone",
        category: "search",
    };
    const GPT: BotTag = BotTag {
        id: "gptbot",
        category: "ai-training",
    };

    #[test]
    fn normalizes_and_validates() {
        let settings = SiteSettings {
            host: Some(" https://WWW.Acme.com/ ".into()),
            bot_categories: vec!["Search".into(), " ".into(), "search".into()],
            include_urls: vec!["^/blog/".into()],
            strip_parameters: vec![" sid ".into()],
            ..SiteSettings::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(settings.host.as_deref(), Some("acme.com"));
        assert_eq!(settings.bot_categories, ["search"]);
        assert_eq!(settings.strip_parameters, ["sid"]);

        let bad = |settings: SiteSettings| settings.normalized().unwrap_err();
        assert!(matches!(
            bad(SiteSettings {
                host: Some("not a host".into()),
                ..SiteSettings::default()
            }),
            SettingsError::InvalidHost(_)
        ));
        assert!(matches!(
            bad(SiteSettings {
                bot_categories: vec!["robots".into()],
                ..SiteSettings::default()
            }),
            SettingsError::UnknownCategory(_)
        ));
        assert!(matches!(
            bad(SiteSettings {
                exclude_urls: vec!["(".into()],
                ..SiteSettings::default()
            }),
            SettingsError::InvalidPattern { .. }
        ));
        assert!(matches!(
            bad(SiteSettings {
                strip_parameters: vec!["a=b".into()],
                ..SiteSettings::default()
            }),
            SettingsError::InvalidParameter(_)
        ));
        assert_eq!(
            effective_host("shop.acme.com", &SiteSettings::default()).as_deref(),
            Some("shop.acme.com")
        );
        assert_eq!(effective_host("Acme web", &SiteSettings::default()), None);
    }

    #[test]
    fn filters_traffic_urls_and_parameters() {
        let filter = ImportFilter::new(&SiteSettings {
            traffic: TrafficScope::Bots,
            bot_categories: vec!["search".into()],
            exclude_urls: vec!["^/admin".into()],
            strip_parameters: vec!["sid".into()],
            strip_tracking_parameters: true,
            ..SiteSettings::default()
        })
        .unwrap();
        assert!(matches!(filter.decide(&record("/a"), None), Decision::Drop));
        assert!(matches!(
            filter.decide(&record("/a"), Some(GPT)),
            Decision::Drop
        ));
        assert!(matches!(
            filter.decide(&record("/admin/x"), Some(GOOGLE)),
            Decision::Drop
        ));
        assert!(matches!(
            filter.decide(&record("/a"), Some(GOOGLE)),
            Decision::Keep
        ));
        let Decision::Rewrite(rewritten) =
            filter.decide(&record("/a?SID=9&utm_source=x&page=2"), Some(GOOGLE))
        else {
            panic!("expected a rewrite");
        };
        assert_eq!(rewritten.target, "/a?page=2");

        let include = ImportFilter::new(&SiteSettings {
            include_urls: vec!["^/blog/".into()],
            ..SiteSettings::default()
        })
        .unwrap();
        assert!(matches!(
            include.decide(&record("/blog/x"), None),
            Decision::Keep
        ));
        assert!(matches!(
            include.decide(&record("/shop/x"), None),
            Decision::Drop
        ));
    }

    #[test]
    fn unknown_settings_fields_are_ignored_and_missing_ones_default() {
        let settings = SiteSettings::from_json(r#"{"traffic":"bots","future":1}"#).unwrap();
        assert_eq!(settings.traffic, TrafficScope::Bots);
        assert_eq!(settings.include_urls.len(), 0);
        assert!(SiteSettings::default().filters_nothing());
    }
}
