//! Keyword lists from files: exports of Ahrefs, Semrush, Google Keyword Planner, Search
//! Console and Wordstat, other CSV and Excel tables, or plain lists with one keyword per
//! line. Known columns bring their numbers along.

mod columns;
mod read;

use std::path::Path;

use seonexus_kw_store::{MetricRecord, MonthlyVolume, RunKind, RunStatus};
use seonexus_text::Language;
use serde::{Deserialize, Serialize};

pub use columns::ColumnRole;

use crate::keywords::{AddOutcome, fill_intents, prepare};
use crate::{KeywordWorkspace, KwAppError, now};
use columns::{competition, count, decimal, role, volume};
use read::{FileKind, MAX_FILE, rows};

/// Source name of imported keywords and numbers.
pub const SOURCE: &str = "import";
/// Data rows shown in a preview.
const PREVIEW_ROWS: usize = 20;
/// Rows a header is looked for in (exports put titles above it).
const HEADER_SEARCH: usize = 30;

/// A column of a file and what it was read as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportColumn {
    /// Header text (empty for files without a header).
    pub header: String,
    /// What it means; `None` for columns that are left out.
    pub role: Option<ColumnRole>,
}

/// The start of a file, before importing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreview {
    /// Columns.
    pub columns: Vec<ImportColumn>,
    /// The first data rows.
    pub rows: Vec<Vec<String>>,
    /// Data rows in the file.
    pub total: usize,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportOutcome {
    /// Data rows read.
    pub rows: u64,
    /// Keywords added, known and rejected.
    #[serde(flatten)]
    pub keywords: AddOutcome,
    /// Keywords whose numbers came along.
    pub metrics: u64,
    /// Columns and what they were read as.
    pub columns: Vec<ImportColumn>,
}

/// A file as a table.
struct Table {
    columns: Vec<ImportColumn>,
    keyword: usize,
    data: Vec<Vec<String>>,
}

impl Table {
    /// The header is the first row (of the first [`HEADER_SEARCH`]) with a keyword column;
    /// without one, the first column holds the keywords. Each role is taken by its first
    /// column only.
    fn new(mut rows: Vec<Vec<String>>) -> Self {
        let header = rows.iter().take(HEADER_SEARCH).position(|row| {
            row.iter()
                .any(|cell| role(cell) == Some(ColumnRole::Keyword))
        });
        let Some(at) = header else {
            let width = rows.iter().map(Vec::len).max().unwrap_or(1);
            let columns = (0..width.max(1))
                .map(|column| ImportColumn {
                    header: String::new(),
                    role: (column == 0).then_some(ColumnRole::Keyword),
                })
                .collect();
            return Self {
                columns,
                keyword: 0,
                data: rows,
            };
        };
        let data = rows.split_off(at + 1);
        let mut columns: Vec<ImportColumn> = Vec::new();
        for header in rows.pop().unwrap_or_default() {
            let role = role(&header).filter(|role| {
                !columns
                    .iter()
                    .any(|column| column.role.as_ref() == Some(role))
            });
            columns.push(ImportColumn { header, role });
        }
        let keyword = columns
            .iter()
            .position(|column| column.role == Some(ColumnRole::Keyword))
            .unwrap_or(0);
        Self {
            columns,
            keyword,
            data,
        }
    }

    /// Numbers of a data row; `None` when it has none.
    fn numbers(&self, row: &[String]) -> Option<Numbers> {
        let mut numbers = Numbers::default();
        for (column, cell) in self.columns.iter().zip(row) {
            if cell.trim().is_empty() {
                continue;
            }
            match &column.role {
                Some(ColumnRole::Volume) => {
                    if let Some((value, low, high)) = volume(cell) {
                        numbers.volume = value;
                        numbers.volume_low = low;
                        numbers.volume_high = high;
                    }
                }
                Some(ColumnRole::Cpc) => numbers.cpc = decimal(cell).filter(|cpc| *cpc >= 0.0),
                Some(ColumnRole::Competition) => {
                    numbers.competition = numbers.competition.or_else(|| competition(cell));
                }
                Some(ColumnRole::CompetitionIndex) => {
                    if let Some(index) = decimal(cell).filter(|index| (0.0..=100.0).contains(index))
                    {
                        numbers.competition = Some(index / 100.0);
                    }
                }
                Some(ColumnRole::Difficulty) => {
                    numbers.difficulty = decimal(cell).filter(|kd| (0.0..=100.0).contains(kd));
                }
                Some(ColumnRole::Month(month)) => {
                    if let Some(volume) = count(cell) {
                        numbers.monthly.push(MonthlyVolume {
                            month: month.clone(),
                            volume,
                        });
                    }
                }
                Some(
                    ColumnRole::Keyword
                    | ColumnRole::Clicks
                    | ColumnRole::Impressions
                    | ColumnRole::Position,
                )
                | None => {}
            }
        }
        numbers.monthly.sort_by(|a, b| a.month.cmp(&b.month));
        numbers.any().then_some(numbers)
    }
}

