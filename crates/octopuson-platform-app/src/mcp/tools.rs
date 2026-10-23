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
        "keyword_insights" => ("research_estimate", "get_research", "fetch_research"),
        _ => return None,
    };
    Some(PaidTool {
        estimate,
        kept,
        fetch,
    })
}

pub(super) fn project_property() -> Value {
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

pub(super) fn schema(required: &[&str], properties: Vec<(&str, Value)>) -> Value {
    let properties: Map<String, Value> = properties
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

pub(super) fn tool(
    name: &str,
    title: &str,
    description: &str,
    input: Value,
    read_only: bool,
) -> Value {
    let mut tool = json!({
        "name": name,
        "title": title,
        "description": description,
        "annotations": { "readOnlyHint": read_only, "openWorldHint": !read_only },
    });
    tool["inputSchema"] = input;
    tool
}

/// Another market than the project's.
fn market_property() -> Value {
    json!({ "type": "string", "pattern": "^[a-z]{2}-[a-z]{2,3}$", "description": "Another market than the project's: country-language, like de-de or us-en." })
}

/// Filters, order and paging asked of DataForSEO, over every row it holds.
fn query_property(fields: &str) -> Value {
    json!({
        "type": "object",
        "description": format!("Filters, order and paging, applied by DataForSEO to every row it holds (not only the rows returned). Fields: {fields}. Text fields take eq, ne, in, not_in, contains, not_contains, starts_with, regex, not_regex; numbers eq, ne, lt, le, gt, ge; flags eq; features has, has_not. Up to 8 filters and 3 order rules; offset pages through the list."),
        "properties": {
            "filters": {
                "type": "array",
                "maxItems": 8,
                "items": {
                    "type": "object",
                    "properties": {
                        "field": { "type": "string" },
                        "op": { "type": "string", "enum": ["eq", "ne", "lt", "le", "gt", "ge", "in", "not_in", "contains", "not_contains", "starts_with", "regex", "not_regex", "has", "has_not"] },
                        "value": {}
                    },
                    "required": ["field", "op", "value"],
                    "additionalProperties": false
                }
            },
            "order": {
                "type": "array",
                "maxItems": 3,
                "items": {
                    "type": "object",
                    "properties": {
                        "field": { "type": "string" },
                        "direction": { "type": "string", "enum": ["asc", "desc"] }
                    },
                    "required": ["field", "direction"],
                    "additionalProperties": false
                }
            },
            "offset": { "type": "integer", "minimum": 0, "maximum": 100_000 }
        },
        "additionalProperties": false
    })
}

fn paid_schema(mut properties: Vec<(&'static str, Value)>, required: &[&str]) -> Value {
    properties.extend(spend_properties());
    schema(required, properties)
}

/// Every tool, as `tools/list` gives them.
#[allow(clippy::too_many_lines)] // the tool list
pub(super) fn definitions() -> Vec<Value> {
    let limit = json!({ "type": "integer", "minimum": 10, "maximum": 1000, "description": "Rows of a table report (100, 500 or 1000)." });
    let mut tools = vec![
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
            "Organic and paid keywords, traffic and traffic value of the project's domain (or another), and its history, ranked keywords (organic now, lost, or paid with the ads shown), top pages, competitors, subdomains or traffic in every country (DataForSEO Labs). Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "domain",
                        json!({ "type": "string", "description": "Another domain to look at." }),
                    ),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["summary", "history", "keywords", "pages", "competitors", "subdomains", "countries"], "default": "summary" }),
                    ),
                    ("limit", limit.clone()),
                    (
                        "set",
                        json!({ "type": "string", "enum": ["organic", "lost", "paid"], "default": "organic", "description": "Keywords report: organic results now, organic results lost, or ads (with their text)." }),
                    ),
                    (
                        "months",
                        json!({ "type": "integer", "minimum": 6, "maximum": 120, "description": "History report: months back (24 by default; data starts October 2020)." }),
                    ),
                    ("market", market_property()),
                    (
                        "query",
                        query_property(
                            "keywords report: keyword, volume, cpc, competition, kd, intent, features, position, previous, new, up, down, traffic, value, url",
                        ),
                    ),
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
                    ("market", market_property()),
                    (
                        "query",
                        query_property(
                            "ideas, related and suggestions reports: keyword, volume, cpc, competition, kd, intent, features",
                        ),
                    ),
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
                    ("market", market_property()),
                ],
                &["project_id", "keyword"],
            ),
            false,
        ),
        tool(
            "backlinks",
            "Backlinks",
            "The backlink profile of the project's domain (or a target): summary with authority (OA), backlinks, referring domains, anchors, linked pages, new and lost links over time, or the profile month by month since 2019 (history), or a toxic-link audit (audit). Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["summary", "backlinks", "referring_domains", "anchors", "pages", "new_lost", "history", "audit"], "default": "summary", "description": "audit: each linking site judged by toxic-link rules (spam score, link networks, spam TLDs, site-wide links, link farms, spam anchors), the riskiest first." }),
                    ),
                    (
                        "target",
                        json!({ "type": "string", "description": "Another domain or page." }),
                    ),
                    ("limit", limit.clone()),
                    (
                        "query",
                        query_property(
                            "backlinks report: domain, url, target, anchor, dofollow, rank, domain_rank, spam, first_seen, last_seen, status, country; referring_domains report: domain, rank, backlinks, spam, first_seen, pages",
                        ),
                    ),
                ],
                &["project_id"],
            ),
            false,
        ),
        tool(
            "keyword_gap",
            "Keyword and link gap",
            "Keywords competitors rank for and the project's site does not (missing), keywords both rank for (shared), every keyword of the site and its competitors in Semrush's groups shared/missing/weak/strong/untapped/unique (keywords), keywords a competitor's page ranks for and the site's page does not (pages), sites linking to competitors but not to the site (links), many sites compared (bulk), their organic traffic month by month (share), the categories the site ranks in (categories) or the leading domains of categories (players). Up to 5 competitors; the project's own by default. Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["missing", "shared", "keywords", "pages", "links", "bulk", "share", "categories", "players"], "default": "missing" }),
                    ),
                    (
                        "competitors",
                        json!({ "type": "array", "items": { "type": "string" }, "maxItems": 5 }),
                    ),
                    ("limit", limit.clone()),
                    ("market", market_property()),
                    (
                        "own_page",
                        json!({ "type": "string", "description": "pages report: the site's page, a full address." }),
                    ),
                    (
                        "competitor_page",
                        json!({ "type": "string", "description": "pages report: the competitor's page, a full address." }),
                    ),
                    (
                        "months",
                        json!({ "type": "integer", "minimum": 3, "maximum": 120, "description": "share report: months of history (12 by default)." }),
                    ),
                    (
                        "categories",
                        json!({ "type": "array", "items": { "type": "integer" }, "maxItems": 5, "description": "players report: category codes from the categories report." }),
                    ),
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
            "keyword_insights",
            "Keyword insights",
            "One keyword in depth, in the project's market: its searches month by month since 2021 (history), its top 10 month by month over a year (serp_history), each result of its result page with authority, linking sites and traffic (serp_metrics), the parent topic and traffic potential from the first result's keywords (parent), or its searches in other markets (countries). serp_metrics and parent read the keyword's kept result page: call serp_overview first. Paid.",
            paid_schema(
                vec![
                    ("project_id", project_property()),
                    (
                        "report",
                        json!({ "type": "string", "enum": ["history", "serp_history", "serp_metrics", "parent", "countries"], "default": "history" }),
                    ),
                    (
                        "seed",
                        json!({ "type": "string", "description": "The keyword." }),
                    ),
                    (
                        "mobile",
                        json!({ "type": "boolean", "description": "The mobile result page (result-page reports)." }),
                    ),
                    ("market", market_property()),
                    (
                        "markets",
                        json!({ "type": "array", "items": { "type": "string", "pattern": "^[a-z]{2}-[a-z]{2,3}$" }, "maxItems": 10, "description": "countries report: the markets to measure in, country-language each." }),
                    ),
                ],
                &["project_id", "seed"],
            ),
            false,
        ),
        tool(
            "rank_report",
            "Rank tracking",
            "The rank tracker of a project: visibility and its change, average position, movement, estimated visits, competitor shares and visits, the site's landing pages, SERP elements it holds, and every tracked keyword with its position, change, page, competitors' positions and SERP signals; optionally compared with a past day. Free (stored checks).",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "days",
                        json!({ "type": "integer", "minimum": 7, "maximum": 365, "default": 90 }),
                    ),
                    (
                        "compare",
                        json!({ "type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$", "description": "A past day (yyyy-mm-dd) to compare each keyword's position with." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "local_seo",
            "Local SEO",
            "The project's business on Google Maps: its place at every point of the latest grid around it per keyword (average rank, share of local voice), the businesses that beat it in the local pack, the grid's trend, and its reviews with the rating over time. Free (stored grid runs and reviews).",
            schema(&["project_id"], vec![("project_id", project_property())]),
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
            "How ChatGPT, Claude, Gemini, Perplexity and Google AI Mode answer the project's questions: mention and citation rates, share of voice against rivals (also run by run, brand by brand), the matrix of questions and assistants, the domains and pages the answers cite. Free (stored answers).",
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
            "list_snapshots",
            "Snapshots",
            "A project's snapshots, newest first: each keeps the project's whole state at a moment (organic traffic and keywords, top pages, backlinks and referring domains, chosen competitors, rank tracking, site audit, AI visibility, Search Console, the local map grid and the toxic-link audit), with its cost and status. Also which one is the baseline (where the work began) and the automatic schedule. Free.",
            schema(&["project_id"], vec![("project_id", project_property())]),
            true,
        ),
        tool(
            "compare_snapshots",
            "Compare snapshots",
            "Two snapshots of a project compared: every number before and after with its change, keywords won, lost, up and down, pages, referring domains won and lost, tracked keywords, site audit issues fixed and new, AI answers that now (or no longer) name the brand, map keywords that moved, risky linking sites found or cleared, competitors side by side, and a written summary. By default the baseline against the latest. Free.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "from",
                        json!({ "type": "integer", "description": "The earlier snapshot's id; the baseline when absent." }),
                    ),
                    (
                        "to",
                        json!({ "type": "integer", "description": "The later snapshot's id; the latest when absent." }),
                    ),
                    (
                        "previous",
                        json!({ "type": "boolean", "description": "Compare the latest with the snapshot just before it instead of the baseline." }),
                    ),
                    (
                        "lang",
                        json!({ "type": "string", "enum": ["en", "tr", "de", "es"], "default": "en", "description": "Language of the summary." }),
                    ),
                ],
            ),
            true,
        ),
        tool(
            "take_snapshot",
            "Take a snapshot",
            "Takes a new snapshot of a project in the background. The paid parts (domain overview, ranked keywords, top pages, backlink summary, referring domains, and each chosen competitor's overview and backlinks) are bought unless the modules keep them fresh; the free parts are read from rank tracking, the site audit, AI visibility, Search Console, the local map grid and the kept toxic-link audit. Without allow_spend_usd nothing is bought: the answer gives the price. Paid.",
            schema(
                &["project_id"],
                vec![
                    ("project_id", project_property()),
                    (
                        "competitors",
                        json!({ "type": "array", "items": { "type": "string" }, "description": "The project's competitor domains to keep too; the saved choice when absent." }),
                    ),
                    (
                        "label",
                        json!({ "type": "string", "description": "A name for the snapshot." }),
                    ),
                    spend_properties()[0].clone(),
                ],
            ),
            false,
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
                        json!({ "type": "string", "enum": ["domain_overview", "keyword_research", "serp_overview", "keyword_insights", "backlinks", "keyword_gap", "ai_mentions"] }),
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
    ];
    tools.extend(super::work::definitions());
    tools
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
pub(super) fn answer(value: Value) -> Value {
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
pub(super) fn failure(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

pub(super) fn call_failure(error: &CallError) -> Value {
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
                "total": kept.get("total"),
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
                "total": fetched.get("total"),
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
        "competitor_visits": report.get("traffic"),
        "landing_pages": report.get("landing_pages"),
        "visibility_by_day": report.get("visibility"),
        "compared_with": report.get("compare"),
        "keywords": report.get("rows"),
    }))
}

