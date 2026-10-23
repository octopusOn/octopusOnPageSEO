//! Metric history: each fetched domain or link summary leaves the day's headline numbers, so a
//! trend grows from the user's own fetches beside the providers' histories (`metric_history`).

use octopuson_dataforseo::Market;
use octopuson_platform::{MetricPoint, normalize_domain};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::fetch::Fetched;
use crate::router::{Context, Router};
use crate::{AppError, Workspace};

/// A market as points keep it.
fn market_name(market: &Market) -> String {
    format!("{}-{}", market.country, market.language)
}

/// The day of an RFC 3339 time.
fn day_of(time: &str) -> Option<&str> {
    time.get(0..10)
}

fn note(workspace: &Workspace, project_id: i64, point: &MetricPoint) {
    let noted = workspace
        .store()
        .and_then(|store| Ok(store.note_metrics(project_id, point)?));
    if let Err(error) = noted {
        tracing::warn!(project = project_id, %error, "a metric point was not kept");
    }
}

/// Keeps the day's numbers of a domain overview (`data` as the overview reads it).
pub(crate) fn note_domain(
    workspace: &Workspace,
    project_id: i64,
    domain: &str,
    market: &Market,
    fetched: &Fetched<Value>,
) {
    let Some(day) = day_of(&fetched.fetched_at) else {
        return;
    };
    let metrics = json!({
        "organic": fetched.data.get("organic").cloned().unwrap_or(Value::Null),
        "paid": fetched.data.get("paid").cloned().unwrap_or(Value::Null),
    });
    let point = MetricPoint {
        target: domain.to_owned(),
        kind: "domain".to_owned(),
        market: market_name(market),
        day: day.to_owned(),
        metrics,
        taken_at: fetched.fetched_at.clone(),
    };
    note(workspace, project_id, &point);
}

/// Keeps the day's numbers of a link profile (`data` as the backlinks summary reads it).
pub(crate) fn note_links(
    workspace: &Workspace,
    project_id: i64,
    target: &str,
    fetched: &Fetched<Value>,
) {
    let Some(day) = day_of(&fetched.fetched_at) else {
        return;
    };
    let pick = |name: &str| fetched.data.get(name).cloned().unwrap_or(Value::Null);
    let metrics = json!({
        "rank": pick("rank"),
        "backlinks": pick("backlinks"),
        "referring_domains": pick("referring_domains"),
        "referring_main_domains": pick("referring_main_domains"),
        "referring_pages": pick("referring_pages"),
        "broken_backlinks": pick("broken_backlinks"),
        "spam_score": pick("spam_score"),
    });
    let point = MetricPoint {
        target: target.to_owned(),
        kind: "backlinks".to_owned(),
        market: String::new(),
        day: day.to_owned(),
        metrics,
        taken_at: fetched.fetched_at.clone(),
    };
    note(workspace, project_id, &point);
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    /// `domain` or `backlinks`.
    kind: String,
    /// Another domain or page than the project's.
    #[serde(default)]
    target: Option<String>,
    /// Another market than the project's (domain numbers): `country-language`.
    #[serde(default)]
    market: Option<String>,
}

/// A target's kept points, oldest first: free, nothing is fetched.
fn metric_history(context: &Context<'_>, ask: Ask) -> Result<Vec<MetricPoint>, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let given = ask
        .target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty());
    let (target, market) = match ask.kind.as_str() {
        "domain" => {
            let target = given.map_or_else(|| project.domain.clone(), normalize_domain);
            (
                target,
                market_name(&super::market_of(&project, ask.market.as_deref())?),
            )
        }
        "backlinks" => (
            octopuson_dataforseo::backlinks::target(given.unwrap_or(&project.domain))?,
            String::new(),
        ),
        other => return Err(AppError::Invalid(format!("kind: `{other}` has no history"))),
    };
    Ok(store.metric_points(project.id, &target, &ask.kind, &market)?)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("metric_history", metric_history);
}
