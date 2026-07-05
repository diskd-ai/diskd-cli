use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, BufReader, Cursor, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use diskd_client::{
    biquery_request, database_commit_request, database_create_request, database_drop_request,
    database_insert_request, database_metadata_request, database_query_request,
    database_resolve_by_inode_request, database_resolve_with_settings_request,
    database_rollback_request, database_set_status_request, decode_upload_start,
    download_url_request, glob_request, grep_request, ls_request, metadata_request,
    path_create_request, path_delete_request, path_rename_request, read_file_request,
    request_client_credentials_token, telegram_db_commit_request, telegram_db_create_request,
    telegram_db_drop_request, telegram_db_insert_request, telegram_db_metadata_request,
    telegram_db_query_request, upload_commit_request, upload_start_request, vsearch_request,
    ClientCredentialsTokenParams, GatewayClient, JsonRpcRequest,
};
use diskd_config::{
    decode_jwt_identity, format_config_document, format_stored_credentials, normalize_drive_path,
    parse_client_credentials_file, parse_config_document, parse_stored_credentials,
    resolve_setting, ClientCredentialsFile, DiskdConfig, DriveContext, ProjectId,
    StoredCredentials,
};
use flate2::read::GzDecoder;
use reqwest::blocking::Client as HttpClient;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tar::Archive;

const DEFAULT_BASE_URL: &str = "https://apis.iosya.com";
const DEFAULT_LOGIN_APP_URL: &str = "https://app.iosya.com/oauth-apps";
const DEV_LOGIN_APP_URL: &str = "https://app.upgraide.dev/oauth-apps";
const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITHUB_REPOSITORY: &str = "diskd-ai/diskd-cli";
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_millis(900);
const UPDATE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const UPDATE_CHECK_DISABLE_ENV: &str = "DISKD_NO_UPDATE_CHECK";
const BROWSER_LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const TOKEN_SCOPES: &[&str] = &[
    "drive:read",
    "drive:write",
    "projects:read",
    "projects:write",
];

/// Parses command-line flags and dispatches the requested diskd operation.
#[derive(Debug, Parser)]
#[command(
    name = "diskd",
    version,
    about = "Command-line client for the diskd drive"
)]
struct Cli {
    #[arg(short = 'w', long)]
    workspace: Option<String>,
    #[arg(short = 'p', long)]
    project: Option<String>,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    json: bool,
    #[arg(short, long)]
    quiet: bool,
    #[arg(long)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

/// Enumerates the public command surface exposed by the CLI binary.
#[derive(Debug, Subcommand)]
enum Command {
    Ls {
        path: Option<String>,
        #[arg(long)]
        recursive: bool,
        #[arg(long)]
        long: bool,
        #[arg(long)]
        show_hidden: bool,
        #[arg(long)]
        show_system: bool,
    },
    Tree {
        path: Option<String>,
        #[arg(short = 'L', long = "depth", alias = "level", alias = "deep")]
        depth: Option<usize>,
        #[arg(short = 'a', long = "all", alias = "show-hidden")]
        all: bool,
        #[arg(short = 'd', long)]
        dirs_only: bool,
        #[arg(short = 'f', long)]
        full_path: bool,
        #[arg(short = 's', long)]
        size: bool,
        #[arg(long)]
        show_system: bool,
    },
    Glob {
        pattern: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        show_hidden: bool,
        #[arg(long)]
        show_system: bool,
    },
    Grep {
        query: String,
        paths: Vec<String>,
        #[arg(long)]
        limit: Option<u64>,
        #[arg(long)]
        offset: Option<u64>,
        #[arg(long)]
        ignore_case: bool,
        #[arg(long)]
        files_with_matches: bool,
    },
    Vsearch {
        query: String,
        paths: Vec<String>,
        #[arg(long, alias = "top")]
        limit: Option<u64>,
        #[arg(long)]
        offset: Option<u64>,
    },
    Cat {
        path: String,
        #[arg(long)]
        version: Option<u64>,
    },
    Read {
        path: String,
        #[arg(long, alias = "limit")]
        parts_limit: Option<u64>,
        #[arg(long, alias = "offset")]
        parts_offset: Option<u64>,
    },
    Stat {
        path: String,
    },
    Biquery {
        query: String,
        paths: Vec<String>,
    },
    #[command(alias = "db")]
    Database {
        #[command(subcommand)]
        command: DatabaseCommand,
    },
    TelegramDb {
        #[command(subcommand)]
        command: TelegramDbCommand,
    },
    Upload {
        local: Vec<PathBuf>,
        #[arg(long)]
        dest: Option<String>,
        #[arg(long)]
        recursive: bool,
        #[arg(long)]
        force: bool,
    },
    Mkdir {
        path: String,
    },
    Rm {
        path: String,
        #[arg(long)]
        recursive: bool,
    },
    Mv {
        src: String,
        dst: String,
    },
    Cp {
        src: String,
        dst: String,
        #[arg(long)]
        force: bool,
    },
    Sync {
        folder: PathBuf,
        #[arg(long)]
        dest: Option<String>,
        #[arg(long)]
        once: bool,
        #[arg(long, default_value_t = 2)]
        interval_seconds: u64,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    Login {
        #[arg(long, help = "Store an existing bearer token")]
        token: Option<String>,
        #[arg(
            long,
            help = "Exchange a downloaded credentials.json file for a bearer token"
        )]
        credentials_file: Option<PathBuf>,
        #[arg(long, help = "Open the development app host for browser login")]
        dev: bool,
        #[arg(long, help = "Override the OAuth Apps URL used by browser login")]
        app_url: Option<String>,
    },
    Logout,
    Whoami,
    SetContext {
        project: Option<String>,
        #[arg(long)]
        list: bool,
        #[arg(long, alias = "clear")]
        root: bool,
    },
    GetContext,
    Version,
    Update {
        #[arg(long)]
        force: bool,
    },
}

/// Groups MCP subcommands under the mcp command namespace.
#[derive(Debug, Subcommand)]
enum McpCommand {
    Serve,
}

/// Groups generic Drive DB operations under one command namespace.
#[derive(Debug, Subcommand)]
enum DatabaseCommand {
    Create {
        name: String,
        #[arg(long = "schema", alias = "schema-json")]
        schema_json: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        check_exists: bool,
        #[arg(long)]
        recreate: bool,
        #[arg(long)]
        directory: Option<String>,
        #[arg(long)]
        db_type: Option<String>,
    },
    Insert {
        name: String,
        table: String,
        #[arg(long = "rows", alias = "rows-json")]
        rows_json: Option<String>,
        #[arg(long)]
        rows_file: Option<PathBuf>,
        #[arg(long)]
        db_type: Option<String>,
    },
    Query {
        name: String,
        sql: String,
        #[arg(long = "parameters", alias = "params-json")]
        parameters_json: Option<String>,
        #[arg(long, alias = "params-file")]
        parameters_file: Option<PathBuf>,
        #[arg(long)]
        db_type: Option<String>,
    },
    Commit {
        name: String,
        #[arg(long)]
        db_type: Option<String>,
    },
    Rollback {
        name: String,
        #[arg(long)]
        db_type: Option<String>,
    },
    Metadata {
        name: String,
        #[arg(long)]
        db_type: Option<String>,
    },
    Drop {
        name: String,
        #[arg(long)]
        db_type: Option<String>,
    },
    SetStatus {
        name: String,
        status: String,
        #[arg(long)]
        error: Option<String>,
        #[arg(long)]
        db_type: Option<String>,
    },
    ResolveByInode {
        db_inode: String,
        #[arg(long)]
        db_type: Option<String>,
    },
    ResolveWithSettings {
        db_inode: String,
        #[arg(long)]
        db_type: Option<String>,
    },
}

/// Groups Telegram Drive DB operations under one command namespace.
#[derive(Debug, Subcommand)]
enum TelegramDbCommand {
    Create {
        name: String,
        #[arg(long = "schema", alias = "schema-json")]
        schema_json: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        recreate: bool,
        #[arg(long)]
        directory: Option<String>,
    },
    Insert {
        name: String,
        table: String,
        #[arg(long = "rows", alias = "rows-json")]
        rows_json: Option<String>,
        #[arg(long)]
        rows_file: Option<PathBuf>,
    },
    Query {
        name: String,
        sql: String,
        #[arg(long = "parameters", alias = "params-json")]
        parameters_json: Option<String>,
        #[arg(long, alias = "params-file")]
        parameters_file: Option<PathBuf>,
    },
    Commit {
        name: String,
    },
    Metadata {
        name: String,
    },
    Drop {
        name: String,
    },
}

/// Carries resolved diskd paths and non-secret config loaded at process start.
#[derive(Debug, Clone)]
struct RuntimeState {
    home_dir: PathBuf,
    config_path: PathBuf,
    credentials_path: PathBuf,
    config: DiskdConfig,
}

/// Describes a local file selected for upload with its Drive-relative target path.
#[derive(Debug, Clone)]
struct UploadFile {
    local_path: PathBuf,
    relative_path: PathBuf,
}

/// Represents the GitHub release shape used by update checks and installs.
#[derive(Debug, Clone, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubReleaseAsset>,
}

/// Represents one downloadable GitHub release asset.
#[derive(Debug, Clone, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// Carries a matching platform archive and checksum selected from a release.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseAssetPair {
    archive_url: String,
    checksum_url: String,
}

/// Describes an available release that can update the running binary.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AvailableUpdate {
    current_version: String,
    latest_version: String,
    release_url: String,
    assets: ReleaseAssetPair,
}

/// Starts the diskd CLI process and converts failures into non-zero exits.
fn main() {
    if let Err(error) = run() {
        eprintln!("diskd: {error:#}");
        std::process::exit(1);
    }
}

