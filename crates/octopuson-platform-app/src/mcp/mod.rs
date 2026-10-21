//! The platform as a Model Context Protocol server: AI assistants (Claude, ChatGPT, Cursor …)
//! read a workspace's SEO data and, with the user's allowance, buy fresh data.
//!
//! JSON-RPC 2.0 over two transports that share [`McpServer::handle`]: standard input and output
//! of the `octopuson-platform mcp` command (the desktop workspace), and `POST /mcp` of the web
//! server with a personal access token (the account's workspace). The server offers tools only
//! ([`tools`]): they call the platform's own commands, so they answer exactly like the pages.
//!
//! Paid data is never bought silently. A paid tool answers from the kept data while it is fresh;
//! otherwise it returns the price and asks to be called again with `allow_spend_usd` at least
//! that high. The sources' monthly budgets apply as everywhere.

mod tools;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde_json::{Value, json};

use crate::Platform;

/// Name the server gives itself to clients.
pub const NAME: &str = "octopuson-platform";

/// Protocol versions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// JSON-RPC error codes.
mod code {
    pub(super) const PARSE: i64 = -32_700;
    pub(super) const INVALID_REQUEST: i64 = -32_600;
    pub(super) const METHOD_NOT_FOUND: i64 = -32_601;
    pub(super) const INVALID_PARAMS: i64 = -32_602;
}

/// What the assistant is told about the server when it connects.
const INSTRUCTIONS: &str = "OctopusOn GEO-SEO Platform: SEO and AI-visibility data of the user's projects. \
Start with list_projects to get project ids. Tools that read stored data (rank_report, site_audit_issues, \
ai_visibility, search_console, spending) are free. Tools that buy data (domain_overview, keyword_research, \
serp_overview, backlinks, keyword_gap, ai_mentions) answer from kept data when it is fresh; otherwise they \
return the price (status needs_approval) and must be called again with allow_spend_usd at least that \
price. Ask the user before spending, state the price, and never raise allow_spend_usd on your own.";

/// The MCP server of one platform.
pub struct McpServer {
    platform: Arc<Platform>,
    name: String,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

fn error(id: &Value, code: i64, message: impl Into<String>) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

fn result(id: &Value, result: Value) -> Value {
    let mut answer = json!({ "jsonrpc": "2.0", "id": id });
    answer["result"] = result;
    answer
}

impl McpServer {
    /// A server over `platform`, naming itself `name` to clients.
    pub fn new(platform: Arc<Platform>, name: &str) -> Self {
        Self {
            platform,
            name: name.to_owned(),
        }
    }

    /// Answers one message; `None` for notifications (and answers to them).
    pub fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // A response from the client (to nothing this server asks), or no method at all.
            return match id {
                Some(id) if message.get("result").is_none() && message.get("error").is_none() => {
                    Some(error(
                        &id,
                        code::INVALID_REQUEST,
                        "a request needs a method",
                    ))
                }
                _ => None,
            };
        };
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return id.map(|id| error(&id, code::INVALID_REQUEST, "jsonrpc must be \"2.0\""));
        }
        let id = id?;
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => {
                let asked = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let version = PROTOCOL_VERSIONS
                    .iter()
                    .find(|known| **known == asked)
                    .unwrap_or(&PROTOCOL_VERSIONS[0]);
                result(
                    &id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": { "tools": { "listChanged": false } },
                        "serverInfo": {
                            "name": self.name,
                            "title": "OctopusOn GEO-SEO Platform",
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                        "instructions": INSTRUCTIONS,
                    }),
                )
            }
            "ping" => result(&id, json!({})),
            "tools/list" => result(&id, json!({ "tools": tools::definitions() })),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(
                        &id,
                        code::INVALID_PARAMS,
                        "tools/call needs a tool name",
                    ));
                };
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match tools::call(&self.platform, name, &arguments) {
                    Some(answer) => result(&id, answer),
                    None => error(&id, code::INVALID_PARAMS, format!("unknown tool `{name}`")),
                }
            }
            other => error(
                &id,
                code::METHOD_NOT_FOUND,
                format!("method `{other}` is not offered"),
            ),
        })
    }

    /// Answers a message as text (one JSON object, or a batch array); `None` when nothing is
    /// answered.
    pub fn handle_text(&self, text: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(text) {
            Ok(message) => message,
            Err(problem) => {
                return Some(
                    error(&Value::Null, code::PARSE, format!("not JSON: {problem}")).to_string(),
                );
            }
        };
        match message {
            Value::Array(batch) => {
                let answers: Vec<Value> = batch.iter().filter_map(|one| self.handle(one)).collect();
                (!answers.is_empty()).then(|| Value::Array(answers).to_string())
            }
            one => self.handle(&one).map(|answer| answer.to_string()),
        }
    }
}

/// Answers messages from `input` on `output` until `input` ends: one JSON-RPC message per line,
/// one answer per line (the stdio transport). Anything else, such as logs, belongs on standard
/// error.
///
/// # Errors
/// Reading or writing failed.
pub fn serve(server: &McpServer, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = server.handle_text(&line) {
            output.write_all(answer.as_bytes())?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// Serves `platform` on this process's standard input and output (`octopuson-platform mcp`, or
/// the desktop program started with `mcp`).
///
/// # Errors
/// Reading or writing failed.
pub fn stdio(platform: &Arc<Platform>) -> io::Result<()> {
    let server = McpServer::new(Arc::clone(platform), NAME);
    serve(&server, io::stdin().lock(), io::stdout().lock())
}
