use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

use super::{
    param_json, param_string, param_string_list, param_u64, request, ClientError, GatewayClient,
    JsonRpcRequest,
};

/// Describes the session model/provider configuration persisted by Drive.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    #[serde(default, alias = "operative_id")]
    pub operative_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, alias = "prompt_text")]
    pub prompt_text: Option<String>,
    #[serde(
        alias = "drive_sources_muted",
        deserialize_with = "deserialize_bool_like"
    )]
    pub drive_sources_muted: bool,
}

/// Represents one exchange in a persisted session document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExchange {
    pub id: String,
    pub kind: String,
    pub metadata: Value,
    #[serde(alias = "created_at")]
    pub created_at: String,
}

/// Represents one participant in a persisted session document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionParticipant {
    #[serde(alias = "exchange_id")]
    pub exchange_id: String,
    #[serde(alias = "participant_kind")]
    pub participant_kind: String,
    #[serde(alias = "participant_id")]
    pub participant_id: String,
    #[serde(alias = "joined_at")]
    pub joined_at: String,
    #[serde(default, alias = "left_at")]
    pub left_at: Option<String>,
}

/// Represents one typed message in a persisted Drive session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    #[serde(alias = "participant_kind")]
    pub participant_kind: String,
    #[serde(default, alias = "participant_id")]
    pub participant_id: Option<String>,
    #[serde(default, alias = "participant_name")]
    pub participant_name: Option<String>,
    #[serde(default, alias = "participant_slug")]
    pub participant_slug: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, alias = "content_blocks_json")]
    pub content_blocks_json: Option<String>,
    #[serde(default, alias = "source_origin")]
    pub source_origin: Option<String>,
    #[serde(default, alias = "turn_correlation_id")]
    pub turn_correlation_id: Option<String>,
    #[serde(default, alias = "turn_context_json")]
    pub turn_context_json: Option<String>,
    #[serde(default, alias = "function_call")]
    pub function_call: Option<Value>,
    #[serde(default, alias = "tool_calls")]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(default, alias = "tool_call_id")]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub context: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub attachments: Option<Vec<String>>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default, alias = "parent_message_id")]
    pub parent_message_id: Option<String>,
    #[serde(alias = "is_sidechain", deserialize_with = "deserialize_bool_like")]
    pub is_sidechain: bool,
    #[serde(default, alias = "token_count")]
    pub token_count: Option<u64>,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(default, alias = "updated_at")]
    pub updated_at: Option<String>,
    #[serde(default, alias = "deleted_at")]
    pub deleted_at: Option<String>,
}

/// Represents the canonical project-scoped session document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDocument {
    pub id: String,
    #[serde(alias = "workspace_id")]
    pub workspace_id: String,
    #[serde(alias = "project_id")]
    pub project_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub config: SessionConfig,
    #[serde(default)]
    pub exchanges: Vec<SessionExchange>,
    #[serde(default)]
    pub participants: Vec<SessionParticipant>,
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
    #[serde(default, alias = "source_origin")]
    pub source_origin: Option<String>,
    #[serde(default, alias = "fork_source_session_id")]
    pub fork_source_session_id: Option<String>,
    #[serde(default, alias = "fork_source_message_id")]
    pub fork_source_message_id: Option<String>,
}

/// Represents a compact list item returned by drive/session/list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
    #[serde(alias = "session_id")]
    pub session_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(alias = "message_count")]
    pub message_count: u64,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Wraps the list result so the response stays aligned with platform-api.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SessionListResult {
    pub items: Vec<SessionListItem>,
}

/// Wraps a complete session read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SessionGetResult {
    pub session: SessionDocument,
}

/// Wraps a session preview and its total message count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGetPreviewResult {
    pub session: SessionDocument,
    pub messages: Vec<SessionMessage>,
    #[serde(alias = "message_count")]
    pub message_count: u64,
}

/// Wraps a backward-paginated range of session messages.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGetMessageRangeResult {
    pub messages: Vec<SessionMessage>,
    #[serde(alias = "has_more")]
    pub has_more: bool,
}