/// Runs command dispatch after parsing CLI arguments.
fn run() -> Result<()> {
    let cli = Cli::parse();
    if should_show_mcp_setup_instructions(&cli) {
        return print_mcp_setup_instructions();
    }
    maybe_show_update_notice(&cli);
    match &cli.command {
        Command::Version => return print_version(&cli),
        Command::Update { force } => return update_cli(&cli, *force),
        _ => {}
    }

    let mut state = load_runtime_state(cli.config.as_deref())?;
    match &cli.command {
        Command::Login {
            token,
            credentials_file,
            dev,
            app_url,
        } => login(
            &mut state,
            token.as_deref(),
            credentials_file.as_deref(),
            *dev,
            app_url.as_deref(),
            cli.quiet,
        ),
        Command::Logout => logout(&state, cli.quiet),
        Command::Whoami => whoami(&cli, &state),
        Command::SetContext {
            project,
            list,
            root,
        } => set_context(&cli, &mut state, project.as_deref(), *list, *root),
        Command::GetContext => get_context(&cli, &state),
        Command::Mcp {
            command: McpCommand::Serve,
        } => run_mcp_serve(&cli, &state),
        _ => run_drive_command(&cli, &state),
    }
}

/// Dispatches commands that require an authenticated Drive gateway client.
fn run_drive_command(cli: &Cli, state: &RuntimeState) -> Result<()> {
    let base_url = effective_base_url(cli, state);
    let token = effective_token(state)?;
    let context = effective_drive_context(cli, state)?;
    let mut client = GatewayClient::new(&base_url, &token)?;

    match &cli.command {
        Command::Ls {
            path,
            recursive,
            long,
            show_hidden,
            show_system,
        } => {
            let path = normalize_drive_path(&context, path.as_deref())?;
            let result = client.call_drive(&ls_request(
                Some(path.as_str()),
                flag_opt(*recursive),
                flag_opt(*show_hidden),
                flag_opt(*show_system),
            ))?;
            render_ls(&result, cli.json, *long)
        }
        Command::Tree {
            path,
            depth,
            all,
            dirs_only,
            full_path,
            size,
            show_system,
        } => {
            let normalized_path = normalize_drive_path(&context, path.as_deref())?;
            let result = collect_tree_listing(
                &mut client,
                normalized_path.as_str(),
                *depth,
                *all,
                *show_system,
            )?;
            let root_label = tree_root_label(path.as_deref(), normalized_path.as_str(), *full_path);
            render_tree(
                &result,
                cli.json,
                &root_label,
                normalized_path.as_str(),
                TreeRenderOptions {
                    depth: *depth,
                    dirs_only: *dirs_only,
                    full_path: *full_path,
                    show_size: *size,
                },
            )
        }
        Command::Glob {
            pattern,
            path,
            show_hidden,
            show_system,
        } => {
            let path = normalize_drive_path(&context, path.as_deref())?;
            let result = client.call_drive(&glob_request(
                pattern,
                Some(path.as_str()),
                flag_opt(*show_hidden),
                flag_opt(*show_system),
            ))?;
            render_value(&result, cli.json)
        }
        Command::Grep {
            query,
            paths,
            limit,
            offset,
            ignore_case,
            files_with_matches,
        } => {
            reject_unsupported_flag(*ignore_case, "--ignore-case")?;
            reject_unsupported_flag(*files_with_matches, "--files-with-matches")?;
            let paths = normalize_many_paths(&context, paths)?;
            let result = client.call_drive(&grep_request(query, &paths, *limit, *offset))?;
            render_value(&result, cli.json)
        }
        Command::Vsearch {
            query,
            paths,
            limit,
            offset,
        } => {
            let paths = normalize_many_paths(&context, paths)?;
            let result = client.call_drive(&vsearch_request(query, &paths, *limit, *offset))?;
            render_value(&result, cli.json)
        }
        Command::Cat { path, version } => {
            let path = normalize_drive_path(&context, Some(path))?;
            let result = client.call_drive(&download_url_request(path.as_str(), *version))?;
            let url = read_string_field(&result, "url")?;
            let bytes = client.download_bytes(&url)?;
            io::stdout().write_all(&bytes)?;
            Ok(())
        }
        Command::Read {
            path,
            parts_limit,
            parts_offset,
        } => {
            let path = normalize_drive_path(&context, Some(path))?;
            let result = client.call_drive(&read_file_request(
                path.as_str(),
                *parts_limit,
                *parts_offset,
            ))?;
            render_value(&result, cli.json)
        }
        Command::Stat { path } => {
            let path = normalize_drive_path(&context, Some(path))?;
            let result = client.call_drive(&metadata_request(path.as_str()))?;
            render_value(&result, cli.json)
        }
        Command::Biquery { query, paths } => {
            let paths = normalize_many_paths(&context, paths)?;
            let result = client.call_drive(&biquery_request(query, &paths))?;
            render_value(&result, cli.json)
        }
        Command::Database { command } => run_database_command(command, &mut client, cli),
        Command::TelegramDb { command } => run_telegram_db_command(command, &mut client, cli),
        Command::Upload {
            local,
            dest,
            recursive,
            force,
        } => {
            let dest = normalize_drive_path(&context, dest.as_deref())?;
            let files = collect_upload_files(local, *recursive)?;
            let results = upload_files(&mut client, dest.as_str(), &files, *force)?;
            render_value(&Value::Array(results), cli.json)
        }
        Command::Mkdir { path } => {
            let path = normalize_drive_path(&context, Some(path))?;
            let (parent, name) = split_drive_parent_name(path.as_str())?;
            let result =
                client.call_drive(&path_create_request(&name, parent.as_deref(), "dir"))?;
            render_value(&result, cli.json)
        }
        Command::Rm { path, recursive } => {
            let path = normalize_drive_path(&context, Some(path))?;
            let paths = vec![path.as_str().to_owned()];
            let result = client.call_drive(&path_delete_request(&paths, flag_opt(*recursive)))?;
            render_value(&result, cli.json)
        }
        Command::Mv { src, dst } => {
            let src = normalize_drive_path(&context, Some(src))?;
            let dst = normalize_drive_path(&context, Some(dst))?;
            let (new_parent, new_name) = split_drive_parent_name(dst.as_str())?;
            let result = client.call_drive(&path_rename_request(
                src.as_str(),
                &new_name,
                new_parent.as_deref(),
            ))?;
            render_value(&result, cli.json)
        }
        Command::Cp { src, dst, force } => {
            let src = normalize_drive_path(&context, Some(src))?;
            let dst = normalize_drive_path(&context, Some(dst))?;
            let result = copy_drive_file(&mut client, src.as_str(), dst.as_str(), *force)?;
            render_value(&result, cli.json)
        }
        Command::Sync {
            folder,
            dest,
            once,
            interval_seconds,
        } => sync_folder(
            &mut client,
            &context,
            folder,
            dest.as_deref(),
            *once,
            *interval_seconds,
            cli,
        ),
        _ => bail!("command is not a Drive command"),
    }
}

/// Dispatches generic Drive DB commands to the source-backed JSON-RPC methods.
fn run_database_command(
    command: &DatabaseCommand,
    client: &mut GatewayClient,
    cli: &Cli,
) -> Result<()> {
    let request = match command {
        DatabaseCommand::Create {
            name,
            schema_json,
            schema_file,
            check_exists,
            recreate,
            directory,
            db_type,
        } => {
            let schema = parse_optional_json_arg("schema", schema_json.as_deref(), schema_file)?
                .map(|value| expect_json_object("schema", value))
                .transpose()?;
            database_create_request(
                name,
                schema,
                flag_opt(*check_exists),
                flag_opt(*recreate),
                directory.as_deref(),
                db_type.as_deref(),
            )
        }
        DatabaseCommand::Insert {
            name,
            table,
            rows_json,
            rows_file,
            db_type,
        } => {
            let rows = expect_json_array(
                "rows",
                parse_required_json_arg("rows", rows_json.as_deref(), rows_file)?,
            )?;
            database_insert_request(name, table, rows, db_type.as_deref())
        }
        DatabaseCommand::Query {
            name,
            sql,
            parameters_json,
            parameters_file,
            db_type,
        } => {
            let parameters =
                parse_optional_json_arg("parameters", parameters_json.as_deref(), parameters_file)?
                    .map(|value| expect_json_array("parameters", value))
                    .transpose()?;
            database_query_request(name, sql, parameters, db_type.as_deref())
        }
        DatabaseCommand::Commit { name, db_type } => {
            database_commit_request(name, db_type.as_deref())
        }
        DatabaseCommand::Rollback { name, db_type } => {
            database_rollback_request(name, db_type.as_deref())
        }
        DatabaseCommand::Metadata { name, db_type } => {
            database_metadata_request(name, db_type.as_deref())
        }
        DatabaseCommand::Drop { name, db_type } => database_drop_request(name, db_type.as_deref()),
        DatabaseCommand::SetStatus {
            name,
            status,
            error,
            db_type,
        } => database_set_status_request(name, status, error.as_deref(), db_type.as_deref()),
        DatabaseCommand::ResolveByInode { db_inode, db_type } => {
            database_resolve_by_inode_request(db_inode, db_type.as_deref())
        }
        DatabaseCommand::ResolveWithSettings { db_inode, db_type } => {
            database_resolve_with_settings_request(db_inode, db_type.as_deref())
        }
    };
    let result = client.call_drive(&request)?;
    render_value(&result, cli.json)
}

/// Dispatches Telegram Drive DB commands to the source-backed JSON-RPC methods.
fn run_telegram_db_command(
    command: &TelegramDbCommand,
    client: &mut GatewayClient,
    cli: &Cli,
) -> Result<()> {
    let request = match command {
        TelegramDbCommand::Create {
            name,
            schema_json,
            schema_file,
            recreate,
            directory,
        } => {
            let schema = parse_optional_json_arg("schema", schema_json.as_deref(), schema_file)?
                .map(|value| expect_json_object("schema", value))
                .transpose()?;
            telegram_db_create_request(
                name,
                schema,
                None,
                flag_opt(*recreate),
                directory.as_deref(),
            )
        }
        TelegramDbCommand::Insert {
            name,
            table,
            rows_json,
            rows_file,
        } => {
            let rows = expect_json_array(
                "rows",
                parse_required_json_arg("rows", rows_json.as_deref(), rows_file)?,
            )?;
            telegram_db_insert_request(name, table, rows)
        }
        TelegramDbCommand::Query {
            name,
            sql,
            parameters_json,
            parameters_file,
        } => {
            let parameters =
                parse_optional_json_arg("parameters", parameters_json.as_deref(), parameters_file)?
                    .map(|value| expect_json_array("parameters", value))
                    .transpose()?;
            telegram_db_query_request(name, sql, parameters)
        }
        TelegramDbCommand::Commit { name } => telegram_db_commit_request(name),
        TelegramDbCommand::Metadata { name } => telegram_db_metadata_request(name),
        TelegramDbCommand::Drop { name } => telegram_db_drop_request(name),
    };
    let result = client.call_drive(&request)?;
    render_value(&result, cli.json)
}

