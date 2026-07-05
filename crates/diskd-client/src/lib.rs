use base64::prelude::{Engine as _, BASE64_STANDARD};
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, ETAG};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use thiserror::Error;

/// Captures JSON values needed by the first Drive JSON-RPC contract slice.
#[derive(Debug, Clone, PartialEq)]
pub enum RpcValue {
    String(String),
    U64(u64),
    Bool(bool),
    StringList(Vec<String>),
    Json(Value),
}

/// Keeps JSON-RPC params ordered and typed so tests can verify exact wire intent.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcParam {
    pub name: String,
    pub value: RpcValue,
}

/// Describes a Drive JSON-RPC request before transport serializes it to JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: Vec<RpcParam>,
}

/// Represents the minimal project fields needed by CLI context selection.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
}

/// Represents the upload intent returned by drive/upload/start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadStart {
    pub intent_id: String,
    pub inode: String,
    pub upload_url: String,
}

/// Describes an OAuth client-credentials token request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCredentialsTokenParams {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub audience: String,
    pub scopes: Vec<String>,
}

/// Models URL, transport, JSON-RPC, and OAuth failures at the gateway boundary.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("base URL must not be empty")]
    EmptyBaseUrl,
    #[error("bearer token must not be empty")]
    EmptyBearerToken,
    #[error("HTTP transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("Invalid JSON response: {reason}")]
    InvalidJson { reason: String },
    #[error("Invalid JSON-RPC response: expected object")]
    InvalidJsonRpcResponse,
    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),
    #[error("response is missing required field '{field}'")]
    MissingField { field: &'static str },
    #[error("response field '{field}' has an invalid type")]
    InvalidField { field: &'static str },
}

/// Executes authenticated calls through apis-service.
pub struct GatewayClient {
    base_url: String,
    bearer_token: String,
    http: Client,
    next_id: u64,
}

impl GatewayClient {
    /// Creates a gateway client after validating base URL and bearer material.
    pub fn new(base_url: &str, bearer_token: &str) -> Result<Self, ClientError> {
        let base_url = trim_base_url(base_url)?;
        let bearer_token = bearer_token.trim();
        if bearer_token.is_empty() {
            return Err(ClientError::EmptyBearerToken);
        }
        Ok(Self {
            base_url,
            bearer_token: bearer_token.to_owned(),
            http: Client::new(),
            next_id: 1,
        })
    }

    /// Calls the Drive JSON-RPC endpoint and returns the raw result value.
    pub fn call_drive(&mut self, request: &JsonRpcRequest) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        let url = drive_rpc_url(&self.base_url)?;
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.bearer_token)
            .json(&json_rpc_payload(request, id))
            .send()?;
        read_json_rpc_response(response)
    }

    /// Lists projects visible to the current credential through platform projects.
    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, ClientError> {
        let url = projects_list_url(&self.base_url)?;
        let response = self.http.get(url).bearer_auth(&self.bearer_token).send()?;
        read_json_response(response)
    }

    /// Downloads bytes from a URL returned by drive/files/download-url.
    pub fn download_bytes(&self, url: &str) -> Result<Vec<u8>, ClientError> {
        let response = self.http.get(url).bearer_auth(&self.bearer_token).send()?;
        read_bytes_response(response)
    }

    /// Uploads bytes to an upload-proxy URL returned by drive/upload/start.
    pub fn put_upload(
        &self,
        upload_url: &str,
        intent_id: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> Result<String, ClientError> {
        let url = resolve_drive_relative_url(&self.base_url, upload_url)?;
        let response = self
            .http
            .put(url)
            .bearer_auth(&self.bearer_token)
            .header(CONTENT_TYPE, content_type)
            .header("X-Upload-Intent-Id", intent_id)
            .body(body)
            .send()?;
        read_upload_etag(response)
    }
}

/// Builds the public apis-service Drive JSON-RPC URL used by every Drive command.
pub fn drive_rpc_url(base_url: &str) -> Result<String, ClientError> {
    let base = trim_base_url(base_url)?;
    Ok(format!("{base}/v1/os/drive/api/v1"))
}

