use std::path::PathBuf;

use clap::{Args, Subcommand};
use miette::IntoDiagnostic;
use seonexus_log_app::Workspace;
use seonexus_log_app::saved::{
    SaveQueryRequest, export_saved_queries, import_saved_queries, list_saved_queries,
    remove_saved_query, save_query, saved_query,
};

use super::output::{write_json, write_text};

#[derive(Debug, Args)]
pub(crate) struct SavedArgs {
    #[command(subcommand)]
    command: SavedCommand,
}

#[derive(Debug, Subcommand)]
enum SavedCommand {
    /// List saved queries.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Save a query under a name (run it later with `snx-log query SITE --saved NAME`).
    Add {
        /// Name of the saved query.
        name: String,
        /// Query text; several words are joined with spaces.
        #[arg(required = true, value_name = "QUERY")]
        query: Vec<String>,
        /// Description shown next to the name.
        #[arg(long, default_value = "")]
        description: String,
        /// Replace an existing query of the same name.
        #[arg(long)]
        force: bool,
    },
    /// Show one saved query.
    Show {
        /// Name of the saved query.
        name: String,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Delete a saved query.
    Remove {
        /// Name of the saved query.
        name: String,
    },
    /// Write all saved queries to a JSON file (to share them with colleagues).
    Export {
        /// Destination file.
        path: PathBuf,
    },
    /// Add the queries of a JSON file written by `saved export`.
    Import {
        /// Source file.
        path: PathBuf,
        /// Replace queries that already exist.
        #[arg(long)]
        force: bool,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn run(workspace: &Workspace, args: &SavedArgs) -> miette::Result<()> {
    match &args.command {
        SavedCommand::List { json } => {
            let queries = list_saved_queries(workspace).into_diagnostic()?;
            if *json {
                return write_json(&queries);
            }
            write_text(|out| {
                for query in &queries {
                    writeln!(out, "{}", query.name)?;
                    if !query.description.is_empty() {
                        writeln!(out, "    {}", query.description)?;
                    }
                    writeln!(out, "    {}\n", query.query)?;
                }
                Ok(())
            })
        }
        SavedCommand::Add {
            name,
            query,
            description,
            force,
        } => {
            let request = SaveQueryRequest {
                name: name.clone(),
                description: description.clone(),
                query: query.join(" "),
                overwrite: *force,
            };
            let saved = save_query(workspace, &request).into_diagnostic()?;
            tracing::info!(query = %saved.query, "saved `{}`", saved.name);
            Ok(())
        }
        SavedCommand::Show { name, json } => {
            let saved = saved_query(workspace, name).into_diagnostic()?;
            if *json {
                return write_json(&saved);
            }
            write_text(|out| {
                writeln!(out, "Name         {}", saved.name)?;
                if !saved.description.is_empty() {
                    writeln!(out, "Description  {}", saved.description)?;
                }
                writeln!(out, "Query        {}", saved.query)?;
                writeln!(out, "Updated      {}", saved.updated_at)
            })
        }
        SavedCommand::Remove { name } => {
            remove_saved_query(workspace, name).into_diagnostic()?;
            tracing::info!("saved query `{}` removed", name.trim());
            Ok(())
        }
        SavedCommand::Export { path } => {
            let count = export_saved_queries(workspace, path).into_diagnostic()?;
            tracing::info!(count, "saved queries written to {}", path.display());
            Ok(())
        }
        SavedCommand::Import { path, force, json } => {
            let report = import_saved_queries(workspace, path, *force).into_diagnostic()?;
            if *json {
                return write_json(&report);
            }
            for skipped in &report.skipped {
                tracing::warn!("skipped `{}`: {}", skipped.name, skipped.reason);
            }
            tracing::info!(
                added = report.added.len(),
                replaced = report.replaced.len(),
                skipped = report.skipped.len(),
                "saved queries imported"
            );
            Ok(())
        }
    }
}
