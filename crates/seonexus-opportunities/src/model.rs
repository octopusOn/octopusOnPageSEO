//! Public wire models. Missing observations stay optional, never silently zero-filled.
#![allow(missing_docs)]

use seonexus_core::Date;
use serde::{Deserialize, Serialize};

pub const RULE_VERSION: &str = "2026-10-18.3";
pub const MAX_DATASET_ROWS: usize = 100_000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BusinessGoal {
    #[default]
    AllEvents,
    Event,
    Revenue,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpportunityProject {
    pub id: String,
    pub name: String,
    pub site_url: String,
    pub gsc_property: String,
    pub ga4_property: String,
    pub google_connection: String,
    pub brand_terms: Vec<String>,
    pub goal: BusinessGoal,
    pub event_name: String,
    pub ga4_origin_confirmed: bool,
    pub crawl_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Date,
    pub end: Date,
}
impl DateRange {
    pub fn days(self) -> i32 {
        self.end.days_since_epoch() - self.start.days_since_epoch() + 1
    }
    pub fn valid(self) -> bool {
        (1..=366).contains(&self.days())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub project_id: String,
    pub current: DateRange,
    pub previous: DateRange,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub path_prefix: String,
}
impl Default for AnalysisRequest {
    fn default() -> Self {
        let end = Date::today().add_days(-3);
        Self {
            project_id: String::new(),
            current: DateRange {
                start: end.add_days(-27),
                end,
            },
            previous: DateRange {
                start: end.add_days(-55),
                end: end.add_days(-28),
            },
            country: String::new(),
            device: String::new(),
            brand: String::new(),
            path_prefix: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataSource {
    Gsc,
    Ga4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataGrain {
    Pages,
    QueryPages,
    Queries,
    Daily,
    LandingPages,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceRow {
    pub url: Option<String>,
    pub query: Option<String>,
    pub date: Option<Date>,
    pub country: Option<String>,
    pub device: Option<String>,
    pub clicks: Option<f64>,
    pub impressions: Option<f64>,
    pub position: Option<f64>,
    pub sessions: Option<f64>,
    pub key_events: Option<f64>,
    pub goal_rate: Option<f64>,
    pub revenue: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DataCoverage {
    pub limited: bool,
    pub warnings: Vec<String>,
    pub timezone: String,
    pub currency: Option<String>,
    pub google_organic: bool,
    pub origin_confirmed: bool,
    pub goal_key: String,
    pub dimensions: Vec<String>,
    pub site_url: String,
    pub property: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDataset {
    pub id: String,
    pub project_id: String,
    pub source: DataSource,
    pub grain: DataGrain,
    pub range: DateRange,
    pub coverage: DataCoverage,
    pub rows: Vec<PerformanceRow>,
    pub origin: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub clicks: f64,
    pub impressions: f64,
    pub ctr: Option<f64>,
    pub position: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryComparison {
    pub query: String,
    pub current: Metrics,
    pub previous: Option<Metrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunitySignal {
    pub kind: String,
    pub query: Option<String>,
    pub current: Metrics,
    pub previous: Option<Metrics>,
    pub benchmark_ctr: Option<f64>,
    pub peer_urls: usize,
    pub estimated_clicks: Option<f64>,
    pub review_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalObservation {
    pub sessions: f64,
    pub key_events: Option<f64>,
    pub goal_rate: Option<f64>,
    pub revenue: Option<f64>,
    pub currency: Option<String>,
    pub boost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub url: String,
    pub current: Metrics,
    pub previous: Option<Metrics>,
    pub queries: Vec<QueryComparison>,
    pub signals: Vec<OpportunitySignal>,
    pub confidence: String,
    pub priority: f64,
    pub priority_basis: String,
    pub impact_clicks: Option<f64>,
    pub business: Option<GoalObservation>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub source: DataSource,
    pub grain: DataGrain,
    pub range: DateRange,
    pub rows: usize,
    pub coverage: DataCoverage,
    pub origin: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyObservation {
    pub date: Date,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTotals {
    pub sessions: Option<f64>,
    pub key_events: Option<f64>,
    pub revenue: Option<f64>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityReport {
    pub rule_version: String,
    pub request: AnalysisRequest,
    pub current: Option<Metrics>,
    pub previous: Option<Metrics>,
    pub sessions: Option<f64>,
    pub key_events: Option<f64>,
    pub revenue: Option<f64>,
    pub currency: Option<String>,
    pub opportunities: Vec<Opportunity>,
    pub coverage: Vec<CoverageSummary>,
    pub warnings: Vec<String>,
    pub previous_ga4: Option<GoalTotals>,
    pub unknown_current_urls: usize,
    pub daily: Vec<DailyObservation>,
}

impl OpportunityProject {
    pub fn goal_key(&self) -> String {
        match self.goal {
            BusinessGoal::AllEvents => "events:any".into(),
            BusinessGoal::Event => format!("event:{}", self.event_name),
            BusinessGoal::Revenue => "purchase-revenue".into(),
        }
    }
}
