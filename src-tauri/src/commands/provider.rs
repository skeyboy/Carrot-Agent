use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::application::CarrotService;
use crate::domain::provider::{ProviderCapabilities, ProviderProfile};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Type)]
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
            store_responses: profile.store_responses,
            capabilities: profile.capabilities.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfilesDto {
    pub config_path: String,
    pub profiles: Vec<ProviderProfileDto>,
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_list(
    service: State<'_, CarrotService>,
) -> Result<ProviderProfilesDto, AppError> {
    Ok(ProviderProfilesDto {
        config_path: service.provider_config_path(),
        profiles: service
            .provider_profiles()
            .await
            .into_iter()
            .map(ProviderProfileDto::from)
            .collect(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn provider_profile_reload(
    service: State<'_, CarrotService>,
) -> Result<ProviderProfilesDto, AppError> {
    let profiles = service.reload_provider_profiles().await?;
    Ok(ProviderProfilesDto {
        config_path: service.provider_config_path(),
        profiles: profiles.into_iter().map(ProviderProfileDto::from).collect(),
    })
}