/// Represents the terminal result of save and append operations.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSaveResult {
    #[serde(alias = "session_id")]
    pub session_id: String,
    #[serde(alias = "message_count")]
    pub message_count: u64,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// Uses the same terminal shape for append while preserving operation intent.
pub type SessionAppendMessagesResult = SessionSaveResult;

/// Represents the terminal result of message deletion or rollback.
pub type SessionDeleteMessagesResult = SessionSaveResult;

/// Represents the terminal result of deleting a session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeleteResult {
    #[serde(alias = "session_id")]
    pub session_id: String,
    pub status: String,
}

/// Makes the two mutually exclusive delete-messages wire variants unrepresentable together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionDeleteMessages {
    ByIds(Vec<String>),
    RollbackAfter(String),
}

impl GatewayClient {
    /// Calls the project-scoped Drive Session JSON-RPC endpoint.
    pub fn call_session(&mut self, request: &JsonRpcRequest) -> Result<Value, ClientError> {
        let url = session_rpc_url(&self.base_url)?;
        self.call_json_rpc(&url, request)
    }

    /// Saves a complete typed session document.
    pub fn save_session(
        &mut self,
        project_id: &str,
        document: &SessionDocument,
        attributes: &[String],
    ) -> Result<SessionSaveResult, ClientError> {
        validate_session_document_input(project_id, document)?;
        let value = self.call_session(&session_save_request(project_id, document, attributes)?)?;
        decode_and_validate(value, validate_save_result)
    }

    /// Gets one complete session document.
    pub fn get_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<SessionGetResult, ClientError> {
        let value = self.call_session(&session_get_request(project_id, session_id)?)?;
        decode_and_validate(value, validate_get_result)
    }

    /// Gets one session document plus its newest messages.
    pub fn get_session_preview(
        &mut self,
        project_id: &str,
        session_id: &str,
        limit: u64,
    ) -> Result<SessionGetPreviewResult, ClientError> {
        let value =
            self.call_session(&session_get_preview_request(project_id, session_id, limit)?)?;
        decode_and_validate(value, validate_preview_result)
    }

    /// Gets a backward-paginated range of session messages.
    pub fn get_session_message_range(
        &mut self,
        project_id: &str,
        session_id: &str,
        limit: u64,
        before: Option<&str>,
    ) -> Result<SessionGetMessageRangeResult, ClientError> {
        let value = self.call_session(&session_get_message_range_request(
            project_id, session_id, limit, before,
        )?)?;
        decode_and_validate(value, validate_message_range_result)
    }

    /// Lists sessions under one project scope.
    pub fn list_sessions(&mut self, project_id: &str) -> Result<SessionListResult, ClientError> {
        let value = self.call_session(&session_list_request(project_id)?)?;
        decode_and_validate(value, validate_list_result)
    }

    /// Appends typed messages to one session.
    pub fn append_session_messages(
        &mut self,
        project_id: &str,
        session_id: &str,
        messages: &[SessionMessage],
    ) -> Result<SessionAppendMessagesResult, ClientError> {
        for message in messages {
            validate_message(message).map_err(input_error)?;
        }
        let value = self.call_session(&session_append_messages_request(
            project_id, session_id, messages,
        )?)?;
        decode_and_validate(value, validate_save_result)
    }

    /// Removes messages using exactly one typed deletion variant.
    pub fn delete_session_messages(
        &mut self,
        project_id: &str,
        session_id: &str,
        deletion: &SessionDeleteMessages,
    ) -> Result<SessionDeleteMessagesResult, ClientError> {
        let value = self.call_session(&session_delete_messages_request(
            project_id, session_id, deletion,
        )?)?;
        decode_and_validate(value, validate_save_result)
    }

    /// Deletes one session by canonical session id.
    pub fn delete_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<SessionDeleteResult, ClientError> {
        let value = self.call_session(&session_delete_request(project_id, session_id)?)?;
        decode_and_validate(value, validate_delete_result)
    }
}

