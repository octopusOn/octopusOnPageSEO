//! The Sources page: own keys, own key or credits per source, budgets, spending.

use octopuson_cloud::SourceMode;
use octopuson_platform::UsageRow;
use serde::Deserialize;

use crate::AppError;
use crate::router::{Context, NoArgs, Router};
use crate::sources::ProviderInfo;

fn list_providers(context: &Context<'_>, _: NoArgs) -> Result<Vec<ProviderInfo>, AppError> {
    context.workspace.providers()
}

#[derive(Debug, Deserialize)]
struct SetKey {
    provider: String,
    name: String,
    value: String,
}

fn set_key(context: &Context<'_>, arguments: SetKey) -> Result<(), AppError> {
    context
        .workspace
        .set_key(&arguments.provider, &arguments.name, &arguments.value)
}

#[derive(Debug, Deserialize)]
struct RemoveKey {
    provider: String,
    name: String,
}

fn remove_key(context: &Context<'_>, arguments: RemoveKey) -> Result<(), AppError> {
    context
        .workspace
        .remove_key(&arguments.provider, &arguments.name)
}

#[derive(Debug, Deserialize)]
struct SetMode {
    provider: String,
    mode: SourceMode,
}

fn set_source_mode(context: &Context<'_>, arguments: SetMode) -> Result<(), AppError> {
    context
        .workspace
        .set_source_mode(&arguments.provider, arguments.mode)
}

#[derive(Debug, Deserialize)]
struct SetBudget {
    provider: String,
    usd: Option<f64>,
}

fn set_budget(context: &Context<'_>, arguments: SetBudget) -> Result<(), AppError> {
    context
        .workspace
        .set_budget(&arguments.provider, arguments.usd)
}

fn usage(context: &Context<'_>, _: NoArgs) -> Result<Vec<UsageRow>, AppError> {
    Ok(context.workspace.store()?.usage()?)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("list_providers", list_providers);
    router.add("set_key", set_key);
    router.add("remove_key", remove_key);
    router.add("set_source_mode", set_source_mode);
    router.add("set_budget", set_budget);
    router.add("usage", usage);
}