/// Logs in with a raw token, keyfile, or browser-created diskd CLI credentials.
fn login(
    state: &mut RuntimeState,
    token: Option<&str>,
    credentials_file: Option<&Path>,
    dev: bool,
    app_url: Option<&str>,
    quiet: bool,
) -> Result<()> {
    ensure_private_home(&state.home_dir)?;
    let access_token = match (token, credentials_file) {
        (Some(token), None) => token.trim().to_owned(),
        (None, Some(path)) => {
            let document = fs::read_to_string(path).with_context(|| {
                format!("failed to read credentials fixture {}", path.display())
            })?;
            let fixture = parse_client_credentials_file(&document)?;
            request_and_store_client_credentials_token(state, fixture, quiet)?
        }
        (Some(_), Some(_)) => bail!("use either --token or --credentials-file, not both"),
        (None, None) => browser_login(state, dev, app_url, quiet)?,
    };
    store_access_token(state, access_token, quiet)
}

/// Exchanges client credentials for a bearer token and stores the gateway URL.
fn request_and_store_client_credentials_token(
    state: &mut RuntimeState,
    fixture: ClientCredentialsFile,
    quiet: bool,
) -> Result<String> {
    state.config.base_url = Some(fixture.apis_url.clone());
    let params = ClientCredentialsTokenParams {
        issuer: fixture.issuer,
        client_id: fixture.client_id,
        client_secret: fixture.client_secret,
        audience: fixture.audience,
        scopes: TOKEN_SCOPES
            .iter()
            .map(|scope| (*scope).to_owned())
            .collect(),
    };
    let token = match request_client_credentials_token(&params) {
        Ok(token) => token,
        Err(error) if error.to_string().contains("invalid_scope") => {
            if !quiet {
                eprintln!(
                    "requested gateway scopes were rejected by issuer; retrying with client defaults"
                );
            }
            request_client_credentials_token(&ClientCredentialsTokenParams {
                scopes: Vec::new(),
                ..params
            })?
        }
        Err(error) => return Err(error.into()),
    };
    save_config(state)?;
    Ok(token)
}

/// Persists a bearer token in the private credentials file.
fn store_access_token(state: &RuntimeState, access_token: String, quiet: bool) -> Result<()> {
    if access_token.is_empty() {
        bail!("login token must not be empty");
    }
    let credentials = StoredCredentials {
        access_token,
        token_type: Some("Bearer".to_owned()),
        ..StoredCredentials::default()
    };
    write_secret_file(
        &state.credentials_path,
        format_stored_credentials(&credentials)?.as_bytes(),
    )?;
    if !quiet {
        eprintln!("stored credentials in {}", state.credentials_path.display());
    }
    Ok(())
}

/// Runs browser login by opening the app and waiting for credentials on localhost.
fn browser_login(
    state: &mut RuntimeState,
    dev: bool,
    app_url: Option<&str>,
    quiet: bool,
) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("failed to bind login callback")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure login callback")?;
    let callback_url = format!("http://{}/callback", listener.local_addr()?);
    let login_state = make_browser_login_state()?;
    let login_url = browser_login_url(
        resolve_login_app_url(dev, app_url)?,
        &callback_url,
        &login_state,
    );

    if !quiet {
        eprintln!("opening {login_url}");
    }
    if let Err(error) = open_browser(&login_url) {
        eprintln!("failed to open browser automatically: {error:#}");
        eprintln!("open this URL to continue diskd login:\n{login_url}");
    }

    let fixture = wait_for_browser_credentials(&listener, &login_state, BROWSER_LOGIN_TIMEOUT)?;
    request_and_store_client_credentials_token(state, fixture, quiet)
}

/// Resolves the app URL used by browser login.
fn resolve_login_app_url(dev: bool, app_url: Option<&str>) -> Result<&str> {
    if dev && app_url.is_some() {
        bail!("use either --dev or --app-url, not both");
    }
    if let Some(url) = app_url {
        if url.trim().is_empty() {
            bail!("--app-url must not be empty");
        }
        return Ok(url.trim());
    }
    Ok(if dev {
        DEV_LOGIN_APP_URL
    } else {
        DEFAULT_LOGIN_APP_URL
    })
}

/// Builds the OAuth Apps deep link consumed by the web app.
fn browser_login_url(app_url: &str, callback_url: &str, state: &str) -> String {
    let separator = if app_url.contains('?') { '&' } else { '?' };
    format!(
        "{app_url}{separator}source=diskd-cli&callback={}&state={}",
        percent_encode(callback_url),
        percent_encode(state)
    )
}

/// Creates a per-run callback state for the local browser login handshake.
fn make_browser_login_state() -> Result<String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX epoch")?
        .as_nanos();
    Ok(format!("{}-{nanos}", std::process::id()))
}

/// Opens the browser using the current platform's standard launcher.
fn open_browser(url: &str) -> Result<()> {
    if let Ok(command) = env::var("DISKD_BROWSER") {
        if !command.trim().is_empty() {
            return run_browser_command(command.trim(), &[url]);
        }
    }

    open_default_browser(url)
}

/// Opens the URL with the native browser launcher for the current platform.
#[cfg(target_os = "macos")]
fn open_default_browser(url: &str) -> Result<()> {
    run_browser_command("open", &[url])
}

/// Opens the URL with the native browser launcher for the current platform.
#[cfg(target_os = "windows")]
fn open_default_browser(url: &str) -> Result<()> {
    run_browser_command("cmd", &["/C", "start", "", url])
}

/// Opens the URL with the native browser launcher for the current platform.
#[cfg(all(unix, not(target_os = "macos")))]
fn open_default_browser(url: &str) -> Result<()> {
    run_browser_command("xdg-open", &[url])
}

/// Returns a visible error on platforms without a known browser launcher.
#[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
fn open_default_browser(_url: &str) -> Result<()> {
    bail!("opening a browser is not supported on this platform")
}

/// Runs a browser launcher and converts non-zero exits into visible errors.
fn run_browser_command(command: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(command)
        .args(args)
        .status()
        .with_context(|| format!("failed to run {command}"))?;
    if !status.success() {
        bail!("{command} exited with {status}");
    }
    Ok(())
}

/// Waits for one browser callback and returns the posted credentials.
fn wait_for_browser_credentials(
    listener: &TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<ClientCredentialsFile> {
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let result = read_browser_credentials_request(&mut stream, expected_state);
                match &result {
                    Ok(_) => write_login_callback_response(
                        &mut stream,
                        "200 OK",
                        "diskd login complete. You can close this window.",
                    )?,
                    Err(error) => write_login_callback_response(
                        &mut stream,
                        "400 Bad Request",
                        &format!("diskd login failed: {error:#}"),
                    )?,
                }
                return result;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if started.elapsed() >= timeout {
                    bail!("timed out waiting for browser login callback");
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error).context("failed to accept login callback"),
        }
    }
}

/// Reads and validates the form POST sent by the OAuth Apps page.
fn read_browser_credentials_request(
    stream: &mut TcpStream,
    expected_state: &str,
) -> Result<ClientCredentialsFile> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("failed to set login callback read timeout")?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if !request_line.starts_with("POST ") {
        bail!("expected POST callback");
    }

    let mut content_length = 0_usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>()?;
        } else if let Some(value) = trimmed.strip_prefix("content-length:") {
            content_length = value.trim().parse::<usize>()?;
        }
    }
    if content_length == 0 {
        bail!("callback body is empty");
    }
    let mut body = vec![0_u8; content_length];
    reader.read_exact(&mut body)?;
    parse_browser_credentials_form(&String::from_utf8(body)?, expected_state)
}

/// Parses the x-www-form-urlencoded credentials callback body.
fn parse_browser_credentials_form(
    body: &str,
    expected_state: &str,
) -> Result<ClientCredentialsFile> {
    let mut state = None;
    let mut credentials = None;
    for pair in body.split('&') {
        let Some((raw_key, raw_value)) = pair.split_once('=') else {
            continue;
        };
        let key = percent_decode_form(raw_key)?;
        let value = percent_decode_form(raw_value)?;
        match key.as_str() {
            "state" => state = Some(value),
            "credentials" => credentials = Some(value),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        bail!("callback state mismatch");
    }
    let Some(document) = credentials else {
        bail!("callback is missing credentials");
    };
    Ok(parse_client_credentials_file(&document)?)
}

/// Writes a minimal HTML response to the browser callback request.
fn write_login_callback_response(
    stream: &mut TcpStream,
    status: &str,
    message: &str,
) -> Result<()> {
    let escaped = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let body = format!("<!doctype html><title>diskd login</title><p>{escaped}</p>");
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()?;
    Ok(())
}

/// Percent-encodes URL query values without adding a dependency.
fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Decodes form-urlencoded field values from the local browser callback.
fn percent_decode_form(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let high = decode_hex(bytes[index + 1])?;
                let low = decode_hex(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            b'%' => bail!("incomplete percent escape"),
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    Ok(String::from_utf8(decoded)?)
}

/// Converts one ASCII hex byte into its numeric value.
fn decode_hex(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("invalid percent escape"),
    }
}

/// Removes the stored bearer token without touching non-secret config.
fn logout(state: &RuntimeState, quiet: bool) -> Result<()> {
    match fs::remove_file(&state.credentials_path) {
        Ok(()) => {
            if !quiet {
                eprintln!("removed {}", state.credentials_path.display());
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("failed to remove credentials"),
    }
}

/// Prints identity metadata derived from the current bearer token.
fn whoami(cli: &Cli, state: &RuntimeState) -> Result<()> {
    let token = effective_token(state)?;
    let identity = decode_jwt_identity(&token)?;
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "workspace_id": identity.workspace_id,
                "subject": identity.subject,
                "user_id": identity.user_id,
                "client_id": identity.client_id,
                "scopes": identity.scopes,
            }))?
        );
    } else {
        println!("workspace_id: {}", identity.workspace_id);
        if let Some(subject) = identity.subject {
            println!("subject: {subject}");
        }
        if let Some(user_id) = identity.user_id {
            println!("user_id: {user_id}");
        }
        if let Some(client_id) = identity.client_id {
            println!("client_id: {client_id}");
        }
        if !identity.scopes.is_empty() {
            println!("scopes: {}", identity.scopes.join(" "));
        }
    }
    Ok(())
}