/// Builds the public apis-service project-listing URL used by set-context --list.
pub fn projects_list_url(base_url: &str) -> Result<String, ClientError> {
    let base = trim_base_url(base_url)?;
    Ok(format!("{base}/v1/platform/projects/api/projects"))
}

/// Requests a bearer token from the issuer's OIDC token endpoint.
pub fn request_client_credentials_token(
    params: &ClientCredentialsTokenParams,
) -> Result<String, ClientError> {
    let http = Client::new();
    let discovery = fetch_oidc_discovery(&http, &params.issuer)?;
    let basic = BASE64_STANDARD.encode(format!("{}:{}", params.client_id, params.client_secret));
    let mut form = vec![
        ("grant_type", "client_credentials".to_owned()),
        ("client_id", params.client_id.clone()),
        ("audience", params.audience.clone()),
    ];
    if !params.scopes.is_empty() {
        form.push(("scope", params.scopes.join(" ")));
    }
    let response = http
        .post(discovery.token_endpoint)
        .header("Authorization", format!("Basic {basic}"))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&form)
        .send()?;
    let token: OAuthTokenResponse = read_json_response(response)?;
    if token.access_token.trim().is_empty() {
        return Err(ClientError::MissingField {
            field: "access_token",
        });
    }
    Ok(token.access_token)
}

/// Creates the resolved ls JSON-RPC request contract.
pub fn ls_request(
    path: Option<&str>,
    recursive: Option<bool>,
    show_hidden: Option<bool>,
    show_system: Option<bool>,
) -> JsonRpcRequest {
    let mut params = Vec::new();
    if let Some(path) = path {
        params.push(param_string("path", path));
    }
    if let Some(recursive) = recursive {
        params.push(param_bool("recursive", recursive));
    }
    if let Some(show_hidden) = show_hidden {
        params.push(param_bool("show_hidden", show_hidden));
    }
    if let Some(show_system) = show_system {
        params.push(param_bool("show_system", show_system));
    }
    request("paths/tools/ls", params)
}

/// Creates the resolved glob JSON-RPC request contract.
pub fn glob_request(
    pattern: &str,
    path: Option<&str>,
    show_hidden: Option<bool>,
    show_system: Option<bool>,
) -> JsonRpcRequest {
    let mut params = vec![param_string("pattern", pattern)];
    if let Some(path) = path {
        params.push(param_string("path", path));
    }
    if let Some(show_hidden) = show_hidden {
        params.push(param_bool("show_hidden", show_hidden));
    }
    if let Some(show_system) = show_system {
        params.push(param_bool("show_system", show_system));
    }
    request("paths/tools/glob", params)
}

/// Creates the resolved grep JSON-RPC request contract.
pub fn grep_request(
    query: &str,
    paths: &[String],
    limit: Option<u64>,
    offset: Option<u64>,
) -> JsonRpcRequest {
    let mut params = vec![
        param_string("query", query),
        param_string_list("paths", paths.to_vec()),
    ];
    if let Some(limit) = limit {
        params.push(param_u64("limit", limit));
    }
    if let Some(offset) = offset {
        params.push(param_u64("offset", offset));
    }
    request("paths/tools/grep", params)
}

/// Creates the resolved vsearch JSON-RPC request contract.
pub fn vsearch_request(
    query: &str,
    paths: &[String],
    limit: Option<u64>,
    offset: Option<u64>,
) -> JsonRpcRequest {
    let mut params = vec![
        param_string("query", query),
        param_string_list("paths", paths.to_vec()),
    ];
    if let Some(limit) = limit {
        params.push(param_u64("limit", limit));
    }
    if let Some(offset) = offset {
        params.push(param_u64("offset", offset));
    }
    request("paths/tools/vsearch", params)
}

/// Creates the structured file-read JSON-RPC request contract.
pub fn read_file_request(
    path: &str,
    parts_limit: Option<u64>,
    parts_offset: Option<u64>,
) -> JsonRpcRequest {
    let mut params = vec![param_string("path", path)];
    if let Some(parts_limit) = parts_limit {
        params.push(param_u64("parts_limit", parts_limit));
    }
    if let Some(parts_offset) = parts_offset {
        params.push(param_u64("parts_offset", parts_offset));
    }
    request("paths/tools/read", params)
}

