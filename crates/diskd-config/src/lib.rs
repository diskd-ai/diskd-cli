use base64::prelude::{Engine as _, BASE64_URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Identifies the selected project context without leaking storage details into callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectId(String);

impl ProjectId {
    /// Creates a project id after rejecting empty values from config, env, or CLI flags.
    pub fn new(value: impl Into<String>) -> Result<Self, ConfigError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ConfigError::EmptyProjectId);
        }
        Ok(Self(value))
    }

    /// Exposes the opaque project id for display and config persistence.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Captures the effective Drive scope used by command path normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveContext {
    WorkspaceRoot,
    Project(ProjectId),
}

/// Represents a normalized absolute Drive path ready for JSON-RPC payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrivePath(String);

impl DrivePath {
    /// Returns the normalized virtual Drive path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Holds non-secret CLI preferences parsed from config.yaml.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiskdConfig {
    pub base_url: Option<String>,
    pub workspace: Option<String>,
    pub project: Option<String>,
    pub project_name: Option<String>,
    pub output: Option<String>,
}

/// Stores OAuth bearer material separately from non-secret config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCredentials {
    #[serde(default, alias = "accessToken")]
    pub access_token: String,
    #[serde(default, alias = "refreshToken")]
    pub refresh_token: Option<String>,
    #[serde(default, alias = "tokenType")]
    pub token_type: Option<String>,
    #[serde(default, alias = "expiresAt")]
    pub expires_at: Option<u64>,
}

/// Describes a client-credentials fixture used for non-interactive login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCredentialsFile {
    pub issuer: String,
    #[serde(alias = "clientId")]
    pub client_id: String,
    #[serde(alias = "clientSecret")]
    pub client_secret: String,
    pub audience: String,
    #[serde(alias = "apisUrl")]
    pub apis_url: String,
}

/// Contains identity metadata decoded from a JWT payload without signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwtIdentity {
    pub workspace_id: String,
    pub subject: Option<String>,
    pub user_id: Option<String>,
    pub client_id: Option<String>,
    pub scopes: Vec<String>,
}

/// Models local configuration and path validation failures before any network call is made.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("project id must not be empty")]
    EmptyProjectId,
    #[error("drive path contains invalid segment: {segment}")]
    InvalidPathSegment { segment: String },
    #[error("JWT must have exactly 3 dot-separated parts")]
    InvalidJwtParts,
    #[error("JWT payload is not valid base64url: {reason}")]
    InvalidJwtPayloadEncoding { reason: String },
    #[error("JWT payload is not valid JSON: {reason}")]
    InvalidJwtPayloadJson { reason: String },
    #[error("JWT has no workspace_id claim")]
    MissingWorkspaceIdClaim,
    #[error("config line {line} must use 'key: value' syntax")]
    InvalidConfigLine { line: usize },
    #[error("credentials document is invalid JSON: {reason}")]
    InvalidCredentialsJson { reason: String },
}

/// Applies flag -> env -> config precedence for scalar non-secret settings.
pub fn resolve_setting(
    flag: Option<&str>,
    env: Option<&str>,
    config: Option<&str>,
) -> Option<String> {
    first_non_empty([flag, env, config])
}

/// Normalizes user command paths under either workspace root or /Projects/{projectId}.
pub fn normalize_drive_path(
    context: &DriveContext,
    input: Option<&str>,
) -> Result<DrivePath, ConfigError> {
    let relative = normalize_relative_path(input)?;
    let path = match context {
        DriveContext::WorkspaceRoot => join_root("/", &relative),
        DriveContext::Project(project_id) => {
            let prefix = format!("/Projects/{}", project_id.as_str());
            join_root(&prefix, &relative)
        }
    };
    Ok(DrivePath(path))
}

/// Parses the CLI's small YAML config subset without accepting secrets.
pub fn parse_config_document(document: &str) -> Result<DiskdConfig, ConfigError> {
    let mut config = DiskdConfig::default();
    for (index, raw_line) in document.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            return Err(ConfigError::InvalidConfigLine { line: index + 1 });
        };
        let key = raw_key.trim();
        let value = normalize_config_value(raw_value);
        let value = if value.is_empty() { None } else { Some(value) };
        match key {
            "base_url" => config.base_url = value,
            "workspace" => config.workspace = value,
            "project" => config.project = value,
            "project_name" => config.project_name = value,
            "output" => config.output = value,
            _ => {}
        }
    }
    Ok(config)
}