/// Builds the public APIS endpoint for project-scoped Drive Sessions.
pub fn session_rpc_url(base_url: &str) -> Result<String, ClientError> {
    let base = super::trim_base_url(base_url)?;
    Ok(format!("{base}/v1/platform/sessions/api/v1"))
}

/// Builds a save request while deriving Drive root_path from the project id.
pub fn session_save_request(
    project_id: &str,
    document: &SessionDocument,
    attributes: &[String],
) -> Result<JsonRpcRequest, ClientError> {
    validate_session_document_input(project_id, document)?;
    let mut params = vec![
        param_string("root_path", &project_root_path(project_id)?),
        param_json("session", encode_session_document(document)),
    ];
    if !attributes.is_empty() {
        params.push(param_string_list("attributes", attributes.to_vec()));
    }
    Ok(request("drive/session/save", params))
}

/// Builds a complete session read request.
pub fn session_get_request(
    project_id: &str,
    session_id: &str,
) -> Result<JsonRpcRequest, ClientError> {
    Ok(request(
        "drive/session/get",
        vec![
            param_string("root_path", &project_root_path(project_id)?),
            param_string("session_id", validate_session_id(session_id)?),
        ],
    ))
}

/// Builds a newest-message session preview request.
pub fn session_get_preview_request(
    project_id: &str,
    session_id: &str,
    limit: u64,
) -> Result<JsonRpcRequest, ClientError> {
    validate_positive_limit(limit)?;
    Ok(request(
        "drive/session/get-preview",
        vec![
            param_string("root_path", &project_root_path(project_id)?),
            param_string("session_id", validate_session_id(session_id)?),
            param_u64("limit", limit),
        ],
    ))
}

/// Builds a backward-paginated session message range request.
pub fn session_get_message_range_request(
    project_id: &str,
    session_id: &str,
    limit: u64,
    before: Option<&str>,
) -> Result<JsonRpcRequest, ClientError> {
    validate_positive_limit(limit)?;
    let mut params = vec![
        param_string("root_path", &project_root_path(project_id)?),
        param_string("session_id", validate_session_id(session_id)?),
        param_u64("limit", limit),
    ];
    if let Some(before) = before {
        params.push(param_string("before", validate_message_id(before)?));
    }
    Ok(request("drive/session/get-message-range", params))
}

/// Builds a project-scoped session list request.
pub fn session_list_request(project_id: &str) -> Result<JsonRpcRequest, ClientError> {
    Ok(request(
        "drive/session/list",
        vec![param_string("root_path", &project_root_path(project_id)?)],
    ))
}

/// Builds an append request from typed messages.
pub fn session_append_messages_request(
    project_id: &str,
    session_id: &str,
    messages: &[SessionMessage],
) -> Result<JsonRpcRequest, ClientError> {
    if messages.is_empty() {
        return Err(ClientError::InvalidInput {
            field: "messages",
            reason: "must contain at least one message".to_owned(),
        });
    }
    for message in messages {
        validate_message(message).map_err(input_error)?;
    }
    Ok(request(
        "drive/session/append-messages",
        vec![
            param_string("root_path", &project_root_path(project_id)?),
            param_string("session_id", validate_session_id(session_id)?),
            param_json(
                "messages",
                Value::Array(messages.iter().map(encode_session_message).collect()),
            ),
        ],
    ))
}

/// Builds one of the two mutually exclusive delete-messages requests.
pub fn session_delete_messages_request(
    project_id: &str,
    session_id: &str,
    deletion: &SessionDeleteMessages,
) -> Result<JsonRpcRequest, ClientError> {
    let mut params = vec![
        param_string("root_path", &project_root_path(project_id)?),
        param_string("session_id", validate_session_id(session_id)?),
    ];
    match deletion {
        SessionDeleteMessages::ByIds(message_ids) => {
            if message_ids.is_empty() {
                return Err(ClientError::InvalidInput {
                    field: "message_ids",
                    reason: "must contain at least one message id".to_owned(),
                });
            }
            for message_id in message_ids {
                validate_message_id(message_id)?;
            }
            params.push(param_string_list("message_ids", message_ids.clone()));
        }
        SessionDeleteMessages::RollbackAfter(message_id) => {
            params.push(param_string(
                "rollback_after_message_id",
                validate_message_id(message_id)?,
            ));
        }
    }
    Ok(request("drive/session/delete-messages", params))
}

