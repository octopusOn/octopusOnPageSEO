//! Projects: the site, its market, competitors and brand names every module works on.

use octopuson_platform::{ProjectRow, ProjectSpec};
use serde::Deserialize;

use crate::router::{Context, NoArgs, Router};
use crate::{AppError, now};

fn list_projects(context: &Context<'_>, _: NoArgs) -> Result<Vec<ProjectRow>, AppError> {
    Ok(context.workspace.store()?.projects()?)
}

#[derive(Debug, Deserialize)]
struct ById {
    id: i64,
}

fn get_project(context: &Context<'_>, arguments: ById) -> Result<ProjectRow, AppError> {
    Ok(context.workspace.store()?.project(arguments.id)?)
}

#[derive(Debug, Deserialize)]
struct Create {
    spec: ProjectSpec,
}

fn create_project(context: &Context<'_>, arguments: Create) -> Result<ProjectRow, AppError> {
    Ok(context
        .workspace
        .store()?
        .create_project(&arguments.spec, &now())?)
}

#[derive(Debug, Deserialize)]
struct Update {
    id: i64,
    spec: ProjectSpec,
}

fn update_project(context: &Context<'_>, arguments: Update) -> Result<ProjectRow, AppError> {
    Ok(context
        .workspace
        .store()?
        .update_project(arguments.id, &arguments.spec, &now())?)
}

fn delete_project(context: &Context<'_>, arguments: ById) -> Result<(), AppError> {
    super::audit::delete_crawls(context.workspace, arguments.id)?;
    Ok(context.workspace.store()?.delete_project(arguments.id)?)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("list_projects", list_projects);
    router.add("get_project", get_project);
    router.add("create_project", create_project);
    router.add("update_project", update_project);
    router.add("delete_project", delete_project);
}
