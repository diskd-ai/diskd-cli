use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use diskd_client::{
    biquery_request, decode_upload_start, download_url_request, glob_request, grep_request,
    ls_request, metadata_request, path_create_request, path_delete_request, path_rename_request,
    read_file_request, request_client_credentials_token, upload_commit_request,
    upload_start_request, vsearch_request, ClientCredentialsTokenParams, GatewayClient,
    JsonRpcRequest,
};
use diskd_config::{
    decode_jwt_identity, format_config_document, format_stored_credentials, normalize_drive_path,
    parse_client_credentials_file, parse_config_document, parse_stored_credentials,
    resolve_setting, DiskdConfig, DriveContext, ProjectId, StoredCredentials,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_BASE_URL: &str = "https://apis.diskd.ai";
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
        ignore_case: bool,
        #[arg(long)]
        files_with_matches: bool,
    },
    Vsearch {
        query: String,
        paths: Vec<String>,
        #[arg(long, alias = "limit")]
        top: Option<u64>,
    },
    Cat {
        path: String,
        #[arg(long)]
        version: Option<u64>,
    },
    Read {
        path: String,
        #[arg(long)]
        parts_limit: Option<u64>,
        #[arg(long)]
        parts_offset: Option<u64>,
    },
    Stat {
        path: String,
    },
    Biquery {
        query: String,
        paths: Vec<String>,
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
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        credentials_file: Option<PathBuf>,
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
}

/// Groups MCP subcommands under the mcp command namespace.
#[derive(Debug, Subcommand)]
enum McpCommand {
    Serve,
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
    let mut state = load_runtime_state(cli.config.as_deref())?;
    match &cli.command {
        Command::Login {
            token,
            credentials_file,
        } => login(
            &mut state,
            token.as_deref(),
            credentials_file.as_deref(),
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
        Command::Version => print_version(&cli),
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
            ignore_case,
            files_with_matches,
        } => {
            reject_unsupported_flag(*ignore_case, "--ignore-case")?;
            reject_unsupported_flag(*files_with_matches, "--files-with-matches")?;
            let paths = normalize_many_paths(&context, paths)?;
            let result = client.call_drive(&grep_request(query, &paths))?;
            render_value(&result, cli.json)
        }
        Command::Vsearch { query, paths, top } => {
            let paths = normalize_many_paths(&context, paths)?;
            let result = client.call_drive(&vsearch_request(query, &paths, *top))?;
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

/// Logs in with either a raw token or the provided OAuth client-credentials fixture.
fn login(
    state: &mut RuntimeState,
    token: Option<&str>,
    credentials_file: Option<&Path>,
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
            token
        }
        (Some(_), Some(_)) => bail!("use either --token or --credentials-file, not both"),
        (None, None) => bail!("login requires --token or --credentials-file"),
    };
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
            Ok(read_file_request(
                path.as_str(),
                read_optional_u64(arguments, "parts_limit"),
                read_optional_u64(arguments, "parts_offset"),
            ))
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
            ))
        }
        "tools__vsearch" => {
            let query = read_string_field(arguments, "query")?;
            let paths = read_string_array(arguments, "paths")?;
            let limit = read_optional_u64(arguments, "limit")
                .or_else(|| read_optional_u64(arguments, "top_k"));
            Ok(vsearch_request(
                &query,
                &normalize_many_paths(context, &paths)?,
                limit,
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
                    "parts_offset": { "type": "integer" }
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
                    "paths": { "type": "array", "items": { "type": "string" } }
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
                    "limit": { "type": "integer" }
                },
                "required": ["query", "paths"]
            }
        }),
        json!({
            "name": "tools__bi_query",
            "description": "Run SQL queries on spreadsheet/Excel files mapped to SQLite.",
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
fn render_ls(value: &Value, json_mode: bool, long: bool) -> Result<()> {
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
        let name = entry
            .get("full_path")
            .or_else(|| entry.get("fullPath"))
            .or_else(|| entry.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if long {
            let kind = entry.get("type").and_then(Value::as_str).unwrap_or("?");
            let size = entry.get("size").and_then(Value::as_u64).unwrap_or(0);
            println!("{kind}\t{size}\t{name}");
        } else {
            println!("{name}");
        }
    }
    Ok(())
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
