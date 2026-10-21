//! The job list.

use serde::Deserialize;

use crate::AppError;
use crate::jobs::Job;
use crate::router::{Context, NoArgs, Router};

fn list_jobs(context: &Context<'_>, _: NoArgs) -> Result<Vec<Job>, AppError> {
    Ok(context.jobs.list())
}

#[derive(Debug, Deserialize)]
struct ById {
    id: String,
}

fn cancel_job(context: &Context<'_>, arguments: ById) -> Result<bool, AppError> {
    Ok(context.jobs.cancel(&arguments.id))
}

fn clear_jobs(context: &Context<'_>, _: NoArgs) -> Result<(), AppError> {
    context.jobs.clear();
    Ok(())
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("list_jobs", list_jobs);
    router.add("cancel_job", cancel_job);
    router.add("clear_jobs", clear_jobs);
}