/// Serializes non-secret config back to config.yaml using the supported key set.
pub fn format_config_document(config: &DiskdConfig) -> String {
    let mut lines = Vec::new();
    push_config_line(&mut lines, "base_url", config.base_url.as_deref());
    push_config_line(&mut lines, "workspace", config.workspace.as_deref());
    push_config_line(&mut lines, "project", config.project.as_deref());
    push_config_line(&mut lines, "project_name", config.project_name.as_deref());
    push_config_line(&mut lines, "output", config.output.as_deref());
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

/// Parses a stored credentials JSON document from the secret token file.
pub fn parse_stored_credentials(document: &str) -> Result<StoredCredentials, ConfigError> {
    serde_json::from_str(document).map_err(|error| ConfigError::InvalidCredentialsJson {
        reason: error.to_string(),
    })
}

/// Serializes stored credentials as compact JSON for the secret token file.
pub fn format_stored_credentials(credentials: &StoredCredentials) -> Result<String, ConfigError> {
    serde_json::to_string(credentials).map_err(|error| ConfigError::InvalidCredentialsJson {
        reason: error.to_string(),
    })
}

/// Parses a client-credentials login fixture without persisting its secret fields.
pub fn parse_client_credentials_file(document: &str) -> Result<ClientCredentialsFile, ConfigError> {
    serde_json::from_str(document).map_err(|error| ConfigError::InvalidCredentialsJson {
        reason: error.to_string(),
    })
}

/// Extracts workspace/user metadata from a JWT payload using the platform SDK precedence.
pub fn decode_jwt_identity(access_token: &str) -> Result<JwtIdentity, ConfigError> {
    let parts = access_token.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(ConfigError::InvalidJwtParts);
    }
    let payload = BASE64_URL_SAFE_NO_PAD.decode(parts[1]).map_err(|error| {
        ConfigError::InvalidJwtPayloadEncoding {
            reason: error.to_string(),
        }
    })?;
    let claims: Value =
        serde_json::from_slice(&payload).map_err(|error| ConfigError::InvalidJwtPayloadJson {
            reason: error.to_string(),
        })?;

    let ext = claims.get("ext").and_then(Value::as_object);
    let workspace_id = read_nested_string(ext, "workspace_id")
        .or_else(|| read_top_string(&claims, "workspace_id"))
        .or_else(|| read_top_string(&claims, "sub"))
        .ok_or(ConfigError::MissingWorkspaceIdClaim)?;

    Ok(JwtIdentity {
        workspace_id,
        subject: read_top_string(&claims, "sub"),
        user_id: read_nested_string(ext, "user_id").or_else(|| read_top_string(&claims, "user_id")),
        client_id: read_top_string(&claims, "client_id"),
        scopes: read_scopes(&claims),
    })
}

fn first_non_empty(values: [Option<&str>; 3]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_relative_path(input: Option<&str>) -> Result<String, ConfigError> {
    let raw = input.unwrap_or("/").trim();
    let without_leading = raw.trim_start_matches('/');
    let mut parts = Vec::new();

    for segment in without_leading.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." {
            return Err(ConfigError::InvalidPathSegment {
                segment: segment.to_owned(),
            });
        }
        parts.push(segment);
    }

    Ok(parts.join("/"))
}

fn join_root(prefix: &str, relative: &str) -> String {
    if relative.is_empty() {
        return prefix.to_owned();
    }
    if prefix == "/" {
        format!("/{relative}")
    } else {
        format!("{prefix}/{relative}")
    }
}

fn normalize_config_value(raw_value: &str) -> String {
    raw_value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_owned()
}

fn push_config_line(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            lines.push(format!("{key}: {}", value.trim()));
        }
    }
}

