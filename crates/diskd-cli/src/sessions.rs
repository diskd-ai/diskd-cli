use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use diskd_client::{GatewayClient, SessionDeleteMessages, SessionDocument, SessionMessage};
use diskd_config::DriveContext;
use serde::de::DeserializeOwned;

use super::{
    effective_base_url, effective_drive_context, effective_token, render_value, Cli, RuntimeState,
};

/// Groups project-scoped Drive Session operations under one namespace.
#[derive(Debug, Subcommand)]
pub(crate) enum SessionCommand {
    List,
    Read {
        session_id: String,
        #[arg(long)]
        limit: Option<u64>,
    },
    Messages {
        session_id: String,
        #[arg(long)]
        limit: u64,
        #[arg(long)]
        before: Option<String>,
    },
    Save {
        document_file: PathBuf,
        #[arg(long = "attribute")]
        attributes: Vec<String>,
    },
    Append {
        session_id: String,
        messages_file: PathBuf,
    },
    Remove {
        session_id: String,
        message_ids: Vec<String>,
    },
    Rollback {
        session_id: String,
        after_message_id: String,
    },
    Delete {
        session_id: String,
        #[arg(long, help = "Confirm permanent session deletion")]
        yes: bool,
    },
}

/// Dispatches the complete low-level Drive Session contract through APIS.
pub(crate) fn run_session_command(
    command: &SessionCommand,
    cli: &Cli,
    state: &RuntimeState,
) -> Result<()> {
    let project_id = effective_project_id(cli, state)?;
    let base_url = effective_base_url(cli, state);
    let token = effective_token(state)?;
    let mut client = GatewayClient::new(&base_url, &token)?;

    match command {
        SessionCommand::List => {
            let result = client.list_sessions(&project_id)?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
        SessionCommand::Read { session_id, limit } => {
            let value = match limit {
                Some(limit) => serde_json::to_value(client.get_session_preview(
                    &project_id,
                    session_id,
                    *limit,
                )?)?,
                None => serde_json::to_value(client.get_session(&project_id, session_id)?)?,
            };
            render_value(&value, cli.json)
        }
        SessionCommand::Messages {
            session_id,
            limit,
            before,
        } => {
            let result = client.get_session_message_range(
                &project_id,
                session_id,
                *limit,
                before.as_deref(),
            )?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
        SessionCommand::Save {
            document_file,
            attributes,
        } => {
            let document: SessionDocument = read_typed_json(document_file, "session document")?;
            let result = client.save_session(&project_id, &document, attributes)?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
        SessionCommand::Append {
            session_id,
            messages_file,
        } => {
            let messages: Vec<SessionMessage> = read_typed_json(messages_file, "session messages")?;
            let result = client.append_session_messages(&project_id, session_id, &messages)?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
        SessionCommand::Remove {
            session_id,
            message_ids,
        } => {
            let result = client.delete_session_messages(
                &project_id,
                session_id,
                &SessionDeleteMessages::ByIds(message_ids.clone()),
            )?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
        SessionCommand::Rollback {
            session_id,
            after_message_id,
        } => {
            let result = client.delete_session_messages(
                &project_id,
                session_id,
                &SessionDeleteMessages::RollbackAfter(after_message_id.clone()),
            )?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
        SessionCommand::Delete { session_id, yes } => {
            if !yes {
                bail!("session delete requires --yes");
            }
            let result = client.delete_session(&project_id, session_id)?;
            render_value(&serde_json::to_value(result)?, cli.json)
        }
    }
}

fn effective_project_id(cli: &Cli, state: &RuntimeState) -> Result<String> {
    match effective_drive_context(cli, state)? {
        DriveContext::Project(project_id) => Ok(project_id.as_str().to_owned()),
        DriveContext::WorkspaceRoot => {
            bail!("session commands require --project or a saved project context")
        }
    }
}

fn read_typed_json<T>(path: &Path, label: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read {label} from {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {label} from {}", path.display()))
}
