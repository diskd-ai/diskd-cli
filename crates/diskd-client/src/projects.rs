use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{projects_list_url, read_json_response, ClientError, GatewayClient};

/// Represents the canonical project value returned by platform-api 6.1.1.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    pub updated_at: String,
}

/// Carries project creation fields without exposing transport details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
}

/// Carries only the project fields intentionally changed by an update.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
}

impl GatewayClient {
    /// Lists projects visible to the current credential.
    pub fn list_projects(&self) -> Result<Vec<Project>, ClientError> {
        let url = projects_list_url(&self.base_url)?;
        let response = self.http.get(url).bearer_auth(&self.bearer_token).send()?;
        read_json_response(response)
    }

    /// Gets one project by canonical project id.
    pub fn get_project(&self, project_id: &str) -> Result<Project, ClientError> {
        let url = project_item_url(&self.base_url, project_id)?;
        let response = self.http.get(url).bearer_auth(&self.bearer_token).send()?;
        read_json_response(response)
    }

    /// Creates one project after validating the required name.
    pub fn create_project(&self, params: &ProjectCreateParams) -> Result<Project, ClientError> {
        validate_project_name(&params.name)?;
        let url = projects_list_url(&self.base_url)?;
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.bearer_token)
            .json(params)
            .send()?;
        read_json_response(response)
    }

    /// Updates one project by canonical project id.
    pub fn update_project(
        &self,
        project_id: &str,
        params: &ProjectUpdateParams,
    ) -> Result<Project, ClientError> {
        if let Some(name) = params.name.as_deref() {
            validate_project_name(name)?;
        }
        if params == &ProjectUpdateParams::default() {
            return Err(ClientError::InvalidInput {
                field: "project update",
                reason: "at least one field must be provided".to_owned(),
            });
        }
        let url = project_item_url(&self.base_url, project_id)?;
        let response = self
            .http
            .put(url)
            .bearer_auth(&self.bearer_token)
            .json(params)
            .send()?;
        read_json_response(response)
    }

    /// Deletes one project by canonical project id.
    pub fn delete_project(&self, project_id: &str) -> Result<(), ClientError> {
        let url = project_item_url(&self.base_url, project_id)?;
        let response = self
            .http
            .delete(url)
            .bearer_auth(&self.bearer_token)
            .send()?;
        let _: Value = read_json_response(response)?;
        Ok(())
    }
}

/// Builds an encoded project item URL for get, update, and delete.
pub fn project_item_url(base_url: &str, project_id: &str) -> Result<String, ClientError> {
    validate_project_id(project_id)?;
    Ok(format!(
        "{}/{}",
        projects_list_url(base_url)?,
        percent_encode_path_segment(project_id)
    ))
}

fn validate_project_id(project_id: &str) -> Result<(), ClientError> {
    if project_id.trim().is_empty() {
        return Err(ClientError::InvalidInput {
            field: "project_id",
            reason: "must not be empty".to_owned(),
        });
    }
    Ok(())
}

fn validate_project_name(name: &str) -> Result<(), ClientError> {
    if name.trim().is_empty() {
        return Err(ClientError::InvalidInput {
            field: "name",
            reason: "must not be empty".to_owned(),
        });
    }
    Ok(())
}

fn percent_encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /* REQ-DISKD-CLIENT-001: Project item URLs must encode canonical IDs as one REST path segment. */
    #[test]
    fn encodes_project_item_url() {
        assert_eq!(
            project_item_url("https://apis.example/", "project alpha/1").unwrap(),
            "https://apis.example/v1/platform/projects/api/projects/project%20alpha%2F1"
        );
    }

    /* REQ-DISKD-CLIENT-002: Project update payloads must use the SDK camelCase wire fields and omit absent fields. */
    #[test]
    fn serializes_project_update_fields() {
        let value = serde_json::to_value(ProjectUpdateParams {
            name: Some("Alpha".to_owned()),
            icon_color: Some("blue".to_owned()),
            ..ProjectUpdateParams::default()
        })
        .unwrap();

        assert_eq!(value, json!({ "name": "Alpha", "iconColor": "blue" }));
    }
}