fn read_top_string(claims: &Value, key: &str) -> Option<String> {
    claims
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn read_nested_string(
    object: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<String> {
    object?
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn read_scopes(claims: &Value) -> Vec<String> {
    claims
        .get("scope")
        .and_then(Value::as_str)
        .map(|scope| {
            scope
                .split_whitespace()
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::prelude::BASE64_URL_SAFE_NO_PAD;

    /* REQ-DISKD-CLI-001: Workspace context paths must normalize to absolute Drive paths without project scope. */
    #[test]
    fn normalizes_workspace_root_paths() {
        assert_eq!(
            normalize_drive_path(&DriveContext::WorkspaceRoot, None)
                .unwrap()
                .as_str(),
            "/"
        );
        assert_eq!(
            normalize_drive_path(&DriveContext::WorkspaceRoot, Some("contracts/a.txt"))
                .unwrap()
                .as_str(),
            "/contracts/a.txt"
        );
        assert_eq!(
            normalize_drive_path(&DriveContext::WorkspaceRoot, Some("/contracts/a.txt"))
                .unwrap()
                .as_str(),
            "/contracts/a.txt"
        );
    }

    /* REQ-DISKD-CLI-002: Project context paths must normalize under /Projects/{projectId}. */
    #[test]
    fn normalizes_project_context_paths() {
        let context = DriveContext::Project(ProjectId::new("01PROJECT").unwrap());

        assert_eq!(
            normalize_drive_path(&context, None).unwrap().as_str(),
            "/Projects/01PROJECT"
        );
        assert_eq!(
            normalize_drive_path(&context, Some("/")).unwrap().as_str(),
            "/Projects/01PROJECT"
        );
        assert_eq!(
            normalize_drive_path(&context, Some("notes/a.md"))
                .unwrap()
                .as_str(),
            "/Projects/01PROJECT/notes/a.md"
        );
        assert_eq!(
            normalize_drive_path(&context, Some("/notes/a.md"))
                .unwrap()
                .as_str(),
            "/Projects/01PROJECT/notes/a.md"
        );
    }

    /* REQ-DISKD-CLI-003: Project path normalization must reject traversal segments before JSON-RPC payload creation. */
    #[test]
    fn rejects_traversal_segments() {
        let context = DriveContext::Project(ProjectId::new("01PROJECT").unwrap());

        assert_eq!(
            normalize_drive_path(&context, Some("../secret")).unwrap_err(),
            ConfigError::InvalidPathSegment {
                segment: "..".to_owned()
            }
        );
    }

    /* REQ-DISKD-CLI-004: Scalar config resolution must use flag, then environment, then config file precedence. */
    #[test]
    fn resolves_setting_precedence() {
        assert_eq!(
            resolve_setting(Some(" flag "), Some("env"), Some("config")),
            Some("flag".to_owned())
        );
        assert_eq!(
            resolve_setting(Some(""), Some("env"), Some("config")),
            Some("env".to_owned())
        );
        assert_eq!(
            resolve_setting(None, Some(""), Some("config")),
            Some("config".to_owned())
        );
    }

    /* REQ-DISKD-CLI-012: Config files must keep only non-secret CLI settings. */
    #[test]
    fn parses_and_formats_non_secret_config() {
        let parsed = parse_config_document(
            r#"
            base_url: https://apis.example
            project: 01PROJECT
            project_name: Demo
            ignored_secret: no
            "#,
        )
        .unwrap();

        assert_eq!(parsed.base_url.as_deref(), Some("https://apis.example"));
        assert_eq!(parsed.project.as_deref(), Some("01PROJECT"));
        assert_eq!(
            format_config_document(&parsed),
            "base_url: https://apis.example\nproject: 01PROJECT\nproject_name: Demo\n"
        );
    }

    /* REQ-DISKD-CLI-013: whoami must derive workspace from ext.workspace_id, workspace_id, then sub. */
    #[test]
    fn decodes_jwt_identity_with_platform_precedence() {
        let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = BASE64_URL_SAFE_NO_PAD.encode(
            r#"{"sub":"client-sub","workspace_id":"ws-top","ext":{"workspace_id":"ws-ext","user_id":"user-1"},"scope":"drive:read projects:read"}"#,
        );
        let token = format!("{header}.{payload}.signature");

        let identity = decode_jwt_identity(&token).unwrap();

        assert_eq!(identity.workspace_id, "ws-ext");
        assert_eq!(identity.subject.as_deref(), Some("client-sub"));
        assert_eq!(identity.user_id.as_deref(), Some("user-1"));
        assert_eq!(
            identity.scopes,
            vec!["drive:read".to_owned(), "projects:read".to_owned()]
        );
    }
}
