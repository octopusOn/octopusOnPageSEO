//! The MCP tools: each one calls the platform's own commands and returns their answer, trimmed
//! for an assistant to read.

use serde_json::{Map, Value, json};

use crate::Platform;
use crate::router::CallError;

/// Rows of any list an answer keeps.
const MAX_ROWS: usize = 200;

/// Rows kept when an answer is still too long.
const FEW_ROWS: usize = 40;

/// Longest answer text before lists are cut further.
const MAX_TEXT: usize = 120_000;

/// A paid tool: the module's estimate, kept-answer and fetch commands.
struct PaidTool {
    estimate: &'static str,
    kept: &'static str,
    fetch: &'static str,
}

fn paid_tool(name: &str) -> Option<PaidTool> {
    let (estimate, kept, fetch) = match name {
        "domain_overview" => ("overview_estimate", "get_overview", "fetch_overview"),
        "keyword_research" | "serp_overview" => {
            ("keywords_estimate", "get_keywords", "fetch_keywords")
        }
        "backlinks" => ("backlinks_estimate", "get_backlinks", "fetch_backlinks"),
        "keyword_gap" => ("gap_estimate", "get_gap", "fetch_gap"),
        "ai_mentions" => (
            "ai_mentions_estimate",
            "get_ai_mentions",
            "fetch_ai_mentions",
        ),
        _ => return None,
    };
    Some(PaidTool {
        estimate,
        kept,
        fetch,
    })
}

fn project_property() -> Value {
    json!({ "type": "integer", "description": "Project id from list_projects." })
}

