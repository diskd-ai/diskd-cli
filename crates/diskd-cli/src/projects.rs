use anyhow::{bail, Result};
use clap::Subcommand;
use diskd_client::{GatewayClient, ProjectCreateParams, ProjectUpdateParams};
use serde_json::{json, Value};

use super::{effective_base_url, effective_token, render_value, save_config, Cli, RuntimeState};

/// Groups project lifecycle operations under the project command namespace.
#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    List,
    Get {
        project_id: String,
    },
    Create {
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        icon: Option<String>,
        #[arg(long)]
        icon_color: Option<String>,
    },
    Update {
        project_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        icon: Option<String>,
        #[arg(long)]
        icon_color: Option<String>,
    },
    Delete {
        project_id: String,
        #[arg(long, help = "Confirm permanent project deletion")]
        yes: bool,
    },
}

/// Dispatches typed project REST operations through the shared gateway client.
pub(crate) fn run_project_command(
    command: &ProjectCommand,
    cli: &Cli,
    state: &mut RuntimeState,
) -> Result<()> {
    let base_url = effective_base_url(cli, state);
    let token = effective_token(state)?;
    let client = GatewayClient::new(&base_url, &token)?;

    match command {
        ProjectCommand::List => {
            let projects = client.list_projects()?;
            render_value(&serde_json::to_value(projects)?, cli.json)
        }
        ProjectCommand::Get { project_id } => {
            let project = client.get_project(project_id)?;
            render_value(&serde_json::to_value(project)?, cli.json)
        }
        ProjectCommand::Create {
            name,
            description,
            icon,
            icon_color,
        } => {
            let project = client.create_project(&ProjectCreateParams {
                name: name.clone(),
                description: description.clone(),
                icon: icon.clone(),
                icon_color: icon_color.clone(),
            })?;
            render_value(&serde_json::to_value(project)?, cli.json)
        }
        ProjectCommand::Update {
            project_id,
            name,
            description,
            icon,
            icon_color,
        } => {
            let project = client.update_project(
                project_id,
                &ProjectUpdateParams {
                    name: name.clone(),
                    description: description.clone(),
                    icon: icon.clone(),
                    icon_color: icon_color.clone(),
                },
            )?;
            render_value(&serde_json::to_value(project)?, cli.json)
        }
        ProjectCommand::Delete { project_id, yes } => {
            if !yes {
                bail!("project delete requires --yes");
            }
            client.delete_project(project_id)?;
            if state.config.project.as_deref() == Some(project_id.as_str()) {
                state.config.project = None;
                state.config.project_name = None;
                save_config(state)?;
            }
            let result: Value = json!({ "projectId": project_id, "status": "deleted" });
            render_value(&result, cli.json)
        }
    }
}
