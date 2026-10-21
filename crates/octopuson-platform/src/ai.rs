//! AI visibility: the questions a project asks AI assistants, which assistants, how often, the
//! rival brands watched, and every answer with how the brands appear in it.

use std::collections::BTreeMap;

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError, RunEnd, list_json, normalize_domain, parse_list};

/// Assistants a project can ask.
pub const AI_ENGINES: &[&str] = &[
    "chatgpt",
    "claude",
    "gemini",
    "perplexity",
    "google_ai_mode",
];

/// Most questions of a project.
pub const MAX_PROMPTS: usize = 200;

/// Longest question.
const MAX_PROMPT: usize = 500;

/// Most rival brands watched.
const MAX_RIVALS: usize = 10;

/// Longest answer kept, in characters.
const MAX_ANSWER: usize = 20_000;

/// Schedules of AI visibility runs.
const SCHEDULES: &[&str] = &["weekly", "monthly", "manual"];

/// A rival brand: its site and the names it goes by.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rival {
    /// Its site's domain.
    pub domain: String,
    /// Names it is written as (the domain is always watched too).
    #[serde(default)]
    pub names: Vec<String>,
}

/// How a project's AI visibility is measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSettings {
    /// `weekly`, `monthly` or `manual`.
    pub schedule: String,
    /// Assistants asked.
    pub engines: Vec<String>,
    /// Models chosen per assistant (the default when absent).
    #[serde(default)]
    pub models: BTreeMap<String, String>,
    /// Rival brands; the project's competitors when empty.
    #[serde(default)]
    pub rivals: Vec<Rival>,
    /// When the last run started, RFC 3339.
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Default for AiSettings {
    /// Asked only by hand until the user picks a schedule; every assistant.
    fn default() -> Self {
        Self {
            schedule: "manual".to_owned(),
            engines: AI_ENGINES.iter().map(|&engine| engine.to_owned()).collect(),
            models: BTreeMap::new(),
            rivals: Vec::new(),
            last_run_at: None,
        }
    }
}

/// A question of a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPrompt {
    /// Question id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// The question.
    pub prompt: String,
    /// The user's tags.
    pub tags: Vec<String>,
    /// RFC 3339.
    pub added_at: String,
}

/// One run of a project's questions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRunRow {
    /// Run id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// Started by the schedule.
    pub scheduled: bool,
    /// `running`, `done`, `partial`, `failed`, `cancelled` or `interrupted`.
    pub status: String,
    /// Answers planned.
    pub total: u32,
    /// Answers read.
    pub checked: u32,
    /// Answers that failed.
    pub failed: u32,
    /// US dollars paid.
    pub cost_usd: f64,
    /// Why it stopped early.
    pub error: Option<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub finished_at: Option<String>,
}

/// How one brand appears in an answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerBrand {
    /// The brand's domain.
    pub key: String,
    /// The project's own brand.
    pub own: bool,
    /// Its place among the brands mentioned.
    pub position: Option<u32>,
    /// Its site is cited.
    pub cited: bool,
    /// Tone of its mentions, -1 to 1.
    pub sentiment: Option<f64>,
}

/// A source an answer cites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerSource {
    /// The page.
    pub url: String,
    /// Its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its host.
    pub domain: String,
}

/// One answer to one question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiAnswerRow {
    /// The run.
    pub run_id: i64,
    /// The project.
    pub project_id: i64,
    /// The question.
    pub prompt_id: i64,
    /// The assistant.
    pub engine: String,
    /// How it was asked: `own` (the assistant's own key), `dataforseo` or `credit`.
    pub via: String,
    /// The model that answered.
    pub model: Option<String>,
    /// RFC 3339.
    pub checked_at: String,
    /// The answer (at most 20,000 characters).
    pub text: String,
    /// The own brand is mentioned.
    pub mentioned: bool,
    /// Its place among the brands mentioned.
    pub position: Option<u32>,
    /// Its site is cited.
    pub cited: bool,
    /// Tone of its mentions.
    pub sentiment: Option<f64>,
    /// Every brand mentioned or cited.
    pub brands: Vec<AnswerBrand>,
    /// The sources.
    pub citations: Vec<AnswerSource>,
    /// Why there is no answer.
    pub error: Option<String>,
}