fn spend_properties() -> [(&'static str, Value); 2] {
    [
        (
            "allow_spend_usd",
            json!({ "type": "number", "minimum": 0, "description": "Most US dollars of provider price this call may spend. Without it nothing is bought: the answer gives the price. Only pass what the user approved." }),
        ),
        (
            "refresh",
            json!({ "type": "boolean", "description": "Buy fresh data even while the kept data is fresh (needs allow_spend_usd)." }),
        ),
    ]
}

fn schema(required: &[&str], properties: Vec<(&str, Value)>) -> Value {
    let properties: Map<String, Value> = properties
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

fn tool(name: &str, title: &str, description: &str, input: Value, read_only: bool) -> Value {
    let mut tool = json!({
        "name": name,
        "title": title,
        "description": description,
        "annotations": { "readOnlyHint": read_only, "openWorldHint": !read_only },
    });
    tool["inputSchema"] = input;
    tool
}

fn paid_schema(mut properties: Vec<(&'static str, Value)>, required: &[&str]) -> Value {
    properties.extend(spend_properties());
    schema(required, properties)
}

/// Every tool, as `tools/list` gives them.
#[allow(clippy::too_many_lines)] // the tool list
pub(super) fn definitions() -> Vec<Value> {
    let limit = json!({ "type": "integer", "minimum": 10, "maximum": 1000, "description": "Rows of a table report (100, 500 or 1000)." });
    vec![
        tool(
            "list_projects",
            "Projects",
            "The user's projects: id, name, domain, country, language, competitors and brand names.",
            schema(&[], Vec::new()),
            true,
        ),
        tool(
            "domain_overview",
            "Domain overview",
            "Organic and paid keywords, traffic and traffic value of the project's domain (or another), and its history, ranked keywords, top pages, competitors or subdomains (DataForSEO Labs). Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "domain",
                        json!({ "type": "string", "description": "Another domain to look at, in the project's market." }),
                    ),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["summary", "history", "keywords", "pages", "competitors", "subdomains"], "default": "summary" }),
                    ),
                    ("limit", limit.clone()),
                ],
                &["project_id"],
            ),
            false,
        ),
        tool(
            "keyword_research",
            "Keyword research",
            "Search volume, difficulty, CPC and intent of keywords (overview, up to 700), or keyword ideas, related keywords, suggestions or Yandex Wordstat phrases from a seed, in the project's market. Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["overview", "ideas", "related", "suggestions", "wordstat"], "default": "overview" }),
                    ),
                    (
                        "keywords",
                        json!({ "type": "array", "items": { "type": "string" }, "description": "Keywords to measure (overview) or seeds (ideas)." }),
                    ),
                    (
                        "seed",
                        json!({ "type": "string", "description": "Seed of related keywords, suggestions and Wordstat." }),
                    ),
                    ("limit", limit.clone()),
                ],
                &["project_id"],
            ),
            false,
        ),
        tool(
            "serp_overview",
            "Search results",
            "The live Google result page of a keyword in the project's market: results with their domains, SERP features and AI Overview. Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    ("keyword", json!({ "type": "string" })),
                    (
                        "mobile",
                        json!({ "type": "boolean", "description": "The mobile result page." }),
                    ),
                ],
                &["project_id", "keyword"],
            ),
            false,
        ),
        tool(
            "backlinks",
            "Backlinks",
            "The backlink profile of the project's domain (or a target): summary with authority (OA), backlinks, referring domains, anchors, linked pages, or new and lost links over time. Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["summary", "backlinks", "referring_domains", "anchors", "pages", "new_lost"], "default": "summary" }),
                    ),
                    (
                        "target",
                        json!({ "type": "string", "description": "Another domain or page." }),
                    ),
                    ("limit", limit.clone()),
                ],
                &["project_id"],
            ),
            false,
        ),
        tool(
            "keyword_gap",
            "Keyword and link gap",
            "Keywords competitors rank for and the project's site does not (missing), keywords both rank for (shared), sites linking to competitors but not to the site (links), or many sites compared (bulk). Up to 5 competitors; the project's own by default. Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["missing", "shared", "links", "bulk"], "default": "missing" }),
                    ),
                    (
                        "competitors",
                        json!({ "type": "array", "items": { "type": "string" }, "maxItems": 5 }),
                    ),
                    ("limit", limit.clone()),
                ],
                &["project_id"],
            ),
            false,
        ),
        tool(
            "ai_mentions",
            "LLM Mentions",
            "How often AI answers (Google AI Overview, ChatGPT) mention or cite the project's domain, the questions they answer, or the domain beside its rivals (DataForSEO LLM Mentions). Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["summary", "queries", "compare"], "default": "summary" }),
                    ),
                    ("limit", limit),
                ],
                &["project_id"],
            ),
            false,
        ),
        tool(
            "rank_report",
            "Rank tracking",
            "The rank tracker of a project: visibility and its change, average position, movement, competitor shares and every tracked keyword with its position, change, page and SERP signals. Free (stored checks).",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "days",
                        json!({ "type": "integer", "minimum": 7, "maximum": 365, "default": 90 }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "site_audit_issues",
            "Site audit",
            "The latest site audit of a project: health and GEO readiness scores, crawl overview, AI crawler access, and every check with findings by priority. Free (stored crawl).",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "lang",
                        json!({ "type": "string", "enum": ["en", "tr"], "default": "en", "description": "Language of the check names." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "ai_visibility",
            "AI visibility",
            "How ChatGPT, Claude, Gemini, Perplexity and Google AI Mode answer the project's questions: mention and citation rates, share of voice against rivals, the matrix of questions and assistants, cited domains. Free (stored answers).",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "days",
                        json!({ "type": "integer", "minimum": 7, "maximum": 365, "default": 90 }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "search_console",
            "Search Console",
            "The project's latest Search Console and Google Analytics sync: clicks, impressions, CTR and position against the period before, top queries and pages with changes, cannibalised queries, devices and countries. Free (stored sync).",
            schema(&["project_id"], vec![("project_id", project_property())]),
            true,
        ),
        tool(
            "estimate_cost",
            "Estimate cost",
            "What a paid tool call would cost now, without buying anything: US dollars, credits, how it would be paid, and whether kept data is fresh.",
            schema(
                &["tool", "arguments"],
                vec![
                    (
                        "tool",
                        json!({ "type": "string", "enum": ["domain_overview", "keyword_research", "serp_overview", "backlinks", "keyword_gap", "ai_mentions"] }),
                    ),
                    (
                        "arguments",
                        json!({ "type": "object", "description": "The arguments the tool would be called with." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "spending",
            "Spending",
            "Each paid source with how it is paid (own key or credits), its monthly budget and this month's spending, and spending by month.",
            schema(&[], Vec::new()),
            true,
        ),
    ]
}

/// Cuts every list to `rows`.
fn trim(value: Value, rows: usize) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .take(rows)
                .map(|item| trim(item, rows))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key, trim(item, rows)))
                .collect(),
        ),
        other => other,
    }
}

/// A successful answer: the JSON as text and as structured content.
fn answer(value: Value) -> Value {
    let mut value = trim(value, MAX_ROWS);
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    if text.len() > MAX_TEXT {
        value = trim(value, FEW_ROWS);
        text = serde_json::to_string(&value).unwrap_or_default();
    }
    let structured = if value.is_object() {
        value
    } else {
        json!({ "result": value })
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false,
    })
}

/// A failed call, told to the assistant.
fn failure(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn call_failure(error: &CallError) -> Value {
    failure(&format!(
        "{} ({})",
        error.message,
        json!(error.code).as_str().unwrap_or("error")
    ))
}

/// The arguments of a tool as the platform command takes them; spending fields removed.
fn command_arguments(name: &str, arguments: &Value) -> Result<Value, String> {
    let Some(given) = arguments.as_object() else {
        return Err("arguments must be an object".to_owned());
    };
    let mut out: Map<String, Value> = given
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "allow_spend_usd" | "refresh" | "keyword"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if name != "list_projects" && name != "spending" && !out.contains_key("project_id") {
        return Err("project_id is required: call list_projects for the ids".to_owned());
    }
    if name == "serp_overview" {
        let keyword = given
            .get("keyword")
            .and_then(Value::as_str)
            .filter(|keyword| !keyword.trim().is_empty())
            .ok_or_else(|| "keyword is required".to_owned())?;
        out.insert("report".to_owned(), json!("serp"));
        out.insert("seed".to_owned(), json!(keyword));
    }
    Ok(Value::Object(out))
}

/// A paid tool: kept data while fresh, else the price, else (with an allowance) fresh data.
fn paid(
    platform: &Platform,
    name: &str,
    commands: &PaidTool,
    arguments: &Value,
) -> Result<Value, String> {
    let args = command_arguments(name, arguments)?;
    let allow = arguments.get("allow_spend_usd").and_then(Value::as_f64);
    let refresh = arguments
        .get("refresh")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let estimate = platform
        .call(commands.estimate, args.clone())
        .map_err(|error| error.message)?;
    let fresh = estimate.pointer("/kept/fresh").and_then(Value::as_bool) == Some(true);
    if fresh && !refresh {
        let kept = platform
            .call(commands.kept, args.clone())
            .map_err(|error| error.message)?;
        if !kept.is_null() {
            return Ok(json!({
                "status": "kept",
                "fetched_at": kept.get("fetched_at"),
                "spent_usd": 0,
                "data": kept.get("data"),
            }));
        }
    }
    if let Some(blocked) = estimate.get("blocked").filter(|blocked| !blocked.is_null()) {
        return Err(format!(
            "this data cannot be bought now: {} — the user can enter their own key or connect their OctopusOn SEO account under Sources",
            blocked
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("no source")
        ));
    }
    let usd = estimate
        .get("usd")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    match allow {
        Some(allow) if allow + 1e-9 >= usd => {
            let mut args = args;
            args["force"] = json!(refresh);
            let fetched = platform
                .call(commands.fetch, args)
                .map_err(|error| error.message)?;
            Ok(json!({
                "status": "fetched",
                "fetched_at": fetched.get("fetched_at"),
                "spent_usd": fetched.get("spent_usd"),
                "paid_with": fetched.get("via"),
                "data": fetched.get("data"),
            }))
        }
        _ => {
            let stale = if estimate.get("kept").is_some_and(|kept| !kept.is_null()) {
                platform
                    .call(commands.kept, args)
                    .ok()
                    .filter(|kept| !kept.is_null())
            } else {
                None
            };
            Ok(json!({
                "status": "needs_approval",
                "price": {
                    "usd": usd,
                    "credits": estimate.get("credits"),
                    "paid_with": estimate.get("via"),
                },
                "message": format!(
                    "Buying this data costs about ${usd:.4} of provider price. Ask the user; if they agree, call {name} again with allow_spend_usd: {usd:.4} (or more)."
                ),
                "kept_data_fetched_at": stale.as_ref().and_then(|kept| kept.get("fetched_at")),
                "kept_data": stale.as_ref().and_then(|kept| kept.get("data")),
            }))
        }
    }
}

fn rank(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let report = platform.call("rank_report", args)?;
    Ok(json!({
        "tracker": report.get("tracker"),
        "summary": report.get("summary"),
        "competitors": report.get("competitors"),
        "visibility_by_day": report.get("visibility"),
        "keywords": report.get("rows"),
    }))
}

fn audit(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let report = platform.call("audit_report", args)?;
    if report.is_null() {
        return Ok(
            json!({ "message": "This project has no site audit yet; the user can start one on the Site audit page." }),
        );
    }
    let mut issues: Vec<Value> = report
        .get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|category| {
            let name = category.get("name").cloned().unwrap_or(Value::Null);
            category
                .get("issues")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|issue| issue.get("count").and_then(Value::as_u64).unwrap_or(0) > 0)
                .map(move |issue| {
                    let mut issue = issue.clone();
                    issue["category"] = name.clone();
                    issue
                })
        })
        .collect();
    let weight = |issue: &Value| match issue.get("priority").and_then(Value::as_str) {
        Some("high") => 0,
        Some("medium") => 1,
        _ => 2,
    };
    issues.sort_by(|left, right| {
        weight(left).cmp(&weight(right)).then_with(|| {
            right
                .get("count")
                .and_then(Value::as_u64)
                .cmp(&left.get("count").and_then(Value::as_u64))
        })
    });
    Ok(json!({
        "audit": report.get("audit"),
        "previous": report.get("previous"),
        "overview": report.get("overview"),
        "geo_readiness": report.get("geo"),
        "issues": issues,
    }))
}