/// Creates the resolved spreadsheet BI query JSON-RPC request contract.
pub fn biquery_request(query: &str, paths: &[String]) -> JsonRpcRequest {
    request(
        "paths/tools/bi-query",
        vec![
            param_string("query", query),
            param_string_list("paths", paths.to_vec()),
        ],
    )
}

/// Creates a Telegram Drive DB database with optional schema and placement flags.
pub fn telegram_db_create_request(
    name: &str,
    schema: Option<Value>,
    check_exists: Option<bool>,
    recreate: Option<bool>,
    directory: Option<&str>,
) -> JsonRpcRequest {
    let mut params = vec![param_string("name", name)];
    if let Some(schema) = schema {
        params.push(param_json("schema", schema));
    }
    if let Some(check_exists) = check_exists {
        params.push(param_bool("check_exists", check_exists));
    }
    if let Some(recreate) = recreate {
        params.push(param_bool("recreate", recreate));
    }
    if let Some(directory) = directory {
        params.push(param_string("directory", directory));
    }
    request("drive/telegram/create", params)
}

/// Creates a Telegram Drive DB insert request with row objects supplied as JSON.
pub fn telegram_db_insert_request(name: &str, table: &str, rows: Value) -> JsonRpcRequest {
    request(
        "drive/telegram/insert",
        vec![
            param_string("name", name),
            param_string("table", table),
            param_json("rows", rows),
        ],
    )
}

/// Creates a Telegram Drive DB SQL query request with optional positional parameters.
pub fn telegram_db_query_request(
    name: &str,
    sql: &str,
    parameters: Option<Value>,
) -> JsonRpcRequest {
    let mut params = vec![param_string("name", name), param_string("sql", sql)];
    if let Some(parameters) = parameters {
        params.push(param_json("parameters", parameters));
    }
    request("drive/telegram/query", params)
}

/// Creates a Telegram Drive DB commit request for pending row changes.
pub fn telegram_db_commit_request(name: &str) -> JsonRpcRequest {
    request("drive/telegram/commit", vec![param_string("name", name)])
}

/// Creates a Telegram Drive DB metadata request.
pub fn telegram_db_metadata_request(name: &str) -> JsonRpcRequest {
    request("drive/telegram/metadata", vec![param_string("name", name)])
}

/// Creates a Telegram Drive DB drop request.
pub fn telegram_db_drop_request(name: &str) -> JsonRpcRequest {
    request("drive/telegram/drop", vec![param_string("name", name)])
}

/// Creates the file metadata request used by diskd stat.
pub fn metadata_request(path: &str) -> JsonRpcRequest {
    request("paths/tools/inode-ls", vec![param_string("path", path)])
}

/// Creates the binary download URL request used by diskd cat.
pub fn download_url_request(path: &str, version: Option<u64>) -> JsonRpcRequest {
    let mut params = vec![param_string("path", path)];
    if let Some(version) = version {
        params.push(param_u64("version", version));
    }
    request("drive/files/download-url", params)
}

/// Creates an upload-start request with optional metadata used by upload and sync.
pub fn upload_start_request(
    name: &str,
    size: u64,
    sha256_root: &str,
    parent_path: Option<&str>,
    mime_type: Option<&str>,
    force: Option<bool>,
) -> JsonRpcRequest {
    let mut params = vec![
        param_string("name", name),
        param_u64("size", size),
        param_string("sha256_root", sha256_root),
    ];
    if let Some(parent_path) = parent_path {
        params.push(param_string("parent_path", parent_path));
    }
    if let Some(mime_type) = mime_type {
        params.push(param_string("mime_type", mime_type));
    }
    if let Some(force) = force {
        params.push(param_bool("force", force));
    }
    request("drive/upload/start", params)
}

/// Creates the upload-commit request used after the upload proxy returns an ETag.
pub fn upload_commit_request(intent_id: &str, etag: &str) -> JsonRpcRequest {
    request(
        "drive/upload/commit",
        vec![
            param_string("intent_id", intent_id),
            param_string("etag", etag),
        ],
    )
}