/// Numbers of one imported row.
#[derive(Debug, Default)]
struct Numbers {
    volume: Option<u64>,
    volume_low: Option<u64>,
    volume_high: Option<u64>,
    cpc: Option<f64>,
    competition: Option<f64>,
    difficulty: Option<f64>,
    monthly: Vec<MonthlyVolume>,
}

impl Numbers {
    fn any(&self) -> bool {
        self.volume.is_some()
            || self.volume_low.is_some()
            || self.cpc.is_some()
            || self.competition.is_some()
            || self.difficulty.is_some()
            || !self.monthly.is_empty()
    }
}

fn load(path: &Path) -> Result<Table, KwAppError> {
    let kind = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(FileKind::of)
        .ok_or_else(|| {
            KwAppError::Invalid(format!(
                "`{}`: import a .csv, .tsv, .txt, .xlsx or Search Console .zip file",
                path.display()
            ))
        })?;
    let size = std::fs::metadata(path).map_err(KwAppError::io(path))?.len();
    if size > MAX_FILE {
        return Err(KwAppError::invalid_file(
            path,
            format!("larger than {} MiB", MAX_FILE >> 20),
        ));
    }
    let bytes = std::fs::read(path).map_err(KwAppError::io(path))?;
    let rows = rows(&bytes, kind).map_err(|error| KwAppError::invalid_file(path, error))?;
    Ok(Table::new(rows))
}

impl KeywordWorkspace {
    /// The columns and first rows of a file, as an import would read them.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown file type; unreadable or invalid files.
    pub fn preview_import(&self, path: &Path) -> Result<ImportPreview, KwAppError> {
        let table = load(path)?;
        Ok(ImportPreview {
            rows: table.data.iter().take(PREVIEW_ROWS).cloned().collect(),
            total: table.data.len(),
            columns: table.columns,
        })
    }

    /// Imports the keywords of a file, with the numbers its known columns carry (stored as
    /// source `import` for the project's primary engine and country).
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown file type; unreadable or invalid files; store
    /// errors.
    pub fn import(&self, project_id: i64, path: &Path) -> Result<ImportOutcome, KwAppError> {
        let table = load(path)?;
        let mut store = self.store()?;
        let project = store.project(project_id)?;
        let language = Language::from_code(&project.spec.language);
        let started = now();
        let rows = u32::try_from(table.data.len()).unwrap_or(u32::MAX);
        let run = store.begin_run(RunKind::Import, Some(project_id), rows, 0.0, &started)?;
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let (keywords, rejected) = prepare(
            table.data.iter().map(|row| {
                (
                    row.get(table.keyword).cloned().unwrap_or_default(),
                    file.clone(),
                )
            }),
            language,
            SOURCE,
        );
        let added = store.add_keywords(project_id, &keywords, &started)?;
        let index = store.key_index(project_id)?;
        let engine = project
            .spec
            .engines
            .first()
            .cloned()
            .unwrap_or_else(|| "google".to_owned());
        let metrics: Vec<MetricRecord> = table
            .data
            .iter()
            .filter_map(|row| {
                let text = row.get(table.keyword)?;
                let key = seonexus_keywords::normalize(text, language)?.key;
                let keyword_id = *index.get(&key)?;
                let numbers = table.numbers(row)?;
                Some(MetricRecord {
                    keyword_id,
                    source: SOURCE.to_owned(),
                    engine: engine.clone(),
                    country: project.spec.country.clone(),
                    volume: numbers.volume,
                    volume_low: numbers.volume_low,
                    volume_high: numbers.volume_high,
                    cpc_usd: numbers.cpc,
                    competition: numbers.competition,
                    difficulty: numbers.difficulty,
                    monthly: numbers.monthly,
                    fetched_at: started.clone(),
                })
            })
            .collect();
        store.put_metrics(&metrics)?;
        fill_intents(&mut store, project_id, &project.spec.language)?;
        store.finish_run(run, RunStatus::Done, rows, &now(), None)?;
        Ok(ImportOutcome {
            rows: u64::from(rows),
            keywords: AddOutcome { added, rejected },
            metrics: u64::try_from(metrics.len()).unwrap_or(u64::MAX),
            columns: table.columns,
        })
    }
}

#[cfg(test)]
mod tests;