fn ai(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let report = platform.call("ai_report", args)?;
    Ok(json!({
        "summary": report.get("summary"),
        "by_assistant": report.get("engines"),
        "brands": report.get("brands"),
        "history": report.get("history"),
        "questions": report.get("prompts"),
        "top_cited_domains": report.get("top_domains"),
        "own_cited_pages": report.get("own_pages"),
    }))
}

fn console(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let report = platform.call("gsc_report", args)?;
    if report.is_null() {
        return Ok(
            json!({ "message": "This project has no Search Console data yet; the user can connect Google and sync on the Search Console page." }),
        );
    }
    Ok(json!({
        "period": report.get("period"),
        "previous_period": report.get("previous"),
        "totals": report.get("now"),
        "totals_before": report.get("before"),
        "organic_traffic": report.get("traffic_now"),
        "organic_traffic_before": report.get("traffic_before"),
        "queries": report.get("queries"),
        "pages": report.get("pages"),
        "cannibalised": report.get("cannibalised"),
        "devices": report.get("devices"),
        "countries": report.get("countries"),
    }))
}

fn spending(platform: &Platform) -> Result<Value, CallError> {
    let providers = platform.call("list_providers", Value::Null)?;
    let sources: Vec<Value> = providers
        .as_array()
        .into_iter()
        .flatten()
        .map(|source| {
            json!({
                "id": source.get("id"),
                "name": source.get("name"),
                "paid_with": source.get("via"),
                "budget_usd": source.get("budget_usd"),
                "spent_this_month_usd": source.get("spent_usd"),
            })
        })
        .collect();
    Ok(json!({ "sources": sources, "by_month": platform.call("usage", Value::Null)? }))
}

