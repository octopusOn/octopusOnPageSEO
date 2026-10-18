//! Bounded live state and lifecycle transitions, independent of Tauri event delivery.

use std::collections::HashMap;

use seonexus_spider_app::{CrawlEvent, CrawlMeta, CrawlPhase, CrawlStatus, SpiderError};

use crate::{Job, RunningJob};

impl Job {
    pub(crate) fn observe(&mut self, event: &CrawlEvent<'_>) -> bool {
        match event {
            CrawlEvent::Created(meta) => self.crawl_id = Some(meta.id.clone()),
            CrawlEvent::Phase(phase) => self.phase = *phase,
            CrawlEvent::Activity(activity) => {
                self.activity.clone_from(activity);
                return false;
            }
            CrawlEvent::Progress(progress) => self.progress = **progress,
        }
        self.revision += 1;
        true
    }

    pub(crate) fn finish(&mut self, result: Result<CrawlMeta, SpiderError>) {
        self.activity.active.clear();
        // Terminal jobs are polled for reconnects; their full URL results are already stored.
        self.activity.recent.clear();
        self.revision += 1;
        match result {
            Ok(meta) => {
                self.progress = meta.progress;
                self.crawl_id = Some(meta.id.clone());
                self.status = match meta.status {
                    CrawlStatus::Stopped => "stopped",
                    CrawlStatus::Failed => "failed",
                    _ => "completed",
                }
                .into();
                self.meta = Some(meta);
            }
            Err(error) => {
                self.status = "failed".into();
                self.error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "running" | "paused" | "stopping")
    }

    pub(crate) fn can_control(&self) -> bool {
        self.is_active() && self.phase != CrawlPhase::Finalizing
    }
}

/// Keep recent terminal jobs for reconnects, including a completion just emitted.
pub(crate) fn prune(jobs: &mut HashMap<String, RunningJob>) {
    let mut finished: Vec<_> = jobs
        .iter()
        .filter(|(_, job)| !job.job.is_active())
        .map(|(id, job)| (job.order, id.clone()))
        .collect();
    finished.sort_unstable_by_key(|(order, _)| std::cmp::Reverse(*order));
    for (_, id) in finished.into_iter().skip(64) {
        jobs.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seonexus_spider_app::{
        CrawlActivity, CrawlConfig, CrawlControl, CrawlMode, CrawlProgress, RenderMode,
    };
    use std::sync::Arc;

    fn job(order: u64) -> RunningJob {
        RunningJob {
            order,
            control: Arc::new(CrawlControl::new()),
            job: Job {
                id: format!("job-{order}"),
                url: "https://example.com/".into(),
                rendering: RenderMode::Html,
                status: "running".into(),
                progress: CrawlProgress::default(),
                crawl_id: None,
                phase: CrawlPhase::Starting,
                activity: CrawlActivity::default(),
                revision: 0,
                meta: None,
                error: None,
            },
        }
    }

    fn meta(status: CrawlStatus) -> CrawlMeta {
        CrawlMeta {
            id: "saved-crawl".into(),
            name: "example.com".into(),
            start_urls: vec!["https://example.com/".into()],
            mode: CrawlMode::Spider,
            created_at_ms: 1,
            finished_at_ms: Some(2),
            status,
            progress: CrawlProgress::default(),
            limit_reached: false,
            error: None,
            config: CrawlConfig::default(),
        }
    }

    #[test]
    fn identity_phase_and_progress_share_monotonic_revisions() {
        let mut job = job(1).job;
        assert!(job.observe(&CrawlEvent::Created(&meta(CrawlStatus::Running))));
        assert_eq!(job.crawl_id.as_deref(), Some("saved-crawl"));
        assert_eq!(job.revision, 1);
        let activity = CrawlActivity {
            rendered: 12,
            ..CrawlActivity::default()
        };
        assert!(!job.observe(&CrawlEvent::Activity(&activity)));
        assert_eq!(job.revision, 1);
        job.status = "paused".into();
        assert!(job.observe(&CrawlEvent::Progress(&CrawlProgress {
            crawled: 12,
            ..CrawlProgress::default()
        })));
        assert_eq!(job.activity.rendered, job.progress.crawled);
        assert_eq!(job.status, "paused");
        assert_eq!(job.revision, 2);
        assert!(job.can_control());
        job.observe(&CrawlEvent::Phase(CrawlPhase::Finalizing));
        assert!(!job.can_control());
        job.finish(Ok(meta(CrawlStatus::Stopped)));
        assert_eq!(job.status, "stopped");
        assert!(!job.is_active());
        assert!(!job.can_control());
        assert_eq!(job.revision, 4);
    }

    #[test]
    fn retains_the_latest_completions_and_all_active_jobs() {
        let mut jobs = HashMap::new();
        for order in 0..90 {
            let mut running = job(order);
            if order != 2 {
                running.job.status = "completed".into();
            }
            jobs.insert(running.job.id.clone(), running);
        }
        prune(&mut jobs);
        assert_eq!(jobs.len(), 65);
        assert!(jobs.contains_key("job-2"));
        assert!(jobs.contains_key("job-89"));
        assert!(jobs.contains_key("job-26"));
        assert!(!jobs.contains_key("job-25"));
    }
}