fn local(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let setup = platform.call("local_setup", args.clone())?;
    if setup.get("business").is_none_or(Value::is_null) {
        return Ok(
            json!({ "message": "This project has no business chosen yet; the user can find it on the Local SEO page." }),
        );
    }
    let grid = platform.call("local_grid", args.clone())?;
    let reviews = platform.call("local_reviews", args)?;
    let keywords: Vec<Value> = grid
        .get("keywords")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|keyword| {
            json!({
                "keyword": keyword.get("keyword"),
                "summary": keyword.get("summary"),
                "positions_by_row": keyword.get("cells").and_then(Value::as_array).map(|cells| {
                    cells.iter().map(|cell| json!([cell.get("row"), cell.get("col"), cell.get("position")])).collect::<Vec<_>>()
                }),
            })
        })
        .collect();
    let latest: Vec<Value> = reviews
        .get("reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(20)
        .cloned()
        .collect();
    Ok(json!({
        "business": setup.get("business"),
        "grid": { "points_a_side": setup.get("grid"), "radius_km": setup.get("radius_km") },
        "run": grid.get("run"),
        "summary": grid.get("summary"),
        "keywords": keywords,
        "rivals": grid.get("rivals"),
        "trend": grid.get("trend"),
        "reviews_summary": reviews.get("summary"),
        "rating_by_day": reviews.get("ratings"),
        "latest_reviews": latest,
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
        "top_cited_pages": report.get("top_pages"),
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

fn snapshots(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let setup = platform.call("snapshot_setup", args)?;
    let rows: Vec<Value> = setup
        .get("snapshots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|row| {
            let mut row = row.clone();
            if let Some(fields) = row.as_object_mut() {
                fields.remove("data");
            }
            row
        })
        .collect();
    Ok(json!({
        "baseline_id": setup.get("baseline_id"),
        "schedule": setup.pointer("/settings/schedule"),
        "competitors_kept": setup.pointer("/settings/competitors"),
        "next_automatic_at_unix": setup.get("next_at"),
        "project_competitors": setup.get("competitors"),
        "snapshots": rows,
    }))
}

fn compare(platform: &Platform, args: Value) -> Result<Value, CallError> {
    let mut args = args;
    if args.get("lang").is_none() {
        args["lang"] = json!("en");
    }
    let compared = platform.call("compare_snapshots", args)?;
    if compared.is_null() {
        return Ok(
            json!({ "message": "This project has fewer than two finished snapshots; take one with take_snapshot (the user can also take them on the Snapshots page)." }),
        );
    }
    Ok(compared)
}

/// Takes a snapshot: at once when it costs nothing or the allowance covers it, else its price.
fn take_snapshot(platform: &Platform, arguments: &Value) -> Result<Value, String> {
    let args = command_arguments("take_snapshot", arguments)?;
    let allow = arguments.get("allow_spend_usd").and_then(Value::as_f64);
    let mut ask = json!({ "project_id": args.get("project_id") });
    if let Some(competitors) = args.get("competitors") {
        ask["competitors"] = competitors.clone();
    }
    let estimate = platform
        .call("snapshot_estimate", ask.clone())
        .map_err(|error| error.message)?;
    let usd = estimate
        .get("usd")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let blocked = estimate.get("blocked").filter(|blocked| !blocked.is_null());
    if let (Some(blocked), None) = (blocked, allow) {
        return Ok(json!({
            "status": "blocked",
            "message": format!(
                "The paid parts cannot be bought now: {} — the user can enter their own key or connect their OctopusOn SEO account under Sources. To keep only the free parts, call take_snapshot again with allow_spend_usd: 0.",
                blocked.get("message").and_then(Value::as_str).unwrap_or("no source")
            ),
        }));
    }
    let covered = blocked.is_some() || usd <= 0.0 || allow.is_some_and(|allow| allow + 1e-9 >= usd);
    if !covered {
        return Ok(json!({
            "status": "needs_approval",
            "price": {
                "usd": usd,
                "credits": estimate.get("credits"),
                "paid_with": estimate.get("via"),
                "parts_to_buy": estimate.get("paid"),
                "parts_kept_fresh": estimate.get("kept"),
            },
            "message": format!(
                "Taking this snapshot costs about ${usd:.4} of provider price. Ask the user; if they agree, call take_snapshot again with allow_spend_usd: {usd:.4} (or more)."
            ),
        }));
    }
    if let Some(label) = args.get("label") {
        ask["label"] = label.clone();
    }
    let job = platform
        .call("start_snapshot", ask)
        .map_err(|error| error.message)?;
    Ok(json!({
        "status": "started",
        "job_id": job,
        "price_usd": if blocked.is_some() { 0.0 } else { usd },
        "message": "The snapshot is being taken in the background, usually in under a minute; list_snapshots shows it when it has finished, and compare_snapshots compares it.",
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
        "take_snapshot" => {
            return Some(match take_snapshot(platform, arguments) {
                Ok(value) => answer(value),
                Err(message) => failure(&message),
            });
        }
        "estimate_cost" => {
            return Some(match estimate(platform, arguments) {
                Ok(value) => answer(value),
                Err(message) => failure(&message),
            });
        }
        "list_projects" | "spending" | "rank_report" | "local_seo" | "site_audit_issues"
        | "ai_visibility" | "search_console" | "list_snapshots" | "compare_snapshots" => {
            match command_arguments(name, arguments) {
                Ok(args) => args,
                Err(message) => return Some(failure(&message)),
            }
        }
        _ => return super::work::call(platform, name, arguments),
    };
    let outcome = match name {
        "list_projects" => platform
            .call("list_projects", Value::Null)
            .map(|projects| json!({ "projects": projects })),
        "spending" => spending(platform),
        "rank_report" => rank(platform, args),
        "local_seo" => local(platform, args),
        "site_audit_issues" => {
            let mut args = args;
            if args.get("lang").is_none() {
                args["lang"] = json!("en");
            }
            audit(platform, args)
        }
        "ai_visibility" => ai(platform, args),
        "list_snapshots" => snapshots(platform, args),
        "compare_snapshots" => compare(platform, args),
        _ => console(platform, args),
    };
    Some(match outcome {
        Ok(value) => answer(value),
        Err(error) => call_failure(&error),
    })
}
