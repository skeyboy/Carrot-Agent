use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::application::CarrotService;
use crate::domain::conversation::{Conversation, ConversationChanges};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDto {
    pub id: String,
    pub title: String,
    pub default_provider_profile_id: String,
    pub default_model: String,
    pub version: i32,
    pub created_at_ms: String,
    pub updated_at_ms: String,
}

impl TryFrom<Conversation> for ConversationDto {
    type Error = AppError;

    fn try_from(conversation: Conversation) -> Result<Self, Self::Error> {
        Ok(Self {
            id: conversation.id,
            title: conversation.title,
            default_provider_profile_id: conversation.default_provider_profile_id,
            default_model: conversation.default_model,
            version: i32::try_from(conversation.version).map_err(|error| AppError::Storage {
                message: format!("conversation version cannot be represented by IPC: {error}"),
            })?,
            created_at_ms: conversation.created_at_ms.to_string(),
            updated_at_ms: conversation.updated_at_ms.to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationRequest {
    pub title: String,
    pub provider_profile_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConversationRequest {
    pub id: String,
    pub expected_version: i32,
    pub title: Option<String>,
    pub default_provider_profile_id: Option<String>,
    pub default_model: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConversationRequest {
    pub id: String,
    pub expected_version: i32,
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_list(
    service: State<'_, CarrotService>,
) -> Result<Vec<ConversationDto>, AppError> {
    service
        .list_conversations()
        .await
        .and_then(|items| items.into_iter().map(ConversationDto::try_from).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_get(
    id: String,
    service: State<'_, CarrotService>,
) -> Result<ConversationDto, AppError> {
    ConversationDto::try_from(service.get_conversation(&id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_create(
    request: CreateConversationRequest,
    service: State<'_, CarrotService>,
) -> Result<ConversationDto, AppError> {
    service
        .create_conversation(request.title, request.provider_profile_id, request.model)
        .await
        .and_then(ConversationDto::try_from)
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_update(
    request: UpdateConversationRequest,
    service: State<'_, CarrotService>,
) -> Result<ConversationDto, AppError> {
    service
        .update_conversation(
            &request.id,
            ConversationChanges {
                expected_version: i64::from(request.expected_version),
                title: request.title,
                default_provider_profile_id: request.default_provider_profile_id,
                default_model: request.default_model,
            },
        )
        .await
        .and_then(ConversationDto::try_from)
}

#[tauri::command]
#[specta::specta]
pub async fn conversation_delete(
    request: DeleteConversationRequest,
    service: State<'_, CarrotService>,
) -> Result<(), AppError> {
    service
        .delete_conversation(&request.id, i64::from(request.expected_version))
        .await
}
