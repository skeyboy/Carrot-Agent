use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::application::CarrotService;
use crate::domain::settings::AppSettings;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshotDto {
    pub settings: AppSettings,
    pub settings_path: String,
    pub database_path: String,
    pub attachment_path: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusDto {
    pub provider_id: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetCredentialRequest {
    pub provider_id: String,
    pub secret: String,
}

#[tauri::command]
#[specta::specta]
pub async fn settings_get(
    service: State<'_, CarrotService>,
) -> Result<SettingsSnapshotDto, AppError> {
    Ok(SettingsSnapshotDto {
        settings: service.settings().await,
        settings_path: service.settings_path(),
        database_path: service.database_path(),
        attachment_path: service.attachment_path(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    service: State<'_, CarrotService>,
    request: UpdateSettingsRequest,
) -> Result<SettingsSnapshotDto, AppError> {
    service.update_settings(request.settings).await?;
    settings_get(service).await
}

#[tauri::command]
#[specta::specta]
pub async fn credential_status_list(
    service: State<'_, CarrotService>,
) -> Result<Vec<CredentialStatusDto>, AppError> {
    let mut statuses = Vec::new();
    for profile in service.provider_profiles().await {
        statuses.push(CredentialStatusDto {
            configured: service.credential_configured(&profile.id).await?,
            provider_id: profile.id,
        });
    }
    Ok(statuses)
}

#[tauri::command]
#[specta::specta]
pub async fn credential_set(
    service: State<'_, CarrotService>,
    request: SetCredentialRequest,
) -> Result<CredentialStatusDto, AppError> {
    service
        .set_credential(&request.provider_id, request.secret)
        .await?;
    Ok(CredentialStatusDto {
        provider_id: request.provider_id,
        configured: true,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn credential_delete(
    service: State<'_, CarrotService>,
    provider_id: String,
) -> Result<CredentialStatusDto, AppError> {
    service.delete_credential(&provider_id).await?;
    Ok(CredentialStatusDto {
        provider_id,
        configured: false,
    })
}