/// Sets, clears, or lists the current project context.
fn set_context(
    cli: &Cli,
    state: &mut RuntimeState,
    project: Option<&str>,
    list: bool,
    root: bool,
) -> Result<()> {
    if root {
        state.config.project = None;
        state.config.project_name = None;
        save_config(state)?;
        return get_context(cli, state);
    }

    let base_url = effective_base_url(cli, state);
    let token = effective_token(state)?;
    let client = GatewayClient::new(&base_url, &token)?;
    let projects = client.list_projects()?;

    if list || project.is_none() {
        let value = Value::Array(
            projects
                .iter()
                .map(|project| json!({ "id": project.id, "name": project.name }))
                .collect(),
        );
        return render_value(&value, cli.json);
    }

    let requested = project.unwrap_or_default();
    let Some(selected) = projects
        .iter()
        .find(|candidate| candidate.id == requested || candidate.name == requested)
    else {
        bail!("project '{requested}' was not found");
    };
    state.config.project = Some(selected.id.clone());
    state.config.project_name = Some(selected.name.clone());
    save_config(state)?;
    get_context(cli, state)
}

/// Prints the current project context without contacting the Drive.
fn get_context(cli: &Cli, state: &RuntimeState) -> Result<()> {
    let value = match (&state.config.project, &state.config.project_name) {
        (Some(id), Some(name)) => json!({ "scope": "project", "id": id, "name": name }),
        (Some(id), None) => json!({ "scope": "project", "id": id }),
        _ => json!({ "scope": "workspace_root", "id": "system_project_id" }),
    };
    render_value(&value, cli.json)
}

/// Prints the compiled CLI version.
fn print_version(cli: &Cli) -> Result<()> {
    let value = json!({
        "name": "diskd",
        "version": env!("CARGO_PKG_VERSION"),
        "repository": "https://github.com/diskd-ai/diskd-cli",
    });
    render_value(&value, cli.json)
}

/// Updates the running diskd binary from the latest GitHub release.
fn update_cli(cli: &Cli, force: bool) -> Result<()> {
    let http = build_update_http_client(UPDATE_DOWNLOAD_TIMEOUT)?;
    let release = fetch_latest_release(&http)?;
    let target = current_release_target()?;
    let current_version = env!("CARGO_PKG_VERSION");
    let Some(update) = available_update_from_release(&release, current_version, target, force)?
    else {
        let latest_version = normalize_release_version(&release.tag_name);
        if cli.json {
            render_value(
                &json!({
                    "updated": false,
                    "current_version": current_version,
                    "latest_version": latest_version,
                }),
                true,
            )
        } else {
            println!("diskd is up to date ({current_version})");
            Ok(())
        }?;
        return Ok(());
    };

    let archive_bytes = download_release_bytes(&http, &update.assets.archive_url)
        .context("failed to download update archive")?;
    let checksum_document = download_release_bytes(&http, &update.assets.checksum_url)
        .context("failed to download update checksum")?;
    verify_archive_checksum(&archive_bytes, &checksum_document)?;
    install_update_archive(&archive_bytes)?;

    if cli.json {
        render_value(
            &json!({
                "updated": true,
                "previous_version": update.current_version,
                "current_version": update.latest_version,
                "release_url": update.release_url,
            }),
            true,
        )
    } else {
        println!(
            "updated diskd from {} to {}",
            update.current_version, update.latest_version
        );
        Ok(())
    }
}

/// Prints a best-effort yellow notice when a newer release exists.
fn maybe_show_update_notice(cli: &Cli) {
    if !should_check_for_updates(cli) {
        return;
    }
    match check_for_available_update(UPDATE_CHECK_TIMEOUT) {
        Ok(Some(update)) => eprintln!(
            "\x1b[33mdiskd {} is available; current is {}. Run `diskd update`.\x1b[0m",
            update.latest_version, update.current_version
        ),
        Ok(None) => {}
        Err(error) => eprintln!("diskd: update check failed: {error:#}"),
    }
}

/// Decides whether this invocation may emit human update text to stderr.
fn should_check_for_updates(cli: &Cli) -> bool {
    if cli.quiet || cli.json || env::var_os(UPDATE_CHECK_DISABLE_ENV).is_some() {
        return false;
    }
    !matches!(
        &cli.command,
        Command::Update { .. }
            | Command::Mcp {
                command: McpCommand::Serve
            }
    )
}

/// Looks up the latest release using a short timeout for command-start checks.
fn check_for_available_update(timeout: Duration) -> Result<Option<AvailableUpdate>> {
    let http = build_update_http_client(timeout)?;
    let release = fetch_latest_release(&http)?;
    let target = current_release_target()?;
    available_update_from_release(&release, env!("CARGO_PKG_VERSION"), target, false)
}

/// Builds an HTTP client for GitHub release metadata and asset downloads.
fn build_update_http_client(timeout: Duration) -> Result<HttpClient> {
    HttpClient::builder()
        .timeout(timeout)
        .build()
        .context("failed to build update HTTP client")
}

/// Fetches the latest public GitHub release metadata for diskd-cli.
fn fetch_latest_release(http: &HttpClient) -> Result<GitHubRelease> {
    let response = http
        .get(latest_release_url())
        .header(USER_AGENT, update_user_agent())
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .context("failed to request latest diskd release")?
        .error_for_status()
        .context("latest diskd release request failed")?;
    response
        .json::<GitHubRelease>()
        .context("failed to decode latest diskd release")
}

/// Builds the GitHub API URL for the latest diskd-cli release.
fn latest_release_url() -> String {
    format!("{GITHUB_API_BASE_URL}/repos/{GITHUB_REPOSITORY}/releases/latest")
}

/// Returns the update user agent required by GitHub API requests.
fn update_user_agent() -> String {
    format!("diskd/{}", env!("CARGO_PKG_VERSION"))
}

/// Resolves whether a GitHub release can update this binary on this platform.
fn available_update_from_release(
    release: &GitHubRelease,
    current_version: &str,
    target: &str,
    force: bool,
) -> Result<Option<AvailableUpdate>> {
    if !force && !is_newer_release(&release.tag_name, current_version) {
        return Ok(None);
    }
    let assets = select_release_asset_pair(release, target)?;
    Ok(Some(AvailableUpdate {
        current_version: current_version.to_owned(),
        latest_version: normalize_release_version(&release.tag_name),
        release_url: release.html_url.clone(),
        assets,
    }))
}

/// Selects the platform archive and checksum assets from a release.
fn select_release_asset_pair(release: &GitHubRelease, target: &str) -> Result<ReleaseAssetPair> {
    let archive_name = release_archive_name(&release.tag_name, target);
    let checksum_name = release_checksum_name(&release.tag_name, target);
    let archive = release
        .assets
        .iter()
        .find(|asset| asset.name == archive_name)
        .with_context(|| format!("release asset is missing: {archive_name}"))?;
    let checksum = release
        .assets
        .iter()
        .find(|asset| asset.name == checksum_name)
        .with_context(|| format!("release asset is missing: {checksum_name}"))?;
    Ok(ReleaseAssetPair {
        archive_url: archive.browser_download_url.clone(),
        checksum_url: checksum.browser_download_url.clone(),
    })
}

/// Builds the platform archive name produced by the release workflow.
fn release_archive_name(tag_name: &str, target: &str) -> String {
    format!("diskd-{}-{target}.tar.gz", normalize_release_tag(tag_name))
}

/// Builds the checksum asset name produced by the release workflow.
fn release_checksum_name(tag_name: &str, target: &str) -> String {
    format!("{}.sha256", release_archive_name(tag_name, target))
}

/// Normalizes release tags to the public v-prefixed form used in asset names.
fn normalize_release_tag(tag_name: &str) -> String {
    let trimmed = tag_name.trim();
    if trimmed.starts_with('v') {
        trimmed.to_owned()
    } else {
        format!("v{trimmed}")
    }
}

/// Normalizes release tags for user-facing version comparisons.
fn normalize_release_version(tag_name: &str) -> String {
    tag_name.trim().trim_start_matches('v').to_owned()
}

/// Compares a latest release tag against the compiled package version.
fn is_newer_release(latest_tag: &str, current_version: &str) -> bool {
    matches!(
        compare_release_versions(latest_tag, current_version),
        Some(Ordering::Greater)
    )
}

/// Compares dotted numeric release versions while ignoring a leading v.
fn compare_release_versions(left: &str, right: &str) -> Option<Ordering> {
    let left_parts = parse_release_version(left)?;
    let right_parts = parse_release_version(right)?;
    let length = left_parts.len().max(right_parts.len());
    for index in 0..length {
        let left_value = left_parts.get(index).copied().unwrap_or(0);
        let right_value = right_parts.get(index).copied().unwrap_or(0);
        match left_value.cmp(&right_value) {
            Ordering::Equal => {}
            ordering => return Some(ordering),
        }
    }
    Some(Ordering::Equal)
}

/// Parses simple semver-like numeric release versions used by diskd tags.
fn parse_release_version(value: &str) -> Option<Vec<u64>> {
    let normalized = value
        .trim()
        .trim_start_matches('v')
        .split_once('-')
        .map_or_else(
            || value.trim().trim_start_matches('v'),
            |(version, _)| version,
        );
    if normalized.is_empty() {
        return None;
    }
    normalized
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                None
            } else {
                part.parse::<u64>().ok()
            }
        })
        .collect()
}

