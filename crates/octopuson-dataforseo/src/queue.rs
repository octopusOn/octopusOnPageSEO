//! The standard queue: tasks posted up to a hundred a request, each answered later and read
//! back by the id the queue gave it, for less than live answers cost.

use serde_json::Value;

use crate::number;

/// Tasks sent to the standard queue in one request.
pub const QUEUE_BATCH: usize = 100;

/// Status of a task the standard queue took.
const CREATED: u64 = 20_100;

/// A task the standard queue took or refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Posted {
    /// The tag it was sent with.
    pub tag: String,
    /// The id to read its answer by, when taken.
    pub id: Option<String>,
    /// Why it was refused.
    pub error: Option<String>,
    /// What it cost, US dollars.
    pub cost_usd: f64,
}

/// Reads the tasks a `task_post` answered with, in the order sent.
pub fn posted_from_tasks(tasks: &[Value]) -> Vec<Posted> {
    tasks
        .iter()
        .map(|task| {
            let status = task
                .get("status_code")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            // The id goes into a path: only what DataForSEO's ids are made of.
            let id = task
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| {
                    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                })
                .filter(|_| status == CREATED)
                .map(str::to_owned);
            let message = task
                .get("status_message")
                .and_then(Value::as_str)
                .unwrap_or("refused");
            Posted {
                tag: task
                    .pointer("/data/tag")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                error: id.is_none().then(|| format!("{status}: {message}")),
                id,
                cost_usd: task.get("cost").and_then(number).unwrap_or_default(),
            }
        })
        .collect()
}
