use serde::Deserialize;
use specta::Type;
use tauri::State;

use crate::application::CarrotService;
use crate::domain::mcp::{
    McpCatalogSnapshot, McpOAuthStart, McpPresetKind, McpServerConfig, McpSystemSettings,
    McpToolPolicy,
};
use crate::error::AppError;

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRequest {
    pub config: McpServerConfig,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpPresetInstallRequest {
    pub preset: McpPresetKind,
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpSystemSettingsRequest {
    pub settings: McpSystemSettings,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpToolPolicyRequest {
    pub server_id: String,
    pub policy: McpToolPolicy,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpAuthRequest {
    pub server_id: String,
    pub secret: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthBeginRequest {
    pub server_id: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthCompleteRequest {
    pub server_id: String,
    pub callback_url: String,
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_catalog_get(
    service: State<'_, CarrotService>,
) -> Result<McpCatalogSnapshot, AppError> {
    Ok(service.mcp_snapshot().await)
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_system_settings_update(
    service: State<'_, CarrotService>,
    request: McpSystemSettingsRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service.update_mcp_system_settings(request.settings).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_preset_install(
    service: State<'_, CarrotService>,
    request: McpPresetInstallRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service
        .install_mcp_preset(request.preset, request.workspace_path)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_server_create(
    service: State<'_, CarrotService>,
    request: McpServerRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service.create_mcp_server(request.config).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_server_update(
    service: State<'_, CarrotService>,
    request: McpServerRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service.update_mcp_server(request.config).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_server_delete(
    service: State<'_, CarrotService>,
    server_id: String,
) -> Result<McpCatalogSnapshot, AppError> {
    service.delete_mcp_server(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_server_connect(
    service: State<'_, CarrotService>,
    server_id: String,
) -> Result<McpCatalogSnapshot, AppError> {
    service.connect_mcp_server(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_server_disconnect(
    service: State<'_, CarrotService>,
    server_id: String,
) -> Result<McpCatalogSnapshot, AppError> {
    service.disconnect_mcp_server(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_tool_policy_set(
    service: State<'_, CarrotService>,
    request: McpToolPolicyRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service
        .set_mcp_tool_policy(&request.server_id, request.policy)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_auth_set(
    service: State<'_, CarrotService>,
    request: McpAuthRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service
        .set_mcp_auth(&request.server_id, request.secret)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_auth_clear(
    service: State<'_, CarrotService>,
    server_id: String,
) -> Result<McpCatalogSnapshot, AppError> {
    service.clear_mcp_auth(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_server_refresh(
    service: State<'_, CarrotService>,
    server_id: String,
) -> Result<McpCatalogSnapshot, AppError> {
    service.refresh_mcp_server(&server_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_oauth_begin(
    service: State<'_, CarrotService>,
    request: McpOAuthBeginRequest,
) -> Result<McpOAuthStart, AppError> {
    service
        .begin_mcp_oauth(&request.server_id, request.redirect_uri)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_oauth_complete(
    service: State<'_, CarrotService>,
    request: McpOAuthCompleteRequest,
) -> Result<McpCatalogSnapshot, AppError> {
    service
        .complete_mcp_oauth(&request.server_id, request.callback_url)
        .await
}