/// Resolves the release target triple for the current operating system.
fn current_release_target() -> Result<&'static str> {
    release_target_for(env::consts::OS, env::consts::ARCH).with_context(|| {
        format!(
            "diskd update is not available for {}-{}",
            env::consts::ARCH,
            env::consts::OS
        )
    })
}

/// Maps Rust runtime OS and architecture labels to release workflow targets.
fn release_target_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-musl"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

/// Downloads one release asset body from GitHub.
fn download_release_bytes(http: &HttpClient, url: &str) -> Result<Vec<u8>> {
    let bytes = http
        .get(url)
        .header(USER_AGENT, update_user_agent())
        .send()
        .with_context(|| format!("failed to request {url}"))?
        .error_for_status()
        .with_context(|| format!("release asset request failed for {url}"))?
        .bytes()
        .with_context(|| format!("failed to read release asset {url}"))?;
    Ok(bytes.to_vec())
}

/// Verifies the archive bytes against the downloaded .sha256 document.
fn verify_archive_checksum(archive_bytes: &[u8], checksum_document: &[u8]) -> Result<()> {
    let checksum_text = std::str::from_utf8(checksum_document)
        .context("update checksum document is not valid UTF-8")?;
    let expected = parse_sha256_checksum(checksum_text)?;
    let actual = sha256_hex(archive_bytes);
    if actual != expected {
        bail!("update checksum mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

/// Parses the first hex digest from a sha256sum-compatible checksum document.
fn parse_sha256_checksum(document: &str) -> Result<String> {
    let checksum = document
        .split_whitespace()
        .next()
        .context("checksum document is empty")?;
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("checksum document does not start with a SHA-256 hex digest");
    }
    Ok(checksum.to_ascii_lowercase())
}

/// Extracts and installs the verified update archive over the running binary.
fn install_update_archive(archive_bytes: &[u8]) -> Result<()> {
    let temp_dir = create_update_temp_dir()?;
    let result = install_update_from_temp_dir(archive_bytes, &temp_dir);
    if let Err(error) = fs::remove_dir_all(&temp_dir) {
        eprintln!(
            "diskd: failed to remove temporary update directory {}: {error}",
            temp_dir.display()
        );
    }
    result
}

/// Performs update extraction and binary replacement using a temporary directory.
fn install_update_from_temp_dir(archive_bytes: &[u8], temp_dir: &Path) -> Result<()> {
    let binary_path = extract_diskd_binary(archive_bytes, temp_dir)?;
    self_replace::self_replace(&binary_path).context("failed to replace current diskd binary")?;
    Ok(())
}

/// Creates a private temporary directory for update extraction.
fn create_update_temp_dir() -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX epoch")?
        .as_nanos();
    for attempt in 0..10 {
        let candidate = env::temp_dir().join(format!(
            "diskd-update-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create {}", candidate.display()));
            }
        }
    }
    bail!("failed to create a unique update temporary directory")
}

/// Extracts the diskd binary from a release tar.gz archive into the temp dir.
fn extract_diskd_binary(archive_bytes: &[u8], temp_dir: &Path) -> Result<PathBuf> {
    let decoder = GzDecoder::new(Cursor::new(archive_bytes));
    let mut archive = Archive::new(decoder);
    let output_path = temp_dir.join(platform_binary_name());
    for entry in archive.entries().context("failed to read update archive")? {
        let mut entry = entry.context("failed to read update archive entry")?;
        let entry_path = entry
            .path()
            .context("failed to read update archive entry path")?
            .into_owned();
        if entry_path.file_name() == Some(OsStr::new("diskd")) {
            entry
                .unpack(&output_path)
                .context("failed to extract diskd binary from update archive")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&output_path, fs::Permissions::from_mode(0o755))
                    .context("failed to set update binary permissions")?;
            }
            return Ok(output_path);
        }
    }
    bail!("update archive does not contain a diskd binary")
}

/// Returns the local executable file name used during update extraction.
fn platform_binary_name() -> &'static str {
    if cfg!(windows) {
        "diskd.exe"
    } else {
        "diskd"
    }
}

/// Detects direct terminal execution of the stdio MCP server command.
fn should_show_mcp_setup_instructions(cli: &Cli) -> bool {
    matches!(
        &cli.command,
        Command::Mcp {
            command: McpCommand::Serve
        }
    ) && io::stdin().is_terminal()
        && io::stdout().is_terminal()
}

/// Prints human setup instructions for connecting diskd to an LLM MCP client.
fn print_mcp_setup_instructions() -> Result<()> {
    let command = env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "diskd".to_owned());
    println!("diskd MCP server");
    println!();
    println!("Add this server to your LLM agent MCP configuration:");
    println!(
        "{}",
        serde_json::to_string_pretty(&mcp_agent_config(&command))?
    );
    println!();
    println!("Authenticate before connecting:");
    println!("  diskd login --token \"$APIS_ACCESS_TOKEN\"");
    println!();
    println!("Or add APIS_ACCESS_TOKEN to the env block in the MCP configuration.");
    println!(
        "The LLM agent must launch this command over stdio; direct terminal runs show this guide."
    );
    Ok(())
}

/// Builds the JSON MCP server config shown to users and tested for stability.
fn mcp_agent_config(command: &str) -> Value {
    json!({
        "mcpServers": {
            "diskd": {
                "command": command,
                "args": ["mcp", "serve"],
                "env": {
                    "APIS_BASE_URL": DEFAULT_BASE_URL
                }
            }
        }
    })
}

/// Runs a one-shot or polling one-way local folder sync to the Drive.
fn sync_folder(
    client: &mut GatewayClient,
    context: &DriveContext,
    folder: &Path,
    dest: Option<&str>,
    once: bool,
    interval_seconds: u64,
    cli: &Cli,
) -> Result<()> {
    if !folder.is_dir() {
        bail!("sync source must be a directory: {}", folder.display());
    }
    let dest = normalize_drive_path(context, dest)?;
    loop {
        let files = collect_upload_files(&[folder.to_path_buf()], true)?;
        let results = upload_files(client, dest.as_str(), &files, true)?;
        if once {
            return render_value(&Value::Array(results), cli.json);
        }
        if !cli.quiet {
            eprintln!("synced {} files", results.len());
        }
        thread::sleep(Duration::from_secs(interval_seconds.max(1)));
    }
}

/// Copies a Drive file by streaming it down and uploading it to a new path.
fn copy_drive_file(
    client: &mut GatewayClient,
    src_path: &str,
    dst_path: &str,
    force: bool,
) -> Result<Value> {
    let download = client.call_drive(&download_url_request(src_path, None))?;
    let url = read_string_field(&download, "url")?;
    let bytes = client.download_bytes(&url)?;
    let (parent, name) = split_drive_parent_name(dst_path)?;
    upload_bytes(
        client,
        parent.as_deref().unwrap_or("/"),
        &name,
        "application/octet-stream",
        bytes,
        force,
    )
}

/// Uploads multiple local files under the requested Drive destination directory.
fn upload_files(
    client: &mut GatewayClient,
    dest_root: &str,
    files: &[UploadFile],
    force: bool,
) -> Result<Vec<Value>> {
    let mut results = Vec::new();
    for file in files {
        let bytes = fs::read(&file.local_path)
            .with_context(|| format!("failed to read {}", file.local_path.display()))?;
        let name = file
            .relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .context("upload source has no file name")?;
        let parent = remote_parent_for(dest_root, &file.relative_path)?;
        ensure_remote_parent_dirs(client, dest_root, &file.relative_path)?;
        let mime = mime_guess::from_path(&file.local_path)
            .first_or_octet_stream()
            .to_string();
        results.push(upload_bytes(client, &parent, name, &mime, bytes, force)?);
    }
    Ok(results)
}

/// Uploads a byte buffer using Drive's start, PUT, and commit contract.
fn upload_bytes(
    client: &mut GatewayClient,
    parent_path: &str,
    name: &str,
    mime_type: &str,
    bytes: Vec<u8>,
    force: bool,
) -> Result<Value> {
    let hash = sha256_hex(&bytes);
    let start = client.call_drive(&upload_start_request(
        name,
        bytes.len() as u64,
        &hash,
        Some(parent_path),
        Some(mime_type),
        Some(force),
    ))?;
    let intent = decode_upload_start(&start)?;
    let etag = client.put_upload(&intent.upload_url, &intent.intent_id, mime_type, bytes)?;
    let commit = client.call_drive(&upload_commit_request(&intent.intent_id, &etag))?;
    Ok(commit)
}

/// Recursively creates parent directories required by nested upload targets.
fn ensure_remote_parent_dirs(
    client: &mut GatewayClient,
    dest_root: &str,
    relative_path: &Path,
) -> Result<()> {
    let mut current = dest_root.to_owned();
    let Some(parent) = relative_path.parent() else {
        return Ok(());
    };
    for segment in parent.components() {
        let name = segment.as_os_str().to_string_lossy();
        if name.is_empty() {
            continue;
        }
        match client.call_drive(&path_create_request(&name, Some(&current), "dir")) {
            Ok(_) => {}
            Err(error) if is_existing_directory_error(&error.to_string()) => {}
            Err(error) => return Err(error.into()),
        }
        current = join_drive_path(&current, &name);
    }
    Ok(())
}

/// Treats idempotent mkdir failures as success for sync/upload parent creation.
fn is_existing_directory_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("exist") || lower.contains("already")
}

/// Collects local files and preserves paths relative to each requested root.
fn collect_upload_files(local: &[PathBuf], recursive: bool) -> Result<Vec<UploadFile>> {
    if local.is_empty() {
        bail!("upload requires at least one local file");
    }
    let mut files = Vec::new();
    for path in local {
        if path.is_file() {
            let file_name = path
                .file_name()
                .context("local file has no file name")?
                .to_owned();
            files.push(UploadFile {
                local_path: path.clone(),
                relative_path: PathBuf::from(file_name),
            });
        } else if path.is_dir() {
            if !recursive {
                bail!("{} is a directory; use --recursive", path.display());
            }
            collect_directory_files(path, path, &mut files)?;
        } else {
            bail!("local path does not exist: {}", path.display());
        }
    }
    Ok(files)
}