/// Creates the path-create request used by mkdir and project setup helpers.
pub fn path_create_request(
    name: &str,
    parent_path: Option<&str>,
    path_type: &str,
) -> JsonRpcRequest {
    let mut params = vec![
        param_string("name", name),
        param_string("dir_name", name),
        param_string("type", path_type),
    ];
    if let Some(parent_path) = parent_path {
        params.push(param_string("parent_path", parent_path));
    }
    request("drive/paths/create", params)
}

/// Creates the path-delete request used by rm.
pub fn path_delete_request(paths: &[String], recursive: Option<bool>) -> JsonRpcRequest {
    let mut params = vec![param_string_list("paths", paths.to_vec())];
    if let Some(recursive) = recursive {
        params.push(param_bool("recursive", recursive));
    }
    request("drive/paths/delete", params)
}

/// Creates the path-rename request used by mv.
pub fn path_rename_request(
    path: &str,
    new_name: &str,
    new_parent_path: Option<&str>,
) -> JsonRpcRequest {
    let mut params = vec![
        param_string("path", path),
        param_string("new_name", new_name),
    ];
    if let Some(new_parent_path) = new_parent_path {
        params.push(param_string("new_parent_path", new_parent_path));
    }
    request("drive/paths/rename", params)
}

/// Converts a tested request contract into the JSON-RPC HTTP payload.
pub fn json_rpc_payload(request: &JsonRpcRequest, id: u64) -> Value {
    json!({
        "jsonrpc": request.jsonrpc,
        "method": request.method,
        "params": rpc_params_json(&request.params),
        "id": id,
    })
}

/// Decodes a drive/upload/start result into the fields needed by PUT + commit.
pub fn decode_upload_start(value: &Value) -> Result<UploadStart, ClientError> {
    Ok(UploadStart {
        intent_id: read_required_string(value, "intent_id")?,
        inode: read_required_string(value, "inode")?,
        upload_url: read_required_string(value, "upload_url")?,
    })
}

fn trim_base_url(base_url: &str) -> Result<String, ClientError> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(ClientError::EmptyBaseUrl);
    }
    Ok(trimmed.to_owned())
}

fn request(method: &'static str, params: Vec<RpcParam>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0",
        method,
        params,
    }
}

fn param_string(name: &str, value: &str) -> RpcParam {
    RpcParam {
        name: name.to_owned(),
        value: RpcValue::String(value.to_owned()),
    }
}

fn param_string_list(name: &str, value: Vec<String>) -> RpcParam {
    RpcParam {
        name: name.to_owned(),
        value: RpcValue::StringList(value),
    }
}

fn param_u64(name: &str, value: u64) -> RpcParam {
    RpcParam {
        name: name.to_owned(),
        value: RpcValue::U64(value),
    }
}

fn param_bool(name: &str, value: bool) -> RpcParam {
    RpcParam {
        name: name.to_owned(),
        value: RpcValue::Bool(value),
    }
}

fn param_json(name: &str, value: Value) -> RpcParam {
    RpcParam {
        name: name.to_owned(),
        value: RpcValue::Json(value),
    }
}

fn rpc_params_json(params: &[RpcParam]) -> Value {
    let mut map = Map::new();
    for param in params {
        map.insert(param.name.clone(), rpc_value_json(&param.value));
    }
    Value::Object(map)
}

fn rpc_value_json(value: &RpcValue) -> Value {
    match value {
        RpcValue::String(value) => Value::String(value.clone()),
        RpcValue::U64(value) => json!(value),
        RpcValue::Bool(value) => json!(value),
        RpcValue::StringList(value) => json!(value),
        RpcValue::Json(value) => value.clone(),
    }
}

fn read_json_rpc_response(response: reqwest::blocking::Response) -> Result<Value, ClientError> {
    let status = response.status();
    let text = response.text()?;
    let body = parse_json_text(&text)?;
    if !status.is_success() {
        return Err(ClientError::Http {
            status: status.as_u16(),
            message: describe_error_body(&body),
        });
    }
    let Value::Object(map) = body else {
        return Err(ClientError::InvalidJsonRpcResponse);
    };
    if let Some(error) = map.get("error") {
        return Err(ClientError::JsonRpc(error.to_string()));
    }
    Ok(map.get("result").cloned().unwrap_or(Value::Null))
}