fn estimate(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let tool = arguments
        .get("tool")
        .and_then(Value::as_str)
        .ok_or_else(|| "tool is required".to_owned())?;
    let commands = paid_tool(tool).ok_or_else(|| format!("`{tool}` is not a paid tool"))?;
    // The tool's arguments, or (as assistants sometimes send them) beside `tool`.
    let given = arguments.get("arguments").cloned().unwrap_or_else(|| {
        let mut flat = arguments.clone();
        if let Some(fields) = flat.as_object_mut() {
            fields.remove("tool");
        }
        flat
    });
    let args = command_arguments(tool, &given)?;
    let estimate = platform
        .call(commands.estimate, args)
        .map_err(|error| error.message)?;
    Ok(json!({
        "usd": estimate.get("usd"),
        "credits": estimate.get("credits"),
        "paid_with": estimate.get("via"),
        "kept_fresh": estimate.pointer("/kept/fresh").and_then(Value::as_bool).unwrap_or(false),
        "blocked": estimate.get("blocked"),
    }))
}

/// Calls a tool; `None` for a tool that does not exist.
pub(super) fn call(platform: &Platform, name: &str, arguments: &Value) -> Option<Value> {
    if let Some(commands) = paid_tool(name) {
        return Some(match paid(platform, name, &commands, arguments) {
            Ok(value) => answer(value),
            Err(message) => failure(&message),
        });
    }
    let args = match name {
        "estimate_cost" => {
            return Some(match estimate(platform, arguments) {
                Ok(value) => answer(value),
                Err(message) => failure(&message),
            });
        }
        "list_projects" | "spending" | "rank_report" | "site_audit_issues" | "ai_visibility"
        | "search_console" => match command_arguments(name, arguments) {
            Ok(args) => args,
            Err(message) => return Some(failure(&message)),
        },
        _ => return None,
    };
    let outcome = match name {
        "list_projects" => platform
            .call("list_projects", Value::Null)
            .map(|projects| json!({ "projects": projects })),
        "spending" => spending(platform),
        "rank_report" => rank(platform, args),
        "site_audit_issues" => {
            let mut args = args;
            if args.get("lang").is_none() {
                args["lang"] = json!("en");
            }
            audit(platform, args)
        }
        "ai_visibility" => ai(platform, args),
        _ => console(platform, args),
    };
    Some(match outcome {
        Ok(value) => answer(value),
        Err(error) => call_failure(&error),
    })
}
