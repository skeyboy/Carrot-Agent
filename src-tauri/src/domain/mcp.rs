use serde::{Deserialize, Serialize};
use specta::Type;

use crate::tools::ToolRisk;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    #[default]
    Stdio,
    StreamableHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthKind {
    #[default]
    None,
    Bearer,
    Oauth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum McpPresetKind {
    WorkspaceFilesystem,
    BraveSearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpSystemSettings {
    pub controlled_local_tools: bool,
    pub remote_http: bool,
    pub secure_auth: bool,
    pub dynamic_updates: bool,
}

impl Default for McpSystemSettings {
    fn default() -> Self {
        Self {
            controlled_local_tools: true,
            remote_http: true,
            secure_auth: true,
            dynamic_updates: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolPolicy {
    pub name: String,
    pub enabled: bool,
    pub risk: ToolRisk,
    pub idempotent: bool,
    pub reconcile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerConfig {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    #[serde(default)]
    pub transport: McpTransportKind,
    #[serde(default)]
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub auth: McpAuthKind,
    #[serde(default)]
    pub oauth_client_id: Option<String>,
    #[serde(default)]
    pub oauth_scopes: Vec<String>,
    #[serde(default)]
    pub preset: Option<McpPresetKind>,
    #[serde(default)]
    pub secret_environment_variable: Option<String>,
    #[serde(default)]
    pub read_directories: Vec<String>,
    #[serde(default)]
    pub allowed_directories: Vec<String>,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default)]
    pub tool_policies: Vec<McpToolPolicy>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    pub server_id: String,
    pub remote_name: String,
    pub alias: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub schema_hash: String,
    pub read_only_hint: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum McpConnectionState {
    Disabled,
    Disconnected,
    Connecting,
    Ready,
    Degraded,
    Reconnecting,
    Failed,
}

impl McpConnectionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Reconnecting => "reconnecting",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpToolSummary {
    pub name: String,
    pub alias: String,
    pub title: Option<String>,
    pub description: String,
    pub schema_hash: String,
    pub read_only_hint: Option<bool>,
    pub enabled: bool,
    pub risk: ToolRisk,
    pub idempotent: bool,
    pub reconcile: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSummary {
    pub config: McpServerConfig,
    pub state: McpConnectionState,
    pub error: Option<String>,
    pub tools: Vec<McpToolSummary>,
    pub auth_configured: bool,
    pub catalog_revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogSnapshot {
    pub config_path: String,
    pub system: McpSystemSettings,
    pub servers: Vec<McpServerSummary>,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthStart {
    pub server_id: String,
    pub authorization_url: String,
}