/// Builds a canonical session delete request.
pub fn session_delete_request(
    project_id: &str,
    session_id: &str,
) -> Result<JsonRpcRequest, ClientError> {
    Ok(request(
        "drive/session/delete",
        vec![
            param_string("root_path", &project_root_path(project_id)?),
            param_string("session_id", validate_session_id(session_id)?),
        ],
    ))
}

fn project_root_path(project_id: &str) -> Result<String, ClientError> {
    let project_id = validate_domain_id("project_id", project_id)?;
    Ok(format!("/Projects/{project_id}"))
}

fn validate_session_id(session_id: &str) -> Result<&str, ClientError> {
    validate_domain_id("session_id", session_id)
}

fn validate_message_id(message_id: &str) -> Result<&str, ClientError> {
    validate_domain_id("message_id", message_id)
}

fn validate_domain_id<'a>(field: &'static str, value: &'a str) -> Result<&'a str, ClientError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ClientError::InvalidInput {
            field,
            reason: "must not be empty".to_owned(),
        });
    }
    if value.contains('/') || value.contains('\\') {
        return Err(ClientError::InvalidInput {
            field,
            reason: "must be a domain id, not a storage path".to_owned(),
        });
    }
    Ok(value)
}

fn validate_positive_limit(limit: u64) -> Result<(), ClientError> {
    if limit == 0 {
        return Err(ClientError::InvalidInput {
            field: "limit",
            reason: "must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn decode_and_validate<T>(
    value: Value,
    validate: fn(&T) -> Result<(), String>,
) -> Result<T, ClientError>
where
    T: for<'de> Deserialize<'de>,
{
    let decoded = serde_json::from_value(value).map_err(|error| ClientError::InvalidJson {
        reason: error.to_string(),
    })?;
    validate(&decoded).map_err(|reason| ClientError::InvalidJson { reason })?;
    Ok(decoded)
}

fn validate_session_document_input(
    project_id: &str,
    document: &SessionDocument,
) -> Result<(), ClientError> {
    project_root_path(project_id)?;
    validate_session_document(document).map_err(input_error)?;
    if document.project_id != project_id {
        return Err(ClientError::InvalidInput {
            field: "session.projectId",
            reason: "must match the selected project context".to_owned(),
        });
    }
    Ok(())
}

fn input_error(reason: String) -> ClientError {
    ClientError::InvalidInput {
        field: "session",
        reason,
    }
}

fn validate_get_result(result: &SessionGetResult) -> Result<(), String> {
    validate_session_document(&result.session)
}

fn validate_preview_result(result: &SessionGetPreviewResult) -> Result<(), String> {
    validate_session_document(&result.session)?;
    for message in &result.messages {
        validate_message(message)?;
    }
    Ok(())
}

fn validate_message_range_result(result: &SessionGetMessageRangeResult) -> Result<(), String> {
    for message in &result.messages {
        validate_message(message)?;
    }
    Ok(())
}

fn validate_list_result(result: &SessionListResult) -> Result<(), String> {
    for item in &result.items {
        require_non_empty("session_id", &item.session_id)?;
        require_non_empty("updated_at", &item.updated_at)?;
    }
    Ok(())
}

fn validate_save_result(result: &SessionSaveResult) -> Result<(), String> {
    require_non_empty("session_id", &result.session_id)?;
    require_non_empty("updated_at", &result.updated_at)
}

fn validate_delete_result(result: &SessionDeleteResult) -> Result<(), String> {
    require_non_empty("session_id", &result.session_id)?;
    require_non_empty("status", &result.status)
}

fn validate_session_document(document: &SessionDocument) -> Result<(), String> {
    require_non_empty("id", &document.id)?;
    require_non_empty("workspace_id", &document.workspace_id)?;
    require_non_empty("project_id", &document.project_id)?;
    require_non_empty("created_at", &document.created_at)?;
    require_non_empty("updated_at", &document.updated_at)?;
    for exchange in &document.exchanges {
        require_non_empty("exchange.id", &exchange.id)?;
        require_non_empty("exchange.kind", &exchange.kind)?;
        require_non_empty("exchange.created_at", &exchange.created_at)?;
        require_object("exchange.metadata", &exchange.metadata)?;
    }
    for participant in &document.participants {
        require_non_empty("participant.exchange_id", &participant.exchange_id)?;
        require_non_empty(
            "participant.participant_kind",
            &participant.participant_kind,
        )?;
        require_non_empty("participant.participant_id", &participant.participant_id)?;
        require_non_empty("participant.joined_at", &participant.joined_at)?;
    }
    for message in &document.messages {
        validate_message(message)?;
    }
    Ok(())
}

fn validate_message(message: &SessionMessage) -> Result<(), String> {
    require_non_empty("message.id", &message.id)?;
    require_non_empty("message.role", &message.role)?;
    require_non_empty("message.participant_kind", &message.participant_kind)?;
    require_non_empty("message.created_at", &message.created_at)?;
    for (field, value) in [
        ("message.function_call", &message.function_call),
        ("message.context", &message.context),
        ("message.metadata", &message.metadata),
    ] {
        if let Some(value) = value {
            require_object(field, value)?;
        }
    }
    if let Some(tool_calls) = &message.tool_calls {
        for tool_call in tool_calls {
            require_object("message.tool_calls", tool_call)?;
        }
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!(
            "Invalid Drive Session payload: '{field}' must be a non-empty string"
        ));
    }
    Ok(())
}

fn require_object(field: &str, value: &Value) -> Result<(), String> {
    if !value.is_object() {
        return Err(format!(
            "Invalid Drive Session payload: '{field}' must be an object"
        ));
    }
    Ok(())
}

fn encode_session_document(document: &SessionDocument) -> Value {
    json!({
        "id": document.id,
        "workspace_id": document.workspace_id,
        "project_id": document.project_id,
        "title": document.title,
        "config": {
            "operative_id": document.config.operative_id,
            "provider": document.config.provider,
            "model": document.config.model,
            "prompt_text": document.config.prompt_text,
            "drive_sources_muted": document.config.drive_sources_muted,
        },
        "exchanges": document.exchanges.iter().map(|exchange| json!({
            "id": exchange.id,
            "kind": exchange.kind,
            "metadata": exchange.metadata,
            "created_at": exchange.created_at,
        })).collect::<Vec<_>>(),
        "participants": document.participants.iter().map(|participant| json!({
            "exchange_id": participant.exchange_id,
            "participant_kind": participant.participant_kind,
            "participant_id": participant.participant_id,
            "joined_at": participant.joined_at,
            "left_at": participant.left_at,
        })).collect::<Vec<_>>(),
        "messages": document.messages.iter().map(encode_session_message).collect::<Vec<_>>(),
        "created_at": document.created_at,
        "updated_at": document.updated_at,
        "source_origin": document.source_origin,
        "fork_source_session_id": document.fork_source_session_id,
        "fork_source_message_id": document.fork_source_message_id,
    })
}

fn encode_session_message(message: &SessionMessage) -> Value {
    json!({
        "id": message.id,
        "role": message.role,
        "participant_kind": message.participant_kind,
        "participant_id": message.participant_id,
        "participant_name": message.participant_name,
        "participant_slug": message.participant_slug,
        "content": message.content,
        "content_blocks_json": message.content_blocks_json,
        "source_origin": message.source_origin,
        "turn_correlation_id": message.turn_correlation_id,
        "turn_context_json": message.turn_context_json,
        "function_call": message.function_call,
        "tool_calls": message.tool_calls,
        "tool_call_id": message.tool_call_id,
        "context": message.context,
        "metadata": message.metadata,
        "attachments": message.attachments,
        "subtype": message.subtype,
        "parent_message_id": message.parent_message_id,
        "is_sidechain": message.is_sidechain,
        "token_count": message.token_count,
        "created_at": message.created_at,
        "updated_at": message.updated_at,
        "deleted_at": message.deleted_at,
    })
}

fn deserialize_bool_like<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Bool(value) => Ok(value),
        Value::Number(value) if value.as_u64() == Some(0) => Ok(false),
        Value::Number(value) if value.as_u64() == Some(1) => Ok(true),
        _ => Err(serde::de::Error::custom("expected boolean or 0/1")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_rpc_payload;

    fn message() -> SessionMessage {
        SessionMessage {
            id: "msg-1".to_owned(),
            role: "user".to_owned(),
            participant_kind: "user".to_owned(),
            participant_id: None,
            participant_name: None,
            participant_slug: None,
            content: Some("hello".to_owned()),
            content_blocks_json: None,
            source_origin: None,
            turn_correlation_id: None,
            turn_context_json: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            context: None,
            metadata: None,
            attachments: None,
            subtype: None,
            parent_message_id: None,
            is_sidechain: false,
            token_count: None,
            created_at: "2026-08-11T10:00:00Z".to_owned(),
            updated_at: None,
            deleted_at: None,
        }
    }

    /* REQ-DISKD-CLIENT-003: Session list requests must derive Drive root_path inside the adapter. */
    #[test]
    fn derives_project_root_for_session_list() {
        let payload = json_rpc_payload(&session_list_request("project-1").unwrap(), 1);
        assert_eq!(payload["method"], "drive/session/list");
        assert_eq!(
            payload["params"],
            json!({ "root_path": "/Projects/project-1" })
        );
    }

    /* REQ-DISKD-CLIENT-004: Session message deletion variants must serialize to mutually exclusive fields. */
    #[test]
    fn serializes_delete_message_variants() {
        let by_ids = json_rpc_payload(
            &session_delete_messages_request(
                "project-1",
                "session-1",
                &SessionDeleteMessages::ByIds(vec!["message-1".to_owned()]),
            )
            .unwrap(),
            1,
        );
        assert_eq!(by_ids["params"]["message_ids"], json!(["message-1"]));
        assert!(by_ids["params"].get("rollback_after_message_id").is_none());

        let rollback = json_rpc_payload(
            &session_delete_messages_request(
                "project-1",
                "session-1",
                &SessionDeleteMessages::RollbackAfter("message-1".to_owned()),
            )
            .unwrap(),
            2,
        );
        assert_eq!(rollback["params"]["rollback_after_message_id"], "message-1");
        assert!(rollback["params"].get("message_ids").is_none());
    }

    /* REQ-DISKD-CLIENT-005: Session append requests must translate camelCase domain messages to the snake_case Drive wire contract. */
    #[test]
    fn serializes_session_message_wire_fields() {
        let payload = json_rpc_payload(
            &session_append_messages_request("project-1", "session-1", &[message()]).unwrap(),
            1,
        );
        assert_eq!(payload["params"]["messages"][0]["participant_kind"], "user");
        assert_eq!(payload["params"]["messages"][0]["is_sidechain"], false);
        assert!(payload["params"]["messages"][0]
            .get("participantKind")
            .is_none());
    }

    /* REQ-DISKD-CLIENT-006: Session response decoding must accept the SDK boolean compatibility values and emit camelCase JSON. */
    #[test]
    fn decodes_session_message_boolean_compatibility() {
        let mut value = serde_json::to_value(message()).unwrap();
        value["isSidechain"] = json!(1);
        let decoded: SessionMessage = serde_json::from_value(value).unwrap();
        assert!(decoded.is_sidechain);
        assert_eq!(serde_json::to_value(decoded).unwrap()["isSidechain"], true);
    }

    /* REQ-DISKD-CLIENT-007: Session public IDs must reject storage-path input. */
    #[test]
    fn rejects_storage_paths_as_session_ids() {
        assert!(session_get_request("project-1", ".sessions/session-1").is_err());
        assert!(session_list_request("folder/project-1").is_err());
    }
}