/// Walks a directory tree using std::fs so sync has no extra runtime dependency.
fn collect_directory_files(root: &Path, dir: &Path, files: &mut Vec<UploadFile>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_directory_files(root, &path, files)?;
        } else if path.is_file() {
            let relative_path = path.strip_prefix(root)?.to_path_buf();
            files.push(UploadFile {
                local_path: path,
                relative_path,
            });
        }
    }
    Ok(())
}

/// Computes the hex SHA-256 digest required by drive/upload/start.
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Starts an embedded MCP server over stdio using the same Drive client.
fn run_mcp_serve(cli: &Cli, state: &RuntimeState) -> Result<()> {
    let base_url = effective_base_url(cli, state);
    let token = effective_token(state)?;
    let context = effective_drive_context(cli, state)?;
    let mut client = GatewayClient::new(&base_url, &token)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Some(message) = read_mcp_message(&mut reader)? {
        if let Some(response) = handle_mcp_message(&mut client, &context, &message)? {
            write_mcp_message(&mut writer, &response)?;
        }
    }
    Ok(())
}

/// Reads one MCP JSON-RPC message from stdio, accepting framed or line-delimited input.
fn read_mcp_message(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut first_line = String::new();
    if reader.read_line(&mut first_line)? == 0 {
        return Ok(None);
    }
    let first = first_line.trim_end_matches(['\r', '\n']);
    if first.is_empty() {
        return read_mcp_message(reader);
    }
    if let Some(length_text) = first.strip_prefix("Content-Length:") {
        let length = length_text.trim().parse::<usize>()?;
        let mut blank = String::new();
        loop {
            blank.clear();
            reader.read_line(&mut blank)?;
            if blank.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        return Ok(Some(serde_json::from_slice(&body)?));
    }
    Ok(Some(serde_json::from_str(first)?))
}

/// Writes one MCP JSON-RPC response using Content-Length framing.
fn write_mcp_message(writer: &mut impl Write, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

/// Handles a single MCP JSON-RPC request or notification.
fn handle_mcp_message(
    client: &mut GatewayClient,
    context: &DriveContext,
    message: &Value,
) -> Result<Option<Value>> {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(id_value) = id else {
        return Ok(None);
    };
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "diskd", "version": env!("CARGO_PKG_VERSION") }
        }),
        "tools/list" => json!({ "tools": mcp_tools() }),
        "tools/call" => {
            let params = message.get("params").unwrap_or(&Value::Null);
            handle_mcp_tool_call(client, context, params)?
        }
        _ => {
            return Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id_value,
                "error": { "code": -32601, "message": format!("unknown method: {method}") }
            })));
        }
    };
    Ok(Some(
        json!({ "jsonrpc": "2.0", "id": id_value, "result": result }),
    ))
}

/// Invokes one MCP tool call by translating it to the Drive JSON-RPC method.
fn handle_mcp_tool_call(
    client: &mut GatewayClient,
    context: &DriveContext,
    params: &Value,
) -> Result<Value> {
    let name = read_string_field(params, "name")?;
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let request = mcp_tool_request(context, &name, &arguments)?;
    let result = client.call_drive(&request)?;
    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&result)? }],
        "isError": false
    }))
}

/// Builds a Drive request for a known MCP tool name and argument object.
fn mcp_tool_request(
    context: &DriveContext,
    name: &str,
    arguments: &Value,
) -> Result<JsonRpcRequest> {
    match name {
        "tools__ls" => {
            let path = read_optional_string(arguments, "path");
            let path = normalize_drive_path(context, path.as_deref())?;
            Ok(ls_request(
                Some(path.as_str()),
                read_optional_bool(arguments, "recursive"),
                None,
                None,
            ))
        }
        "tools__read" => {
            let path = normalize_drive_path(context, Some(&read_string_field(arguments, "path")?))?;
            let limit = read_optional_u64(arguments, "parts_limit")
                .or_else(|| read_optional_u64(arguments, "limit"));
            let offset = read_optional_u64(arguments, "parts_offset")
                .or_else(|| read_optional_u64(arguments, "offset"));
            Ok(read_file_request(path.as_str(), limit, offset))
        }
        "tools__glob" => {
            let pattern = read_string_field(arguments, "pattern")?;
            let path = read_optional_string(arguments, "path");
            let path = normalize_drive_path(context, path.as_deref())?;
            Ok(glob_request(&pattern, Some(path.as_str()), None, None))
        }
        "tools__grep" => {
            let query = read_string_field(arguments, "query")?;
            let paths = read_string_array(arguments, "paths")?;
            Ok(grep_request(
                &query,
                &normalize_many_paths(context, &paths)?,
                read_optional_u64(arguments, "limit"),
                read_optional_u64(arguments, "offset"),
            ))
        }
        "tools__vsearch" => {
            let query = read_string_field(arguments, "query")?;
            let paths = read_string_array(arguments, "paths")?;
            let limit = read_optional_u64(arguments, "limit")
                .or_else(|| read_optional_u64(arguments, "top"))
                .or_else(|| read_optional_u64(arguments, "top_k"));
            Ok(vsearch_request(
                &query,
                &normalize_many_paths(context, &paths)?,
                limit,
                read_optional_u64(arguments, "offset"),
            ))
        }
        "tools__bi_query" => {
            let query = read_string_field(arguments, "query")?;
            let paths = read_string_array(arguments, "paths")?;
            Ok(biquery_request(
                &query,
                &normalize_many_paths(context, &paths)?,
            ))
        }
        _ => bail!("unknown MCP tool: {name}"),
    }
}

/// Returns MCP tool definitions aligned to the existing Drive MCP server names.
fn mcp_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "tools__ls",
            "description": "List directory contents with full path information.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "recursive": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "tools__read",
            "description": "Read file content as structured parts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "parts_limit": { "type": "integer" },
                    "parts_offset": { "type": "integer" },
                    "limit": { "type": "integer" },
                    "offset": { "type": "integer" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "tools__glob",
            "description": "Find files matching a glob pattern.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" }
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "tools__grep",
            "description": "Full-text search in indexed files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": "integer" },
                    "offset": { "type": "integer" }
                },
                "required": ["query", "paths"]
            }
        }),
        json!({
            "name": "tools__vsearch",
            "description": "Semantic/vector search using embeddings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": "integer" },
                    "offset": { "type": "integer" }
                },
                "required": ["query", "paths"]
            }
        }),
        json!({
            "name": "tools__bi_query",
            "description": "Run BI queries on indexed spreadsheet and CSV files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "paths": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["query", "paths"]
            }
        }),
    ]
}

/// Loads config and standard diskd paths from the environment and filesystem.
fn load_runtime_state(config_override: Option<&Path>) -> Result<RuntimeState> {
    let home_dir = default_diskd_home()?;
    let config_path = config_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home_dir.join("config.yaml"));
    let credentials_path = home_dir.join("credentials");
    let config = if config_path.exists() {
        let document = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        parse_config_document(&document)?
    } else {
        DiskdConfig::default()
    };
    Ok(RuntimeState {
        home_dir,
        config_path,
        credentials_path,
        config,
    })
}

/// Resolves the diskd home directory from DISKD_HOME or the user's home.
fn default_diskd_home() -> Result<PathBuf> {
    if let Some(value) = env::var_os("DISKD_HOME") {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = env::var_os("HOME") {
        return Ok(PathBuf::from(value).join(".diskd"));
    }
    if let Some(value) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(value).join(".diskd"));
    }
    bail!("DISKD_HOME is not set and no home directory is available")
}

/// Persists non-secret config with private parent-directory permissions.
fn save_config(state: &RuntimeState) -> Result<()> {
    ensure_private_home(&state.home_dir)?;
    fs::write(&state.config_path, format_config_document(&state.config))
        .with_context(|| format!("failed to write {}", state.config_path.display()))?;
    Ok(())
}

/// Creates the diskd home with owner-only permissions on Unix platforms.
fn ensure_private_home(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Writes a secret file with 0600 permissions on Unix platforms.
fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_home(parent)?;
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Resolves the API base URL using flag, environment, config, then default precedence.
fn effective_base_url(cli: &Cli, state: &RuntimeState) -> String {
    resolve_setting(
        cli.base_url.as_deref(),
        env::var("APIS_BASE_URL").ok().as_deref(),
        state.config.base_url.as_deref(),
    )
    .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
}

/// Resolves the bearer token from environment or the stored credentials file.
fn effective_token(state: &RuntimeState) -> Result<String> {
    if let Ok(token) = env::var("APIS_ACCESS_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token);
        }
    }
    let document = fs::read_to_string(&state.credentials_path).with_context(|| {
        format!(
            "no bearer token found; set APIS_ACCESS_TOKEN or run diskd login (looked for {})",
            state.credentials_path.display()
        )
    })?;
    let credentials = parse_stored_credentials(&document)?;
    if credentials.access_token.trim().is_empty() {
        bail!("stored credentials contain an empty access_token");
    }
    Ok(credentials.access_token)
}

/// Resolves the current project context for path normalization.
fn effective_drive_context(cli: &Cli, state: &RuntimeState) -> Result<DriveContext> {
    let project = resolve_setting(
        cli.project.as_deref(),
        None,
        state.config.project.as_deref(),
    );
    match project {
        Some(project) => Ok(DriveContext::Project(ProjectId::new(project)?)),
        None => Ok(DriveContext::WorkspaceRoot),
    }
}

