use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::application::CarrotService;
use crate::domain::provider::{
    NewProviderProfile, ProviderCapabilities, ProviderCatalog, ProviderKind, ProviderProfile,
    ProviderProfileChanges, ProviderProtocol,
};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilitiesDto {
    pub tools: bool,
    pub images: bool,
    pub files: bool,
}

impl From<ProviderCapabilities> for ProviderCapabilitiesDto {
    fn from(capabilities: ProviderCapabilities) -> Self {
        Self {
            tools: capabilities.tools,
            images: capabilities.images,
            files: capabilities.files,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub protocol: String,
    pub base_url: String,
    pub default_model: String,
    pub available_models: Vec<String>,
    pub enabled_models: Vec<String>,
    pub models_synced_at_ms: Option<String>,
    pub store_responses: bool,
    pub capabilities: ProviderCapabilitiesDto,
}

impl From<ProviderProfile> for ProviderProfileDto {
    fn from(profile: ProviderProfile) -> Self {
        Self {
            id: profile.id,
            label: profile.label,
            kind: profile.kind.as_str().to_owned(),
            protocol: profile.protocol.as_str().to_owned(),
            base_url: profile.base_url,
            default_model: profile.default_model,
            available_models: profile.available_models,
            enabled_models: profile.enabled_models,
            models_synced_at_ms: profile.models_synced_at_ms.map(|value| value.to_string()),
            store_responses: profile.store_responses,
            capabilities: profile.capabilities.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfilesDto {
    pub config_path: String,
    pub default_provider_id: String,
    pub profiles: Vec<ProviderProfileDto>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderProfileRequest {
    pub id: String,
    pub label: String,
    pub kind: ProviderKind,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub default_model: String,
    pub store_responses: bool,
    pub capabilities: ProviderCapabilitiesDto,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderProfileRequest {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub default_model: String,
    pub enabled_models: Vec<String>,
    pub store_responses: bool,
    pub capabilities: ProviderCapabilitiesDto,
}

impl From<ProviderCapabilitiesDto> for ProviderCapabilities {
    fn from(capabilities: ProviderCapabilitiesDto) -> Self {
        Self {
            tools: capabilities.tools,
            images: capabilities.images,
            files: capabilities.files,
        }
    }
}

fn snapshot(config_path: String, catalog: ProviderCatalog) -> ProviderProfilesDto {
    ProviderProfilesDto {
        config_path,
        default_provider_id: catalog.default_provider_id,
        profiles: catalog
            .profiles
            .into_iter()
            .map(ProviderProfileDto::from)
            .collect(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_list(
    service: State<'_, CarrotService>,
) -> Result<ProviderProfilesDto, AppError> {
    Ok(snapshot(
        service.provider_config_path(),
        service.provider_catalog().await,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_reload(
    service: State<'_, CarrotService>,
) -> Result<ProviderProfilesDto, AppError> {
    let catalog = service.reload_provider_profiles().await?;
    Ok(snapshot(service.provider_config_path(), catalog))
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_create(
    service: State<'_, CarrotService>,
    request: CreateProviderProfileRequest,
) -> Result<ProviderProfilesDto, AppError> {
    let catalog = service
        .create_provider_profile(NewProviderProfile {
            id: request.id,
            label: request.label,
            kind: request.kind,
            protocol: request.protocol,
            base_url: request.base_url,
            default_model: request.default_model,
            store_responses: request.store_responses,
            capabilities: request.capabilities.into(),
        })
        .await?;
    Ok(snapshot(service.provider_config_path(), catalog))
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_update(
    service: State<'_, CarrotService>,
    request: UpdateProviderProfileRequest,
) -> Result<ProviderProfilesDto, AppError> {
    let catalog = service
        .update_provider_profile(ProviderProfileChanges {
            id: request.id,
            label: request.label,
            base_url: request.base_url,
            default_model: request.default_model,
            enabled_models: request.enabled_models,
            store_responses: request.store_responses,
            capabilities: request.capabilities.into(),
        })
        .await?;
    Ok(snapshot(service.provider_config_path(), catalog))
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_delete(
    service: State<'_, CarrotService>,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let catalog = service.delete_provider_profile(&provider_id).await?;
    Ok(snapshot(service.provider_config_path(), catalog))
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_set_default(
    service: State<'_, CarrotService>,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let catalog = service.set_default_provider(&provider_id).await?;
    Ok(snapshot(service.provider_config_path(), catalog))
}

#[tauri::command]
#[specta::specta]
pub async fn provider_model_sync(
    service: State<'_, CarrotService>,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let catalog = service.sync_provider_models(&provider_id).await?;
    Ok(snapshot(service.provider_config_path(), catalog))
}