fn read_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::blocking::Response,
) -> Result<T, ClientError> {
    let status = response.status();
    let text = response.text()?;
    let body = parse_json_text(&text)?;
    if !status.is_success() {
        return Err(ClientError::Http {
            status: status.as_u16(),
            message: describe_error_body(&body),
        });
    }
    serde_json::from_value(body).map_err(|error| ClientError::InvalidJson {
        reason: error.to_string(),
    })
}

fn read_bytes_response(response: reqwest::blocking::Response) -> Result<Vec<u8>, ClientError> {
    let status = response.status();
    if !status.is_success() {
        let text = response.text().unwrap_or_else(|error| error.to_string());
        return Err(ClientError::Http {
            status: status.as_u16(),
            message: text.chars().take(200).collect(),
        });
    }
    Ok(response.bytes()?.to_vec())
}

fn read_upload_etag(response: reqwest::blocking::Response) -> Result<String, ClientError> {
    let status = response.status();
    let header_etag = response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let text = response.text()?;
    if !status.is_success() {
        return Err(ClientError::Http {
            status: status.as_u16(),
            message: text.chars().take(200).collect(),
        });
    }
    let body_etag = if text.trim().is_empty() {
        None
    } else {
        let parsed = parse_json_text(&text)?;
        parsed
            .get("etag")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    };
    body_etag
        .or(header_etag)
        .filter(|etag| !etag.trim().is_empty())
        .ok_or(ClientError::MissingField { field: "etag" })
}

fn parse_json_text(text: &str) -> Result<Value, ClientError> {
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(text).map_err(|error| ClientError::InvalidJson {
        reason: error.to_string(),
    })
}

fn describe_error_body(body: &Value) -> String {
    if let Some(error) = body.get("error") {
        return error.to_string();
    }
    body.to_string()
}

fn read_required_string(value: &Value, field: &'static str) -> Result<String, ClientError> {
    value
        .get(field)
        .ok_or(ClientError::MissingField { field })?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(ClientError::InvalidField { field })
}

fn resolve_drive_relative_url(base_url: &str, path: &str) -> Result<String, ClientError> {
    if path.starts_with("http://") || path.starts_with("https://") {
        return Ok(path.to_owned());
    }
    let rpc_url = drive_rpc_url(base_url)?;
    let drive_root = rpc_url
        .trim_end_matches('/')
        .strip_suffix("/api/v1")
        .unwrap_or(rpc_url.trim_end_matches('/'));
    Ok(format!("{drive_root}{path}"))
}