/// Normalizes zero or more paths, defaulting to the current context root.
fn normalize_many_paths(context: &DriveContext, paths: &[String]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Ok(vec![normalize_drive_path(context, None)?
            .as_str()
            .to_owned()]);
    }
    paths
        .iter()
        .map(|path| {
            normalize_drive_path(context, Some(path.as_str())).map(|path| path.as_str().to_owned())
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Converts a boolean flag into the optional field convention used by request builders.
fn flag_opt(value: bool) -> Option<bool> {
    if value {
        Some(true)
    } else {
        None
    }
}

/// Fails when a parsed flag has no matching Drive contract field.
fn reject_unsupported_flag(enabled: bool, name: &str) -> Result<()> {
    if enabled {
        bail!("{name} is not supported by the current Drive grep contract")
    }
    Ok(())
}

/// Parses an optional JSON value supplied inline or through a file flag.
fn parse_optional_json_arg(
    label: &'static str,
    inline: Option<&str>,
    file: &Option<PathBuf>,
) -> Result<Option<Value>> {
    if inline.is_some() && file.is_some() {
        bail!("use either --{label} or --{label}-file, not both");
    }
    if let Some(document) = inline {
        return Ok(Some(parse_json_document(label, document)?));
    }
    if let Some(path) = file {
        let document = fs::read_to_string(path)
            .with_context(|| format!("failed to read {label} JSON from {}", path.display()))?;
        return Ok(Some(parse_json_document(label, &document)?));
    }
    Ok(None)
}

/// Parses a required JSON value supplied inline or through a file flag.
fn parse_required_json_arg(
    label: &'static str,
    inline: Option<&str>,
    file: &Option<PathBuf>,
) -> Result<Value> {
    parse_optional_json_arg(label, inline, file)?
        .with_context(|| format!("telegram-db {label} requires --{label} or --{label}-file"))
}

/// Converts a raw JSON document into a serde value with contextual errors.
fn parse_json_document(label: &'static str, document: &str) -> Result<Value> {
    if document.trim().is_empty() {
        bail!("{label} JSON must not be empty");
    }
    serde_json::from_str(document).with_context(|| format!("{label} must be valid JSON"))
}

/// Validates that a JSON command argument is an object before sending it.
fn expect_json_object(label: &'static str, value: Value) -> Result<Value> {
    if value.is_object() {
        Ok(value)
    } else {
        bail!("{label} must be a JSON object")
    }
}

/// Validates that a JSON command argument is an array before sending it.
fn expect_json_array(label: &'static str, value: Value) -> Result<Value> {
    if value.is_array() {
        Ok(value)
    } else {
        bail!("{label} must be a JSON array")
    }
}

/// Renders a Drive value as JSON or compact text.
fn render_value(value: &Value, json_mode: bool) -> Result<()> {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else if let Some(text) = value.as_str() {
        println!("{text}");
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

/// Renders ls results with a stable text mode for humans and JSON for scripts.
fn render_ls(value: &Value, json_mode: bool, _long: bool) -> Result<()> {
    if json_mode {
        return render_value(value, true);
    }
    let entries = value
        .get("entries")
        .or_else(|| value.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entry in entries {
        let name = ls_entry_display_label(&entry);
        let kind = ls_entry_type_label(&entry);
        let size = ls_entry_size(&entry);
        let indexing = ls_entry_indexing_status(&entry);
        println!("{kind:<7} {size:>8} {indexing:<14} {name}");
    }
    Ok(())
}

/// Selects the ls name column: real path segment, plus display metadata when it differs.
fn ls_entry_display_label(entry: &Value) -> String {
    let name = ls_entry_path_name(entry);
    let Some(display_name) = ls_entry_display_name(entry) else {
        return name;
    };

    if name.is_empty() {
        return display_name.to_owned();
    }
    if display_name == name {
        return name;
    }
    format!("{name} ({display_name})")
}

/// Reads the actual path segment a caller can pass back to Drive.
fn ls_entry_path_name(entry: &Value) -> String {
    if let Some(name) = entry
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        return name.to_owned();
    }

    if let Some(full_path) = ls_entry_full_path(entry) {
        if let Some(segment) = full_path
            .split('/')
            .rev()
            .find(|component| !component.is_empty())
        {
            return segment.to_owned();
        }
    }

    ls_entry_display_name(entry).unwrap_or("").to_owned()
}

/// Selects optional human display metadata without replacing the path name.
fn ls_entry_display_name(entry: &Value) -> Option<&str> {
    let metadata = entry.get("metadata").and_then(Value::as_object);
    entry
        .get("displayName")
        .or_else(|| entry.get("display_name"))
        .or_else(|| metadata.and_then(|value| value.get("displayName")))
        .or_else(|| metadata.and_then(|value| value.get("display_name")))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

/// Normalizes Drive path types into an ls-like fixed-width marker.
fn ls_entry_type_label(entry: &Value) -> String {
    let raw_type = entry
        .get("type")
        .or_else(|| entry.get("path_type"))
        .or_else(|| entry.get("pathType"))
        .and_then(Value::as_str)
        .unwrap_or("file")
        .trim()
        .to_ascii_lowercase();

    match raw_type.as_str() {
        "dir" | "directory" | "folder" => "<DIR>".to_owned(),
        "file" => "<FILE>".to_owned(),
        "symlink" | "link" => "<LINK>".to_owned(),
        "" => "<FILE>".to_owned(),
        other => format!("<{}>", other.to_ascii_uppercase()),
    }
}

/// Reads the size fields Drive list entries may expose for human text output.
fn ls_entry_size(entry: &Value) -> u64 {
    entry
        .get("size")
        .or_else(|| entry.get("size_bytes"))
        .or_else(|| entry.get("sizeBytes"))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        })
        .unwrap_or(0)
}

/// Reads Drive indexing status for human ls output, falling back to a visible marker.
fn ls_entry_indexing_status(entry: &Value) -> &str {
    entry
        .get("indexingStatus")
        .or_else(|| entry.get("indexing_status"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
}

/// Carries user-selected tree rendering flags independently from the API call.
#[derive(Debug, Clone, Copy)]
struct TreeRenderOptions {
    depth: Option<usize>,
    dirs_only: bool,
    full_path: bool,
    show_size: bool,
}

/// Stores one rendered Drive path and its sorted children for tree output.
#[derive(Debug, Clone)]
struct TreeNode {
    item: TreeItem,
    children: BTreeMap<String, TreeNode>,
}

/// Stores the display fields needed by the CLI tree renderer.
#[derive(Debug, Clone)]
struct TreeItem {
    label: String,
    full_path: Option<String>,
    type_label: String,
    size: u64,
    is_dir: bool,
}

/// Stores a directory path still needing a bounded tree listing request.
#[derive(Debug, Clone)]
struct TreeListTask {
    path: String,
    depth: usize,
}

impl TreeItem {
    /// Creates a synthetic directory for parent path components absent from a response.
    fn directory_placeholder(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            full_path: None,
            type_label: "<DIR>".to_owned(),
            size: 0,
            is_dir: true,
        }
    }
}

impl TreeNode {
    /// Creates a tree node with no children.
    fn new(item: TreeItem) -> Self {
        Self {
            item,
            children: BTreeMap::new(),
        }
    }
}

/// Collects Drive entries for tree rendering, applying -L before network traversal.
fn collect_tree_listing(
    client: &mut GatewayClient,
    root_path: &str,
    depth: Option<usize>,
    show_hidden: bool,
    show_system: bool,
) -> Result<Value> {
    let Some(max_depth) = depth else {
        return Ok(client.call_drive(&ls_request(
            Some(root_path),
            Some(true),
            flag_opt(show_hidden),
            flag_opt(show_system),
        ))?);
    };

    let entries =
        collect_depth_limited_tree_entries(client, root_path, max_depth, show_hidden, show_system)?;
    Ok(json!({ "entries": entries }))
}

/// Walks a Drive tree with non-recursive ls calls up to the requested depth.
fn collect_depth_limited_tree_entries(
    client: &mut GatewayClient,
    root_path: &str,
    max_depth: usize,
    show_hidden: bool,
    show_system: bool,
) -> Result<Vec<Value>> {
    if max_depth == 0 {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut queue = VecDeque::from([TreeListTask {
        path: root_path.to_owned(),
        depth: 0,
    }]);

    while let Some(task) = queue.pop_front() {
        if task.depth >= max_depth {
            continue;
        }

        let result = client.call_drive(&ls_request(
            Some(task.path.as_str()),
            None,
            flag_opt(show_hidden),
            flag_opt(show_system),
        ))?;
        let listed_entries = result
            .get("entries")
            .or_else(|| result.get("items"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        for mut entry in listed_entries {
            let full_path = ls_entry_full_path(&entry)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| join_drive_child_path(&task.path, &ls_entry_path_name(&entry)));
            if ls_entry_full_path(&entry).is_none() {
                if let Some(object) = entry.as_object_mut() {
                    object.insert("full_path".to_owned(), Value::String(full_path.clone()));
                }
            }

            let item = tree_item_from_entry(&entry);
            if item.is_dir && task.depth + 1 < max_depth {
                queue.push_back(TreeListTask {
                    path: full_path,
                    depth: task.depth + 1,
                });
            }
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Renders a recursive ls response as an ASCII tree for humans or raw JSON for scripts.
fn render_tree(
    value: &Value,
    json_mode: bool,
    root_label: &str,
    root_path: &str,
    options: TreeRenderOptions,
) -> Result<()> {
    if json_mode {
        return render_value(value, true);
    }

    let mut root = TreeNode::new(TreeItem::directory_placeholder(root_label));
    let entries = value
        .get("entries")
        .or_else(|| value.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for entry in entries {
        let item = tree_item_from_entry(&entry);
        if options.dirs_only && !item.is_dir {
            continue;
        }
        let components = tree_entry_components(&entry, root_path, &item.label);
        if components.is_empty() {
            continue;
        }
        if options
            .depth
            .is_some_and(|max_depth| components.len() > max_depth)
        {
            continue;
        }
        insert_tree_item(&mut root, &components, item);
    }

    println!("{root_label}");
    render_tree_children(&root.children, "", &options);
    Ok(())
}

/// Selects the root line shown before tree children.
fn tree_root_label(path: Option<&str>, normalized_path: &str, full_path: bool) -> String {
    if full_path {
        return normalized_path.to_owned();
    }
    path.filter(|value| !value.trim().is_empty())
        .unwrap_or(".")
        .to_owned()
}

/// Builds the tree display item from one Drive list entry.
fn tree_item_from_entry(entry: &Value) -> TreeItem {
    let label = ls_entry_display_label(entry);
    let type_label = ls_entry_type_label(entry);
    let is_dir = type_label == "<DIR>";
    TreeItem {
        label,
        full_path: ls_entry_full_path(entry).map(ToOwned::to_owned),
        type_label,
        size: ls_entry_size(entry),
        is_dir,
    }
}

/// Reads a Drive entry full path from snake_case or camelCase response fields.
fn ls_entry_full_path(entry: &Value) -> Option<&str> {
    entry
        .get("full_path")
        .or_else(|| entry.get("fullPath"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

/// Computes path components relative to the listed root path for tree placement.
fn tree_entry_components(entry: &Value, root_path: &str, fallback_label: &str) -> Vec<String> {
    let Some(full_path) = ls_entry_full_path(entry) else {
        return vec![fallback_label.to_owned()];
    };

    let root = root_path.trim_matches('/');
    let path = full_path.trim_matches('/');

    if !root.is_empty() {
        if path == root {
            return Vec::new();
        }
        if let Some(relative) = path.strip_prefix(&format!("{root}/")) {
            return split_tree_path(relative);
        }
        return vec![fallback_label.to_owned()];
    }

    split_tree_path(path)
}

/// Splits normalized Drive paths into tree components without empty segments.
fn split_tree_path(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|component| !component.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Builds a child Drive path when a list entry omits full_path metadata.
fn join_drive_child_path(parent: &str, child: &str) -> String {
    let parent = parent.trim_end_matches('/');
    let child = child.trim_matches('/');
    if child.is_empty() {
        return if parent.is_empty() {
            "/".to_owned()
        } else {
            parent.to_owned()
        };
    }
    if parent.is_empty() {
        format!("/{child}")
    } else {
        format!("{parent}/{child}")
    }
}

/// Inserts one Drive item into the tree, creating placeholder parent dirs as needed.
fn insert_tree_item(root: &mut TreeNode, components: &[String], item: TreeItem) {
    let mut node = root;
    for (index, component) in components.iter().enumerate() {
        let is_leaf = index + 1 == components.len();
        let child = node
            .children
            .entry(component.clone())
            .or_insert_with(|| TreeNode::new(TreeItem::directory_placeholder(component)));
        if is_leaf {
            child.item = item.clone();
        }
        node = child;
    }
}

/// Recursively prints sorted tree children with ASCII branch connectors.
fn render_tree_children(
    children: &BTreeMap<String, TreeNode>,
    prefix: &str,
    options: &TreeRenderOptions,
) {
    let total = children.len();
    for (index, node) in children.values().enumerate() {
        let is_last = index + 1 == total;
        let connector = if is_last { "`--" } else { "|--" };
        println!(
            "{prefix}{connector} {}",
            format_tree_item(&node.item, options)
        );
        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}|   ")
        };
        render_tree_children(&node.children, &child_prefix, options);
    }
}

/// Formats a single tree row using optional size and full-path columns.
fn format_tree_item(item: &TreeItem, options: &TreeRenderOptions) -> String {
    let label = if options.full_path {
        item.full_path.as_deref().unwrap_or(&item.label)
    } else {
        &item.label
    };

    if options.show_size {
        format!("{} {:>8} {label}", item.type_label, item.size)
    } else {
        format!("{} {label}", item.type_label)
    }
}

/// Reads a required string from a JSON object.
fn read_string_field(value: &Value, field: &'static str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .with_context(|| format!("response field '{field}' must be a non-empty string"))
}

/// Reads an optional string from a JSON object.
fn read_optional_string(value: &Value, field: &'static str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// Reads an optional bool from a JSON object.
fn read_optional_bool(value: &Value, field: &'static str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

/// Reads an optional unsigned integer from a JSON object.
fn read_optional_u64(value: &Value, field: &'static str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}

/// Reads a required string array from a JSON object.
fn read_string_array(value: &Value, field: &'static str) -> Result<Vec<String>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .context("MCP argument 'paths' must be an array")?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .context("MCP argument 'paths' must contain only strings")
        })
        .collect()
}

/// Splits an absolute Drive path into parent path and terminal name.
fn split_drive_parent_name(path: &str) -> Result<(Option<String>, String)> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        bail!("path must include a name");
    }
    let Some((parent, name)) = trimmed.rsplit_once('/') else {
        bail!("path must be absolute: {path}");
    };
    if name.is_empty() {
        bail!("path must include a name");
    }
    let parent = if parent.is_empty() {
        Some("/".to_owned())
    } else {
        Some(parent.to_owned())
    };
    Ok((parent, name.to_owned()))
}

/// Computes the Drive parent path for a relative upload target.
fn remote_parent_for(dest_root: &str, relative_path: &Path) -> Result<String> {
    let Some(parent) = relative_path.parent() else {
        return Ok(dest_root.to_owned());
    };
    let mut current = dest_root.to_owned();
    for segment in parent.components() {
        let name = segment.as_os_str().to_string_lossy();
        if !name.is_empty() {
            current = join_drive_path(&current, &name);
        }
    }
    Ok(current)
}

/// Joins a normalized Drive base path with one child segment.
fn join_drive_path(base: &str, segment: &str) -> String {
    if base == "/" {
        format!("/{segment}")
    } else {
        format!("{}/{}", base.trim_end_matches('/'), segment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_release_versions_numerically() {
        /* REQ-DISKD-CLI-020: Update checks must compare release tags numerically instead of lexically. */
        assert!(is_newer_release("v0.1.10", "0.1.9"));
        assert!(!is_newer_release("v0.1.0", "0.1.0"));
        assert_eq!(
            compare_release_versions("v1.2", "1.2.0"),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn selects_platform_release_assets() {
        /* REQ-DISKD-CLI-021: Update installs must select the archive and checksum for the current platform target. */
        let release = GitHubRelease {
            tag_name: "v0.2.0".to_owned(),
            html_url: "https://github.com/diskd-ai/diskd-cli/releases/tag/v0.2.0".to_owned(),
            assets: vec![
                GitHubReleaseAsset {
                    name: "diskd-v0.2.0-x86_64-apple-darwin.tar.gz".to_owned(),
                    browser_download_url: "https://example.test/archive".to_owned(),
                },
                GitHubReleaseAsset {
                    name: "diskd-v0.2.0-x86_64-apple-darwin.tar.gz.sha256".to_owned(),
                    browser_download_url: "https://example.test/checksum".to_owned(),
                },
            ],
        };

        let pair = select_release_asset_pair(&release, "x86_64-apple-darwin").unwrap();

        assert_eq!(pair.archive_url, "https://example.test/archive");
        assert_eq!(pair.checksum_url, "https://example.test/checksum");
    }

    #[test]
    fn parses_sha256_checksum_document() {
        /* REQ-DISKD-CLI-022: Update installs must verify the downloaded archive before replacing the binary. */
        let checksum = parse_sha256_checksum(
            "ABCDEFabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123  diskd.tar.gz\n",
        )
        .unwrap();

        assert_eq!(
            checksum,
            "abcdefabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123"
        );
    }

    #[test]
    fn maps_supported_release_targets() {
        /* REQ-DISKD-CLI-023: Update installs must use the release target names emitted by GitHub Actions. */
        assert_eq!(
            release_target_for("linux", "x86_64"),
            Some("x86_64-unknown-linux-musl")
        );
        assert_eq!(
            release_target_for("macos", "aarch64"),
            Some("aarch64-apple-darwin")
        );
        assert_eq!(
            release_target_for("windows", "x86_64"),
            Some("x86_64-pc-windows-msvc")
        );
        assert_eq!(release_target_for("freebsd", "x86_64"), None);
    }

    #[test]
    fn builds_mcp_agent_config_for_stdio_server() {
        /* REQ-DISKD-CLI-024: Direct MCP serve runs must show a stable LLM-agent configuration snippet. */
        let config = mcp_agent_config("/usr/local/bin/diskd");

        assert_eq!(
            config["mcpServers"]["diskd"]["command"],
            "/usr/local/bin/diskd"
        );
        assert_eq!(config["mcpServers"]["diskd"]["args"][0], "mcp");
        assert_eq!(config["mcpServers"]["diskd"]["args"][1], "serve");
        assert_eq!(
            config["mcpServers"]["diskd"]["env"]["APIS_BASE_URL"],
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn builds_browser_login_url_for_production_app() {
        /* REQ-DISKD-CLI-025: Browser login must open the production OAuth Apps page with a local callback by default. */
        let url = browser_login_url(
            DEFAULT_LOGIN_APP_URL,
            "http://127.0.0.1:49152/callback",
            "state 1",
        );

        assert_eq!(
            url,
            "https://app.iosya.com/oauth-apps?source=diskd-cli&callback=http%3A%2F%2F127.0.0.1%3A49152%2Fcallback&state=state%201"
        );
    }

    #[test]
    fn resolves_dev_browser_login_app_url() {
        /* REQ-DISKD-CLI-026: diskd login --dev must use the development app host. */
        assert_eq!(
            resolve_login_app_url(true, None).unwrap(),
            DEV_LOGIN_APP_URL
        );
        assert_eq!(
            resolve_login_app_url(false, Some("https://app.example/oauth-apps")).unwrap(),
            "https://app.example/oauth-apps"
        );
        assert!(resolve_login_app_url(true, Some("https://app.example")).is_err());
    }

    #[test]
    fn parses_browser_login_credentials_form() {
        /* REQ-DISKD-CLI-027: Browser login must accept credentials.json posted from the OAuth Apps page. */
        let credentials = r#"{"issuer":"https://issuer.example","clientId":"client-1","clientSecret":"secret-1","audience":"diskd-api","apisUrl":"https://apis.example"}"#;
        let body = format!("state=state-1&credentials={}", percent_encode(credentials));

        let parsed = parse_browser_credentials_form(&body, "state-1").unwrap();

        assert_eq!(parsed.issuer, "https://issuer.example");
        assert_eq!(parsed.client_id, "client-1");
        assert_eq!(parsed.client_secret, "secret-1");
        assert_eq!(parsed.audience, "diskd-api");
        assert_eq!(parsed.apis_url, "https://apis.example");
    }

    #[test]
    fn rejects_browser_login_state_mismatch() {
        /* REQ-DISKD-CLI-028: Browser login callbacks must match the local state value. */
        let result = parse_browser_credentials_form("state=wrong&credentials=%7B%7D", "expected");

        assert!(result.is_err());
    }
}