fn invalid(message: impl Into<String>) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.into())
}

fn json_of<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_owned())
}

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, PlatformStoreError> {
    serde_json::from_str(text).map_err(|error| PlatformStoreError::Corrupt(error.to_string()))
}

/// Settings checked and cleaned.
fn clean_settings(settings: &AiSettings) -> Result<AiSettings, PlatformStoreError> {
    if !SCHEDULES.contains(&settings.schedule.as_str()) {
        return Err(invalid("schedule: weekly, monthly or manual"));
    }
    let mut engines: Vec<String> = Vec::new();
    for engine in &settings.engines {
        if !AI_ENGINES.contains(&engine.as_str()) {
            return Err(invalid(format!("engines: unknown assistant `{engine}`")));
        }
        if !engines.contains(engine) {
            engines.push(engine.clone());
        }
    }
    if engines.is_empty() {
        return Err(invalid("engines: choose at least one assistant"));
    }
    let mut models = BTreeMap::new();
    for (engine, model) in &settings.models {
        let model = model.trim();
        if !AI_ENGINES.contains(&engine.as_str()) {
            return Err(invalid(format!("models: unknown assistant `{engine}`")));
        }
        if model.chars().count() > 100 {
            return Err(invalid("models: at most 100 characters"));
        }
        if !model.is_empty() {
            models.insert(engine.clone(), model.to_owned());
        }
    }
    let mut rivals: Vec<Rival> = Vec::new();
    for rival in &settings.rivals {
        let domain = normalize_domain(&rival.domain);
        if domain.is_empty() || rivals.iter().any(|known| known.domain == domain) {
            continue;
        }
        let names: Vec<String> = rival
            .names
            .iter()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty() && name.chars().count() <= 80)
            .collect();
        rivals.push(Rival { domain, names });
    }
    if rivals.len() > MAX_RIVALS {
        return Err(invalid("rivals: at most 10"));
    }
    Ok(AiSettings {
        schedule: settings.schedule.clone(),
        engines,
        models,
        rivals,
        last_run_at: None,
    })
}

const RUN_COLUMNS: &str = "id, project_id, scheduled, status, total, checked, failed, cost_usd, error, started_at, finished_at";

fn run(row: &Row<'_>) -> duckdb::Result<AiRunRow> {
    Ok(AiRunRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        scheduled: row.get(2)?,
        status: row.get(3)?,
        total: row.get(4)?,
        checked: row.get(5)?,
        failed: row.get(6)?,
        cost_usd: row.get(7)?,
        error: row.get(8)?,
        started_at: row.get(9)?,
        finished_at: row.get(10)?,
    })
}

