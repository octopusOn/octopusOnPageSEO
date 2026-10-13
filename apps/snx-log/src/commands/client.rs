use clap::{Args, Subcommand};
use miette::IntoDiagnostic;
use seonexus_log_app::Workspace;
use seonexus_log_app::catalog::{add_client, list_clients, remove_client};

use super::output::{write_json, write_text};

#[derive(Debug, Args)]
pub(crate) struct ClientArgs {
    #[command(subcommand)]
    command: ClientCommand,
}

#[derive(Debug, Subcommand)]
enum ClientCommand {
    /// Create a client.
    Add {
        /// Client name.
        name: String,
    },
    /// List clients.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Delete a client that has no sites.
    Remove {
        /// Client name.
        name: String,
    },
}

pub(crate) fn run(workspace: &Workspace, args: &ClientArgs) -> miette::Result<()> {
    match &args.command {
        ClientCommand::Add { name } => {
            let client = add_client(workspace, name).into_diagnostic()?;
            tracing::info!(id = client.id, "client `{}` created", client.name);
            Ok(())
        }
        ClientCommand::List { json } => {
            let clients = list_clients(workspace).into_diagnostic()?;
            if *json {
                return write_json(&clients);
            }
            write_text(|out| {
                for client in &clients {
                    writeln!(out, "{:>4}  {}", client.id, client.name)?;
                }
                Ok(())
            })
        }
        ClientCommand::Remove { name } => {
            remove_client(workspace, name).into_diagnostic()?;
            tracing::info!("client `{}` removed", name.trim());
            Ok(())
        }
    }
}