fn fetch_oidc_discovery(http: &Client, issuer: &str) -> Result<OidcDiscovery, ClientError> {
    let issuer = trim_base_url(issuer)?;
    let response = http
        .get(format!("{issuer}/.well-known/openid-configuration"))
        .send()?;
    read_json_response(response)
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    token_endpoint: String,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /* REQ-DISKD-CLI-005: Drive JSON-RPC calls must use the apis-service /v1/os/drive/api/v1 URL. */
    #[test]
    fn builds_drive_rpc_url() {
        assert_eq!(
            drive_rpc_url("https://apis.example/").unwrap(),
            "https://apis.example/v1/os/drive/api/v1"
        );
    }

    /* REQ-DISKD-CLI-006: Project listing must use the apis-service platform projects route. */
    #[test]
    fn builds_projects_list_url() {
        assert_eq!(
            projects_list_url("https://apis.example/").unwrap(),
            "https://apis.example/v1/platform/projects/api/projects"
        );
    }

    /* REQ-DISKD-CLI-007: Vsearch must call the path-based Drive tool with limit/offset and normalized paths. */
    #[test]
    fn builds_vsearch_request_with_pagination() {
        let paths = vec!["/Projects/01PROJECT/docs".to_owned()];

        assert_eq!(
            vsearch_request("contract terms", &paths, Some(7), Some(14)),
            JsonRpcRequest {
                jsonrpc: "2.0",
                method: "paths/tools/vsearch",
                params: vec![
                    RpcParam {
                        name: "query".to_owned(),
                        value: RpcValue::String("contract terms".to_owned()),
                    },
                    RpcParam {
                        name: "paths".to_owned(),
                        value: RpcValue::StringList(paths),
                    },
                    RpcParam {
                        name: "limit".to_owned(),
                        value: RpcValue::U64(7),
                    },
                    RpcParam {
                        name: "offset".to_owned(),
                        value: RpcValue::U64(14),
                    },
                ],
            }
        );
    }

    /* REQ-DISKD-CLI-026: Grep must pass limit and offset through the path-based Drive search contract. */
    #[test]
    fn builds_grep_request_with_pagination() {
        let paths = vec!["/Projects/01PROJECT/docs".to_owned()];

        assert_eq!(
            grep_request("contract terms", &paths, Some(5), Some(10)),
            JsonRpcRequest {
                jsonrpc: "2.0",
                method: "paths/tools/grep",
                params: vec![
                    RpcParam {
                        name: "query".to_owned(),
                        value: RpcValue::String("contract terms".to_owned()),
                    },
                    RpcParam {
                        name: "paths".to_owned(),
                        value: RpcValue::StringList(paths),
                    },
                    RpcParam {
                        name: "limit".to_owned(),
                        value: RpcValue::U64(5),
                    },
                    RpcParam {
                        name: "offset".to_owned(),
                        value: RpcValue::U64(10),
                    },
                ],
            }
        );
    }

    /* REQ-DISKD-CLI-008: Biquery must call the path-based spreadsheet BI Drive tool, not raw drive/db/query. */
    #[test]
    fn builds_biquery_request() {
        let paths = vec!["/Projects/01PROJECT/sheet.xlsx".to_owned()];
        let request = biquery_request("what is the total amount?", &paths);

        assert_eq!(request.method, "paths/tools/bi-query");
        assert_eq!(
            request.params,
            vec![
                RpcParam {
                    name: "query".to_owned(),
                    value: RpcValue::String("what is the total amount?".to_owned()),
                },
                RpcParam {
                    name: "paths".to_owned(),
                    value: RpcValue::StringList(vec!["/Projects/01PROJECT/sheet.xlsx".to_owned()]),
                },
            ]
        );
    }

    /* REQ-DISKD-CLI-029: Telegram DB commands must use the dedicated Drive Telegram JSON-RPC namespace, not spreadsheet BI tools. */
    #[test]
    fn builds_telegram_db_requests() {
        let schema = json!({
            "items": ["CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, text TEXT)"]
        });
        let rows = json!([{ "id": 1, "text": "hello" }]);
        let parameters = json!(["hello"]);

        assert_eq!(
            telegram_db_create_request(
                "team-chat",
                Some(schema.clone()),
                None,
                Some(true),
                Some("/Telegram")
            ),
            JsonRpcRequest {
                jsonrpc: "2.0",
                method: "drive/telegram/create",
                params: vec![
                    RpcParam {
                        name: "name".to_owned(),
                        value: RpcValue::String("team-chat".to_owned()),
                    },
                    RpcParam {
                        name: "schema".to_owned(),
                        value: RpcValue::Json(schema),
                    },
                    RpcParam {
                        name: "recreate".to_owned(),
                        value: RpcValue::Bool(true),
                    },
                    RpcParam {
                        name: "directory".to_owned(),
                        value: RpcValue::String("/Telegram".to_owned()),
                    },
                ],
            }
        );

        assert_eq!(
            telegram_db_insert_request("team-chat", "messages", rows.clone()).params,
            vec![
                RpcParam {
                    name: "name".to_owned(),
                    value: RpcValue::String("team-chat".to_owned()),
                },
                RpcParam {
                    name: "table".to_owned(),
                    value: RpcValue::String("messages".to_owned()),
                },
                RpcParam {
                    name: "rows".to_owned(),
                    value: RpcValue::Json(rows),
                },
            ]
        );

        assert_eq!(
            telegram_db_query_request(
                "team-chat",
                "SELECT * FROM messages WHERE text = ?",
                Some(parameters.clone())
            ),
            JsonRpcRequest {
                jsonrpc: "2.0",
                method: "drive/telegram/query",
                params: vec![
                    RpcParam {
                        name: "name".to_owned(),
                        value: RpcValue::String("team-chat".to_owned()),
                    },
                    RpcParam {
                        name: "sql".to_owned(),
                        value: RpcValue::String("SELECT * FROM messages WHERE text = ?".to_owned()),
                    },
                    RpcParam {
                        name: "parameters".to_owned(),
                        value: RpcValue::Json(parameters),
                    },
                ],
            }
        );

        assert_eq!(
            telegram_db_commit_request("team-chat").method,
            "drive/telegram/commit"
        );
        assert_eq!(
            telegram_db_metadata_request("team-chat").method,
            "drive/telegram/metadata"
        );
        assert_eq!(
            telegram_db_drop_request("team-chat").method,
            "drive/telegram/drop"
        );
    }

    /* REQ-DISKD-CLI-009: Download URL requests must use the path-based file URL contract for cat. */
    #[test]
    fn builds_download_url_request() {
        let request = download_url_request("/Projects/01PROJECT/a.txt", Some(3));

        assert_eq!(request.method, "drive/files/download-url");
        assert_eq!(
            request.params,
            vec![
                RpcParam {
                    name: "path".to_owned(),
                    value: RpcValue::String("/Projects/01PROJECT/a.txt".to_owned()),
                },
                RpcParam {
                    name: "version".to_owned(),
                    value: RpcValue::U64(3),
                },
            ]
        );
    }

    /* REQ-DISKD-CLI-019: Stat must use the path-based inode-ls tool because deployed metadata RPC is inode-based. */
    #[test]
    fn builds_stat_request_with_path_tool() {
        let request = metadata_request("/Projects/01PROJECT/a.txt");

        assert_eq!(request.method, "paths/tools/inode-ls");
        assert_eq!(
            request.params,
            vec![RpcParam {
                name: "path".to_owned(),
                value: RpcValue::String("/Projects/01PROJECT/a.txt".to_owned()),
            }]
        );
    }

    /* REQ-DISKD-CLI-010: Upload must use start, PUT, then commit with intent_id and etag. */
    #[test]
    fn builds_upload_requests() {
        let start = upload_start_request(
            "a.txt",
            10,
            "abc123",
            Some("/Projects/01PROJECT"),
            Some("text/plain"),
            Some(true),
        );
        let commit = upload_commit_request("intent-1", "etag-1");

        assert_eq!(start.method, "drive/upload/start");
        assert_eq!(commit.method, "drive/upload/commit");
        assert_eq!(
            commit.params,
            vec![
                RpcParam {
                    name: "intent_id".to_owned(),
                    value: RpcValue::String("intent-1".to_owned()),
                },
                RpcParam {
                    name: "etag".to_owned(),
                    value: RpcValue::String("etag-1".to_owned()),
                },
            ]
        );
    }

    /* REQ-DISKD-CLI-011: Path management commands must map to Drive path mutation RPC methods. */
    #[test]
    fn builds_path_management_requests() {
        let create = path_create_request("docs", Some("/Projects/01PROJECT"), "dir");
        assert_eq!(create.method, "drive/paths/create");
        assert!(create.params.contains(&RpcParam {
            name: "dir_name".to_owned(),
            value: RpcValue::String("docs".to_owned()),
        }));
        assert_eq!(
            path_delete_request(&["/Projects/01PROJECT/docs".to_owned()], Some(true)).method,
            "drive/paths/delete"
        );
        assert_eq!(
            path_rename_request("/Projects/01PROJECT/a.txt", "b.txt", None).method,
            "drive/paths/rename"
        );
    }

    /* REQ-DISKD-CLI-014: JSON-RPC payloads must serialize params as an object, matching platform-api. */
    #[test]
    fn serializes_json_rpc_payload() {
        let payload = json_rpc_payload(
            &ls_request(Some("/Projects/01PROJECT"), Some(true), None, None),
            42,
        );

        assert_eq!(
            payload,
            json!({
                "jsonrpc": "2.0",
                "method": "paths/tools/ls",
                "params": {
                    "path": "/Projects/01PROJECT",
                    "recursive": true
                },
                "id": 42
            })
        );
    }
}