impl PlatformStore {
    /// How a project's AI visibility is measured; the defaults until saved.
    ///
    /// # Errors
    /// Database errors, or stored settings that are not JSON.
    pub fn ai_settings(&self, project_id: i64) -> Result<AiSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT schedule, engines, models, rivals, last_run_at FROM ai_settings WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        match rows.next().transpose()? {
            Some((schedule, engines, models, rivals, last_run_at)) => Ok(AiSettings {
                schedule,
                engines: parse_list(&engines)?,
                models: parse(&models)?,
                rivals: parse(&rivals)?,
                last_run_at,
            }),
            None => Ok(AiSettings::default()),
        }
    }

    /// Saves how a project's AI visibility is measured; the last run time stays.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown schedule or assistant, or database errors.
    pub fn save_ai_settings(
        &self,
        project_id: i64,
        settings: &AiSettings,
    ) -> Result<AiSettings, PlatformStoreError> {
        self.project(project_id)?;
        let clean = clean_settings(settings)?;
        self.connection.execute(
            "INSERT INTO ai_settings (project_id, schedule, engines, models, rivals) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET schedule = excluded.schedule,
             engines = excluded.engines, models = excluded.models, rivals = excluded.rivals",
            params![
                project_id,
                clean.schedule,
                list_json(&clean.engines),
                json_of(&clean.models),
                json_of(&clean.rivals)
            ],
        )?;
        self.ai_settings(project_id)
    }

    /// Every project's AI settings with how many questions it has.
    ///
    /// # Errors
    /// Database errors.
    pub fn ai_plans(&self) -> Result<Vec<(i64, AiSettings, u64)>, PlatformStoreError> {
        let ids: Vec<i64> = {
            let mut statement = self.connection.prepare(
                "SELECT s.project_id FROM ai_settings s JOIN projects p ON p.id = s.project_id ORDER BY s.project_id",
            )?;
            statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        let mut plans = Vec::with_capacity(ids.len());
        for id in ids {
            let prompts = self.ai_prompts(id)?.len() as u64;
            plans.push((id, self.ai_settings(id)?, prompts));
        }
        Ok(plans)
    }

    /// A project's questions, in the order they were added.
    ///
    /// # Errors
    /// Database errors.
    pub fn ai_prompts(&self, project_id: i64) -> Result<Vec<AiPrompt>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, prompt, tags, added_at FROM ai_prompts WHERE project_id = ? ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok((
                AiPrompt {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    prompt: row.get(2)?,
                    tags: Vec::new(),
                    added_at: row.get(4)?,
                },
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (mut prompt, tags) = row?;
            prompt.tags = parse_list(&tags)?;
            Ok(prompt)
        })
        .collect()
    }

    /// Adds questions at `now` (repeats skipped); returns how many are new.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for a question over 500 characters or past [`MAX_PROMPTS`], or database errors.
    pub fn add_ai_prompts(
        &self,
        project_id: i64,
        prompts: &[String],
        tags: &[String],
        now: &str,
    ) -> Result<usize, PlatformStoreError> {
        self.project(project_id)?;
        let known = self.ai_prompts(project_id)?;
        let tags: Vec<String> = tags
            .iter()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect();
        let mut new: Vec<String> = Vec::new();
        for prompt in prompts {
            let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
            if prompt.is_empty()
                || known.iter().any(|row| row.prompt == prompt)
                || new.contains(&prompt)
            {
                continue;
            }
            if prompt.chars().count() > MAX_PROMPT {
                return Err(invalid("prompts: at most 500 characters each"));
            }
            new.push(prompt);
        }
        if known.len() + new.len() > MAX_PROMPTS {
            return Err(invalid(format!(
                "prompts: a project asks at most {MAX_PROMPTS} questions"
            )));
        }
        for prompt in &new {
            self.connection.execute(
                "INSERT INTO ai_prompts (project_id, prompt, tags, added_at) VALUES (?, ?, ?, ?)",
                params![project_id, prompt, list_json(&tags), now],
            )?;
        }
        Ok(new.len())
    }

    /// Removes questions and their answers; returns how many are left.
    ///
    /// # Errors
    /// Database errors.
    pub fn remove_ai_prompts(
        &self,
        project_id: i64,
        prompt_ids: &[i64],
    ) -> Result<usize, PlatformStoreError> {
        for id in prompt_ids {
            self.connection.execute(
                "DELETE FROM ai_answers WHERE project_id = ? AND prompt_id = ?",
                params![project_id, id],
            )?;
            self.connection.execute(
                "DELETE FROM ai_prompts WHERE project_id = ? AND id = ?",
                params![project_id, id],
            )?;
        }
        Ok(self.ai_prompts(project_id)?.len())
    }

    /// Starts a run of `total` answers at `now` and moves the schedule on; a run left running
    /// by a program that stopped becomes `interrupted`.
    ///
    /// # Errors
    /// Database errors.
    pub fn begin_ai_run(
        &self,
        project_id: i64,
        scheduled: bool,
        total: u32,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        let defaults = AiSettings::default();
        self.connection.execute(
            "INSERT INTO ai_settings (project_id, schedule, engines, models, rivals, last_run_at)
             VALUES (?, ?, ?, '{}', '[]', ?)
             ON CONFLICT (project_id) DO UPDATE SET last_run_at = excluded.last_run_at",
            params![
                project_id,
                defaults.schedule,
                list_json(&defaults.engines),
                now
            ],
        )?;
        self.connection.execute(
            "UPDATE ai_runs SET status = 'interrupted', finished_at = ?
             WHERE project_id = ? AND status = 'running'",
            params![now, project_id],
        )?;
        Ok(self.connection.query_row(
            "INSERT INTO ai_runs (project_id, scheduled, status, total, started_at)
             VALUES (?, ?, 'running', ?, ?) RETURNING id",
            params![project_id, scheduled, total, now],
            |row| row.get(0),
        )?)
    }

    /// Records how a run ended at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_ai_run(
        &self,
        run_id: i64,
        end: &RunEnd<'_>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE ai_runs SET status = ?, checked = ?, failed = ?, cost_usd = ?, error = ?,
             finished_at = ? WHERE id = ?",
            params![
                end.status,
                end.checked,
                end.failed,
                end.cost_usd,
                end.error,
                now,
                run_id
            ],
        )?;
        Ok(())
    }

    /// A project's latest runs, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn ai_runs(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<AiRunRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM ai_runs WHERE project_id = ? ORDER BY id DESC LIMIT ?"
        ))?;
        let rows = statement.query_map(params![project_id, limit], run)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Keeps an answer (its text cut at 20,000 characters).
    ///
    /// # Errors
    /// Database errors.
    pub fn add_ai_answer(&self, answer: &AiAnswerRow) -> Result<(), PlatformStoreError> {
        let text: String = answer.text.chars().take(MAX_ANSWER).collect();
        self.connection.execute(
            "INSERT INTO ai_answers (run_id, project_id, prompt_id, engine, via, model, checked_at,
             text, mentioned, position, cited, sentiment, brands, citations, error)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                answer.run_id,
                answer.project_id,
                answer.prompt_id,
                answer.engine,
                answer.via,
                answer.model,
                answer.checked_at,
                text,
                answer.mentioned,
                answer.position,
                answer.cited,
                answer.sentiment,
                json_of(&answer.brands),
                json_of(&answer.citations),
                answer.error
            ],
        )?;
        Ok(())
    }

    /// A project's answers, oldest first: since a time (RFC 3339), of one question and one
    /// assistant, when given.
    ///
    /// # Errors
    /// Database errors, or stored lists that are not JSON.
    pub fn ai_answers(
        &self,
        project_id: i64,
        since: Option<&str>,
        prompt_id: Option<i64>,
        engine: Option<&str>,
    ) -> Result<Vec<AiAnswerRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT run_id, project_id, prompt_id, engine, via, model, checked_at, text, mentioned,
                    position, cited, sentiment, brands, citations, error
             FROM ai_answers
             WHERE project_id = ? AND (?::VARCHAR IS NULL OR checked_at >= ?)
               AND (?::BIGINT IS NULL OR prompt_id = ?) AND (?::VARCHAR IS NULL OR engine = ?)
             ORDER BY checked_at, run_id",
        )?;
        let rows = statement.query_map(
            params![
                project_id, since, since, prompt_id, prompt_id, engine, engine
            ],
            |row| {
                Ok((
                    AiAnswerRow {
                        run_id: row.get(0)?,
                        project_id: row.get(1)?,
                        prompt_id: row.get(2)?,
                        engine: row.get(3)?,
                        via: row.get(4)?,
                        model: row.get(5)?,
                        checked_at: row.get(6)?,
                        text: row.get(7)?,
                        mentioned: row.get(8)?,
                        position: row.get(9)?,
                        cited: row.get(10)?,
                        sentiment: row.get(11)?,
                        brands: Vec::new(),
                        citations: Vec::new(),
                        error: row.get(14)?,
                    },
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (mut answer, brands, citations) = row?;
            answer.brands = parse(&brands)?;
            answer.citations = parse(&citations)?;
            Ok(answer)
        })
        .collect()
    }

    /// Removes everything a project's AI visibility holds.
    pub(crate) fn delete_ai_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in ["ai_answers", "ai_runs", "ai_prompts", "ai_settings"] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
