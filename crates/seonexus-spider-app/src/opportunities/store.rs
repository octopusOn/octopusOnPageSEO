use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use super::{
    AnalysisRequest, BusinessGoal, DataSource, GoogleConnection, GoogleStatus, ImportPreview,
    ImportRequest, MAX_DATASET_ROWS, OpportunityProject, OpportunityReport, PerformanceDataset,
    imports, new_id, normalized_url, oauth,
};
use serde::{Deserialize, Serialize};

const MAX_EXPANDED: u64 = 128 * 1024 * 1024;
const MAX_COMPRESSED: u64 = 64 * 1024 * 1024;
type Lock = Arc<Mutex<()>>;

/// Single structured, compressed module store; source HTTP responses and files are not retained.
#[derive(Debug, Clone)]
pub struct OpportunityWorkspace {
    pub(crate) root: PathBuf,
    pub(crate) lock: Lock,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Database {
    #[serde(default = "version")]
    pub version: u32,
    pub projects: Vec<OpportunityProject>,
    pub datasets: Vec<PerformanceDataset>,
    #[serde(default)]
    pub connections: Vec<GoogleConnection>,
}
fn version() -> u32 {
    1
}

#[derive(Debug, Serialize)]
pub struct WorkspaceStatus {
    pub projects: Vec<OpportunityProject>,
    pub bytes: u64,
    pub datasets: usize,
    pub rows: usize,
    pub connections: Vec<GoogleConnection>,
    pub google: GoogleStatus,
}

impl OpportunityWorkspace {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Arc::new(Mutex::new(())),
        }
    }
    pub fn for_spider(workspace: &crate::SpiderWorkspace) -> Self {
        Self::new(
            workspace
                .crawls_dir()
                .parent()
                .unwrap_or(workspace.crawls_dir())
                .join("opportunities"),
        )
    }
    fn path(&self) -> PathBuf {
        self.root.join("opportunities.json.zst")
    }
    pub(crate) fn read(&self) -> Result<Database, String> {
        let path = self.path();
        if !path.exists() {
            return Ok(Database {
                version: 1,
                ..Default::default()
            });
        }
        let file = File::open(&path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_COMPRESSED {
            return Err("Opportunity store exceeds its safety limit".into());
        }
        let decoder = zstd::stream::read::Decoder::new(file).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        decoder
            .take(MAX_EXPANDED + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_EXPANDED {
            return Err("Opportunity data exceeds 128 MiB; remove unused project data".into());
        }
        let data: Database = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot read opportunities: {e}"))?;
        if data.version != 1 {
            return Err("Unsupported opportunity store version".into());
        }
        Ok(data)
    }
    pub(crate) fn write(&self, data: &Database) -> Result<(), String> {
        self.write_with_cancel(data, &|| false)
    }
    fn write_with_cancel(&self, data: &Database, cancel: &dyn Fn() -> bool) -> Result<(), String> {
        if cancel() {
            return Err("Operation cancelled".into());
        }
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec(data).map_err(|e| e.to_string())?;
        if json.len() as u64 > MAX_EXPANDED {
            return Err("Opportunity store is full; existing data was preserved".into());
        }
        let mut temporary =
            tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        {
            let mut encoder = zstd::stream::write::Encoder::new(temporary.as_file_mut(), 3)
                .map_err(|e| e.to_string())?;
            encoder.write_all(&json).map_err(|e| e.to_string())?;
            encoder.finish().map_err(|e| e.to_string())?;
        }
        if temporary
            .as_file()
            .metadata()
            .map_err(|e| e.to_string())?
            .len()
            > MAX_COMPRESSED
        {
            return Err("Compressed opportunity store is full; existing data was preserved".into());
        }
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        if cancel() {
            return Err("Operation cancelled".into());
        }
        temporary
            .persist(self.path())
            .map_err(|e| e.error.to_string())?;
        Ok(())
    }
    /// # Errors
    /// Returns an error when the compressed module store cannot be read.
    pub fn status(&self) -> Result<WorkspaceStatus, String> {
        let guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let data = self.read()?;
        let bytes = fs::metadata(self.path()).map_or(0, |m| m.len());
        drop(guard); // OAuth configuration takes the attempts lock before the workspace lock.
        Ok(WorkspaceStatus {
            projects: data.projects,
            bytes,
            datasets: data.datasets.len(),
            rows: data.datasets.iter().map(|d| d.rows.len()).sum(),
            connections: data.connections,
            google: oauth::status(),
        })
    }
    /// # Errors
    /// Returns an error for invalid project settings or a failed atomic write.
    pub fn save_project(
        &self,
        mut project: OpportunityProject,
    ) -> Result<OpportunityProject, String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        project.site_url =
            normalized_url(project.site_url.trim()).ok_or("Choose an absolute HTTP(S) site URL")?;
        if project.name.trim().is_empty() || project.name.len() > 200 {
            return Err("Project name must contain 1–200 characters".into());
        }
        if project.goal == BusinessGoal::Event
            && (project.event_name.is_empty()
                || !project
                    .event_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        {
            return Err("Choose a valid GA4 key-event name".into());
        }
        project.name = project.name.trim().into();
        project.event_name = project.event_name.trim().into();
        project.brand_terms = project
            .brand_terms
            .into_iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if !project.ga4_property.is_empty() {
            super::google::property_id(&project.ga4_property)?;
        }
        if !project.gsc_property.is_empty() {
            let site = url::Url::parse(&project.site_url).map_err(|_| "Invalid site URL")?;
            let matches = if let Some(domain) = project.gsc_property.strip_prefix("sc-domain:") {
                site.host_str()
                    .is_some_and(|h| h == domain || h.ends_with(&format!(".{domain}")))
            } else {
                url::Url::parse(&project.gsc_property).is_ok_and(|property| {
                    property.origin() == site.origin() && site.path().starts_with(property.path())
                })
            };
            if !matches {
                return Err("GSC property does not cover this site URL".into());
            }
        }
        if project.id.is_empty() {
            project.id = new_id()?;
        }
        let mut data = self.read()?;
        if let Some(index) = data.projects.iter().position(|p| p.id == project.id) {
            data.projects[index] = project.clone();
        } else {
            data.projects.push(project.clone());
        }
        self.write(&data)?;
        Ok(project)
    }
    /// # Errors
    /// Returns an error when project data cannot be atomically removed.
    pub fn remove_project(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut data = self.read()?;
        data.projects.retain(|p| p.id != id);
        data.datasets.retain(|d| d.project_id != id);
        self.write(&data)
    }
    /// # Errors
    /// Returns an error when the module store cannot be atomically updated.
    pub fn clear_api_cache(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut data = self.read()?;
        data.datasets
            .retain(|d| d.project_id != id || d.origin != "google-api");
        self.write(&data)
    }
    pub(crate) fn project(&self, id: &str) -> Result<OpportunityProject, String> {
        self.read()?
            .projects
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "Unknown opportunity project".into())
    }
    pub(crate) fn commit_datasets(
        &self,
        project: &OpportunityProject,
        datasets: Vec<PerformanceDataset>,
    ) -> Result<(), String> {
        self.commit_datasets_with_cancel(project, datasets, &|| false)
    }
    pub(crate) fn commit_datasets_with_cancel(
        &self,
        project: &OpportunityProject,
        datasets: Vec<PerformanceDataset>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut data = self.read()?;
        let current = data
            .projects
            .iter()
            .find(|p| p.id == project.id)
            .ok_or("Project was removed while the operation was running")?;
        if serde_json::to_string(current).ok() != serde_json::to_string(project).ok() {
            return Err("Project changed; repeat the operation with current settings".into());
        }
        for dataset in datasets {
            if cancel() {
                return Err("Operation cancelled".into());
            }
            if dataset.rows.len() > MAX_DATASET_ROWS || !dataset.range.valid() {
                return Err("Invalid dataset size or period".into());
            }
            data.datasets.retain(|d| {
                !(d.project_id == dataset.project_id
                    && d.source == dataset.source
                    && d.grain == dataset.grain
                    && d.range == dataset.range)
            });
            data.datasets.push(dataset);
        }
        // Only re-fetchable API cache is bounded. User-imported datasets are never pruned.
        let mut cached: Vec<_> = data
            .datasets
            .iter()
            .filter(|d| d.project_id == project.id && d.origin == "google-api")
            .map(|d| (d.created_at_ms, d.id.clone()))
            .collect();
        cached.sort();
        let remove: BTreeSet<_> = cached
            .iter()
            .take(cached.len().saturating_sub(12))
            .map(|(_, id)| id.clone())
            .collect();
        data.datasets.retain(|d| !remove.contains(&d.id));
        self.write_with_cancel(&data, cancel)
    }
    /// # Errors
    /// Returns an error for invalid or unavailable observations, or a failed local analysis.
    pub fn analyze(&self, request: &AnalysisRequest) -> Result<OpportunityReport, String> {
        self.analyze_with_cancel(request, &|| false)
    }
    pub(crate) fn analyze_with_cancel(
        &self,
        request: &AnalysisRequest,
        cancel: &dyn Fn() -> bool,
    ) -> Result<OpportunityReport, String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let data = self.read()?;
        let project = data
            .projects
            .iter()
            .find(|p| p.id == request.project_id)
            .ok_or("Unknown opportunity project")?;
        let datasets: Vec<_> = data
            .datasets
            .into_iter()
            .filter(|d| {
                d.coverage.site_url == project.site_url
                    && d.coverage.property
                        == if d.source == DataSource::Gsc {
                            project.gsc_property.clone()
                        } else {
                            project.ga4_property.clone()
                        }
            })
            .collect();
        seonexus_opportunities::analyze_with_cancel(project, request, &datasets, cancel)
    }
    /// # Errors
    /// Returns an error for an unknown project or an unreadable, oversized or unsupported file.
    pub fn preview_import(&self, request: &ImportRequest) -> Result<ImportPreview, String> {
        let project = self.project(&request.project_id)?;
        imports::preview(request, &project)
    }
    /// # Errors
    /// Returns an error if the preview changed, required fields or valid rows are missing, or storage fails.
    pub fn import(
        &self,
        request: &ImportRequest,
        expected_digest: &str,
    ) -> Result<ImportPreview, String> {
        let project = self.project(&request.project_id)?;
        let (preview, dataset) = imports::parse(request, &project)?;
        if preview.digest != expected_digest {
            return Err("File changed since preview; preview again before importing".into());
        }
        if preview.invalid > 0 {
            return Err("Fix invalid rows or column mappings before importing".into());
        }
        if preview.valid == 0 {
            return Err("No valid observations to import".into());
        }
        let dataset = dataset.ok_or("Choose column mappings before importing")?;
        self.commit_datasets(&project, vec![dataset])?;
        Ok(preview)
    }
}
use std::collections::BTreeSet;
#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;
