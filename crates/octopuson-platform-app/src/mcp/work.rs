//! The MCP tools for the work around the data: the project's dashboard, what to do next, alerts,
//! keyword lists, content briefs, link work and Core Web Vitals (free), and the actions an
//! assistant may take for the user — tracking keywords (free), checking rankings, auditing the
//! site, running the AI questions and making a content brief. A paid action first answers its
//! price (`needs_approval`) and runs only with an allowance the user agreed to.

use serde_json::{Value, json};

use super::tools::{answer, call_failure, failure, project_property, schema, tool};
use crate::Platform;
use crate::router::CallError;

/// Most keywords tracked in one call.
const MAX_TRACKED: usize = 500;

fn lang_property() -> Value {
    json!({ "type": "string", "enum": ["en", "tr", "de", "es"], "default": "en", "description": "Language of the texts." })
}

fn allow_property() -> Value {
    json!({ "type": "number", "minimum": 0, "description": "Most US dollars of provider price this call may spend. Without it nothing is started: the answer gives the price. Only pass what the user approved." })
}

/// The tools of this module, as `tools/list` gives them.
#[allow(clippy::too_many_lines)] // the tool list
pub(super) fn definitions() -> Vec<Value> {
    vec![
        tool(
            "project_dashboard",
            "Project dashboard",
            "One project's headline numbers from every module with their trend (organic traffic and keywords, backlinks, rank tracking, Search Console, site audit, AI visibility, local SEO, Core Web Vitals, content), the first opportunities, what the schedule runs next and this month's spending. Free.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    ("lang", lang_property()),
                ],
            ),
            true,
        ),
        tool(
            "opportunities",
            "Opportunities",
            "What to do next on a project, the most valuable for the least work first: keywords close to the top, low click-through rates, pages losing clicks, cannibalised queries, audit problems, orphan and weakly linked pages, Core Web Vitals, AI answers naming a rival, weak content, the map grid, growing competitors and lost links — each with the estimated extra clicks a month, the effort and the page that acts on it. Free (rules over the stored data).",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    ("lang", lang_property()),
                    (
                        "all",
                        json!({ "type": "boolean", "description": "Also those marked done, ignored or snoozed." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "alerts",
            "Alerts",
            "Alerts the platform found, newest first: keywords that fell or left the top 10, visibility and Search Console click drops, site health drops and new high-priority audit problems, AI mention changes, map grid drops, new bad reviews, fast-growing competitors and linking sites won or lost. Of one project or all. Free.",
            schema(
                &[],
                vec![
                    (
                        "project_id",
                        json!({ "type": "integer", "description": "Project id from list_projects; every project's when absent." }),
                    ),
                    (
                        "unread",
                        json!({ "type": "boolean", "description": "Only those not read yet." }),
                    ),
                    (
                        "limit",
                        json!({ "type": "integer", "minimum": 1, "maximum": 500, "default": 50 }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "keyword_lists",
            "Keyword lists",
            "The project's saved keyword lists with their sizes, or (with list_id) one list's keywords with the volume, difficulty, CPC and intent saved with them. Free.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "list_id",
                        json!({ "type": "integer", "description": "A list from the lists answer." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "content_briefs",
            "Content briefs",
            "The project's content briefs, or (with brief_id) one brief: the target keyword, the top results read, suggested terms, headings, questions, length, internal links, the AI-written brief when one was made, and the page scores against it. Free.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "brief_id",
                        json!({ "type": "integer", "description": "A brief from the briefs answer." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "link_work",
            "Link work",
            "The user's own link work on a project: the decisions taken after the toxic-link audit (keep, watch, disavow), the disavow file Google takes, and link prospects with their outreach status. Free.",
            schema(&["project_id"], vec![("project_id", project_property())]),
            true,
        ),
        tool(
            "core_web_vitals",
            "Core Web Vitals",
            "The latest Core Web Vitals of the project's site from Chrome users (CrUX: LCP, INP, CLS on phones and desktops, with their trend) and PageSpeed lab scores of its pages. Free (stored checks).",
            schema(&["project_id"], vec![("project_id", project_property())]),
            true,
        ),
        tool(
            "metric_history",
            "Metric history",
            "The headline numbers of the project's domain (or another) on each day they were fetched here: organic traffic, keywords and position bands (kind domain), or authority, backlinks and referring domains (kind backlinks). The program's own record beside the providers' histories. Free.",
            schema(
                &["project_id", "kind"],
                vec![
                    ("project_id", project_property()),
                    (
                        "kind",
                        json!({ "type": "string", "enum": ["domain", "backlinks"] }),
                    ),
                    (
                        "target",
                        json!({ "type": "string", "description": "Another domain or page than the project's." }),
                    ),
                    (
                        "market",
                        json!({ "type": "string", "pattern": "^[a-z]{2}-[a-z]{2,3}$", "description": "Domain numbers of another market: country-language." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "track_keywords",
            "Track keywords",
            "Adds keywords to the project's rank tracker (every place it tracks). Adding is free; each later check costs (see check_rankings).",
            schema(
                &["project_id", "keywords"],
                vec![
                    ("project_id", project_property()),
                    (
                        "keywords",
                        json!({ "type": "array", "items": { "type": "string" }, "minItems": 1, "maxItems": MAX_TRACKED }),
                    ),
                    (
                        "tags",
                        json!({ "type": "array", "items": { "type": "string" }, "description": "Tags to group them by." }),
                    ),
                ],
            ),
            false,
        ),
        tool(
            "check_rankings",
            "Check rankings now",
            "Checks every tracked keyword of the project now (live result pages), in the background; rank_report shows the result. Without allow_spend_usd nothing is started: the answer gives the price. Paid.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    ("allow_spend_usd", allow_property()),
                ],
            ),
            false,
        ),
        tool(
            "start_site_audit",
            "Audit the site",
            "Crawls the project's site now with the platform's own crawler, in the background; site_audit_issues shows the result. Free on the desktop; on the web crawled pages beyond the month's free ones are metered, so the answer gives the price first. Paid on the web.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    ("allow_spend_usd", allow_property()),
                ],
            ),
            false,
        ),
        tool(
            "run_ai_visibility",
            "Ask the AI questions",
            "Asks the project's AI visibility questions on every chosen assistant now, in the background; ai_visibility shows the answers. Without allow_spend_usd nothing is started: the answer gives the price. Paid.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    ("allow_spend_usd", allow_property()),
                ],
            ),
            false,
        ),
        tool(
            "create_content_brief",
            "Make a content brief",
            "Makes a content brief for a keyword in the background: reads the top results of its result page and suggests terms, headings, questions, length and internal links; content_briefs shows it. The result page is bought unless the Keyword Explorer keeps it fresh. Paid.",
            schema(
                &["project_id", "keyword"],
                vec![
                    ("project_id", project_property()),
                    ("keyword", json!({ "type": "string" })),
                    (
                        "market",
                        json!({ "type": "string", "pattern": "^[a-z]{2}-[a-z]{2,3}$", "description": "Another market than the project's: country-language, like de-de." }),
                    ),
                    (
                        "mobile",
                        json!({ "type": "boolean", "description": "The mobile result page." }),
                    ),
                    (
                        "page",
                        json!({ "type": "string", "description": "A page of the project to score against the brief once made, a full address." }),
                    ),
                    ("allow_spend_usd", allow_property()),
                ],
            ),
            false,
        ),
    ]
}

fn project_of(arguments: &Value) -> Result<i64, String> {
    arguments
        .get("project_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| "project_id is required: call list_projects for the ids".to_owned())
}

fn lang_of(arguments: &Value) -> Value {
    arguments
        .get("lang")
        .cloned()
        .unwrap_or_else(|| json!("en"))
}

fn allowance(arguments: &Value) -> Option<f64> {
    arguments.get("allow_spend_usd").and_then(Value::as_f64)
}

/// The price of an action the user has not agreed to yet.
fn needs_approval(name: &str, usd: f64, details: &Value) -> Value {
    json!({
        "status": "needs_approval",
        "price": details,
        "message": format!(
            "This costs about ${usd:.4} of provider price. Ask the user; if they agree, call {name} again with allow_spend_usd: {usd:.4} (or more)."
        ),
    })
}

/// Why an action cannot be paid now, from an estimate's `blocked`.
fn blocked_message(blocked: &Value) -> String {
    format!(
        "this cannot be paid now: {} — the user can enter their own key or connect their OctopusOn SEO account under Sources",
        blocked
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no source")
    )
}

fn usd_of(value: &Value) -> f64 {
    value.get("usd").and_then(Value::as_f64).unwrap_or_default()
}

fn covered(allow: Option<f64>, usd: f64) -> bool {
    usd <= 0.0 || allow.is_some_and(|allow| allow + 1e-9 >= usd)
}

fn started(job: &Value, usd: f64, message: &str) -> Value {
    json!({ "status": "started", "job_id": job, "price_usd": usd, "message": message })
}

fn dashboard(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    platform.call(
        "project_dashboard",
        json!({ "project_id": project, "lang": lang_of(arguments) }),
    )
}

fn opportunities(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    let all = arguments
        .get("all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    platform.call(
        "opportunities",
        json!({ "project_id": project, "lang": lang_of(arguments), "all": all }),
    )
}

fn alerts(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let mut ask = json!({
        "unread": arguments.get("unread").and_then(Value::as_bool).unwrap_or(false),
        "limit": arguments.get("limit").and_then(Value::as_u64).unwrap_or(50),
    });
    if let Some(project) = arguments.get("project_id").and_then(Value::as_i64) {
        ask["project_id"] = json!(project);
    }
    let list = platform.call("list_alerts", ask)?;
    Ok(json!({
        "unread": list.get("unread"),
        "unread_by_project": list.get("by_project"),
        "alerts": list.get("alerts"),
        "note": "Each alert's `kind` names the rule and `detail` its numbers; `page` and `params` are where the platform shows it.",
    }))
}

fn keyword_lists(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    match arguments.get("list_id").and_then(Value::as_i64) {
        Some(list) => {
            let lists = platform.call("keyword_lists", json!({ "project_id": project }))?;
            let known = lists
                .as_array()
                .into_iter()
                .flatten()
                .find(|row| row.get("id").and_then(Value::as_i64) == Some(list));
            let Some(known) = known.cloned() else {
                return Err(invalid(format!(
                    "list {list} is not a list of this project"
                )));
            };
            Ok(json!({
                "list": known,
                "keywords": platform.call("list_keywords", json!({ "list_id": list }))?,
            }))
        }
        None => Ok(json!({
            "lists": platform.call("keyword_lists", json!({ "project_id": project }))?,
        })),
    }
}

fn briefs(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    match arguments.get("brief_id").and_then(Value::as_i64) {
        Some(brief) => {
            let detail = platform.call("content_brief", json!({ "brief_id": brief }))?;
            if detail.get("project_id").and_then(Value::as_i64) != Some(project) {
                return Err(invalid(format!(
                    "brief {brief} is not a brief of this project"
                )));
            }
            Ok(detail)
        }
        None => Ok(json!({
            "briefs": platform.call("content_briefs", json!({ "project_id": project }))?,
        })),
    }
}

fn link_work(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    let at = json!({ "project_id": project });
    let disavow = platform.call("disavow_file", at.clone())?;
    Ok(json!({
        "decisions": platform.call("link_decisions", at.clone())?,
        "prospects": platform.call("link_prospects", at)?,
        "disavow_file": disavow,
    }))
}

fn vitals(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    let report = platform.call("vitals_report", json!({ "project_id": project }))?;
    if report.is_null() {
        return Ok(
            json!({ "message": "This project has no Core Web Vitals checks yet; the user can run them on the Site audit page (Core Web Vitals tab) with a Google API key." }),
        );
    }
    Ok(report)
}

fn metric_history(platform: &Platform, arguments: &Value) -> Result<Value, CallError> {
    let project = project_of(arguments).map_err(invalid)?;
    let mut ask = json!({ "project_id": project, "kind": arguments.get("kind").cloned().unwrap_or(Value::Null) });
    for name in ["target", "market"] {
        if let Some(value) = arguments.get(name) {
            ask[name] = value.clone();
        }
    }
    Ok(json!({ "points": platform.call("metric_history", ask)? }))
}

fn track(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let project = project_of(arguments)?;
    let keywords: Vec<Value> = arguments
        .get("keywords")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
        .take(MAX_TRACKED)
        .map(|keyword| json!({ "keyword": keyword }))
        .collect();
    if keywords.is_empty() {
        return Err("keywords: give at least one keyword".to_owned());
    }
    let mut ask = json!({ "project_id": project, "keywords": keywords });
    if let Some(tags) = arguments.get("tags") {
        ask["tags"] = tags.clone();
    }
    let tracked = platform
        .call("track_keywords", ask)
        .map_err(|error| error.message)?;
    Ok(json!({
        "status": "tracked",
        "added": tracked.get("added"),
        "tracked_now": tracked.get("total"),
        "message": "The keywords are tracked; the schedule checks them, or check_rankings checks them now (paid).",
    }))
}

fn check_rankings(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let project = project_of(arguments)?;
    let estimate = platform
        .call("rank_estimate", json!({ "project_id": project }))
        .map_err(|error| error.message)?;
    if estimate.get("checks").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return Err("no keywords are tracked yet: add some with track_keywords".to_owned());
    }
    let run = estimate.get("run").cloned().unwrap_or(Value::Null);
    if let Some(blocked) = run.get("blocked").filter(|blocked| !blocked.is_null()) {
        return Err(blocked_message(blocked));
    }
    let usd = usd_of(&run);
    if !covered(allowance(arguments), usd) {
        return Ok(needs_approval(
            "check_rankings",
            usd,
            &json!({ "usd": usd, "credits": run.get("credits"), "paid_with": run.get("via"), "checks": estimate.get("checks") }),
        ));
    }
    let job = platform
        .call("start_rank_check", json!({ "project_id": project }))
        .map_err(|error| error.message)?;
    Ok(started(
        &job,
        usd,
        "The rankings are being checked in the background; rank_report shows them when the check ends.",
    ))
}

/// What an audit may cost on the web: every page up to the limit, at the metered price.
fn audit_price(setup: &Value) -> Option<f64> {
    let meter = setup.get("meter").filter(|meter| !meter.is_null())?;
    let pages = setup
        .pointer("/settings/max_pages")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let micros = meter
        .get("micros_per_unit")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    Some((pages * micros / 1_000_000.0 * 10_000.0).round() / 10_000.0)
}

fn start_audit(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let project = project_of(arguments)?;
    let setup = platform
        .call("audit_setup", json!({ "project_id": project }))
        .map_err(|error| error.message)?;
    let usd = audit_price(&setup).unwrap_or_default();
    if !covered(allowance(arguments), usd) {
        let mut asked = needs_approval(
            "start_site_audit",
            usd,
            &json!({
                "usd_at_most": usd,
                "pages_at_most": setup.pointer("/settings/max_pages"),
                "free_pages_each_month": setup.pointer("/meter/free_per_month"),
            }),
        );
        asked["message"] = json!(format!(
            "Crawling up to {} pages costs at most ${usd:.4} (the month's free pages first). Ask the user; if they agree, call start_site_audit again with allow_spend_usd: {usd:.4} (or more).",
            setup
                .pointer("/settings/max_pages")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        return Ok(asked);
    }
    let job = platform
        .call("start_audit", json!({ "project_id": project }))
        .map_err(|error| error.message)?;
    Ok(started(
        &job,
        usd,
        "The site is being crawled in the background; site_audit_issues shows the audit when it ends.",
    ))
}

fn run_ai(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let project = project_of(arguments)?;
    let estimate = platform
        .call("ai_estimate", json!({ "project_id": project }))
        .map_err(|error| error.message)?;
    if estimate.get("answers").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return Err(
            "no questions or assistants are set up: the user adds them on the AI visibility page"
                .to_owned(),
        );
    }
    let usd = usd_of(&estimate);
    if !covered(allowance(arguments), usd) {
        return Ok(needs_approval(
            "run_ai_visibility",
            usd,
            &json!({
                "usd": usd,
                "credits": estimate.get("credits"),
                "answers": estimate.get("answers"),
                "assistants": estimate.get("engines"),
                "not_available": estimate.get("unavailable"),
            }),
        ));
    }
    let job = platform
        .call("start_ai_run", json!({ "project_id": project }))
        .map_err(|error| error.message)?;
    Ok(started(
        &job,
        usd,
        "The questions are being asked in the background; ai_visibility shows the answers when they are in.",
    ))
}

fn create_brief(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let project = project_of(arguments)?;
    let keyword = arguments
        .get("keyword")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
        .ok_or_else(|| "keyword is required".to_owned())?;
    let mut ask = json!({ "project_id": project, "keyword": keyword });
    for name in ["market", "mobile", "page"] {
        if let Some(value) = arguments.get(name) {
            ask[name] = value.clone();
        }
    }
    let estimate = platform
        .call("content_brief_estimate", ask.clone())
        .map_err(|error| error.message)?;
    let fresh = estimate.pointer("/kept/fresh").and_then(Value::as_bool) == Some(true);
    let usd = if fresh { 0.0 } else { usd_of(&estimate) };
    if !fresh && let Some(blocked) = estimate.get("blocked").filter(|blocked| !blocked.is_null()) {
        return Err(blocked_message(blocked));
    }
    if !covered(allowance(arguments), usd) {
        return Ok(needs_approval(
            "create_content_brief",
            usd,
            &json!({ "usd": usd, "credits": estimate.get("credits"), "paid_with": estimate.get("via") }),
        ));
    }
    let made = platform
        .call("start_content_brief", ask)
        .map_err(|error| error.message)?;
    Ok(json!({
        "status": "started",
        "job_id": made.get("job_id"),
        "brief_id": made.get("brief_id"),
        "price_usd": usd,
        "message": "The brief is being made in the background (about a minute); content_briefs with this brief_id shows it.",
    }))
}

fn invalid(message: impl Into<String>) -> CallError {
    CallError {
        code: crate::router::ErrorCode::Invalid,
        message: message.into(),
        retry_after: None,
    }
}

/// Calls a tool of this module; `None` for another tool.
pub(super) fn call(platform: &Platform, name: &str, arguments: &Value) -> Option<Value> {
    let read = |outcome: Result<Value, CallError>| match outcome {
        Ok(value) => answer(value),
        Err(error) => call_failure(&error),
    };
    let act = |outcome: Result<Value, String>| match outcome {
        Ok(value) => answer(value),
        Err(message) => failure(&message),
    };
    Some(match name {
        "project_dashboard" => read(dashboard(platform, arguments)),
        "opportunities" => read(opportunities(platform, arguments)),
        "alerts" => read(alerts(platform, arguments)),
        "keyword_lists" => read(keyword_lists(platform, arguments)),
        "content_briefs" => read(briefs(platform, arguments)),
        "link_work" => read(link_work(platform, arguments)),
        "core_web_vitals" => read(vitals(platform, arguments)),
        "metric_history" => read(metric_history(platform, arguments)),
        "track_keywords" => act(track(platform, arguments)),
        "check_rankings" => act(check_rankings(platform, arguments)),
        "start_site_audit" => act(start_audit(platform, arguments)),
        "run_ai_visibility" => act(run_ai(platform, arguments)),
        "create_content_brief" => act(create_brief(platform, arguments)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_audit_is_priced_only_where_pages_are_metered() {
        assert_eq!(
            audit_price(&json!({ "meter": null, "settings": { "max_pages": 500 } })),
            None
        );
        let web = json!({ "meter": { "micros_per_unit": 200, "free_per_month": 1000 }, "settings": { "max_pages": 5000 } });
        assert_eq!(audit_price(&web), Some(1.0));
        assert!(covered(None, 0.0), "free work needs no allowance");
        assert!(!covered(None, 0.5));
        assert!(covered(Some(0.5), 0.5));
    }
}
