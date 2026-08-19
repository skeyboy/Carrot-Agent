use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::application::CarrotService;
use crate::domain::attachment::AttachmentDescriptor;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentDto {
    pub id: String,
    pub conversation_id: String,
    pub media_type: String,
    pub file_name: String,
    pub byte_length: String,
    pub content_hash: String,
    pub created_at_ms: String,
}

impl From<AttachmentDescriptor> for AttachmentDto {
    fn from(value: AttachmentDescriptor) -> Self {
        Self {
            id: value.id,
            conversation_id: value.conversation_id,
            media_type: value.media_type,
            file_name: value.file_name,
            byte_length: value.byte_length.to_string(),
            content_hash: value.content_hash,
            created_at_ms: value.created_at_ms.to_string(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn attachment_list(
    service: State<'_, CarrotService>,
    conversation_id: String,
) -> Result<Vec<AttachmentDto>, AppError> {
    Ok(service
        .list_attachments(&conversation_id)
        .await?
        .into_iter()
        .map(AttachmentDto::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn attachment_pick_and_import(
    app: AppHandle,
    service: State<'_, CarrotService>,
    conversation_id: String,
) -> Result<Option<AttachmentDto>, AppError> {
    let selected = app
        .dialog()
        .file()
        .set_title("Choose an image")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| AppError::invalid_input(error.to_string()))?;
    service
        .import_attachment(&conversation_id, path)
        .await
        .map(AttachmentDto::from)
        .map(Some)
}

#[tauri::command]
#[specta::specta]
pub async fn attachment_delete(
    service: State<'_, CarrotService>,
    id: String,
) -> Result<(), AppError> {
    service.delete_attachment(&id).await
}
