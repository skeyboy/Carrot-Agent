use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::application::CarrotService;
use crate::error::AppError;
use crate::providers::ProviderEvent;

pub const CHAT_EVENT_NAME: &str = "carrot://chat-event";

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatStartRequest {
    pub conversation_id: String,
    pub text: String,
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatStartResponse {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatEventDto {
    pub run_id: String,
    pub conversation_id: String,
    pub event: ProviderEvent,
}

#[tauri::command]
#[specta::specta]
pub async fn chat_start(
    app: AppHandle,
    _service: State<'_, CarrotService>,
    request: ChatStartRequest,
) -> Result<ChatStartResponse, AppError> {
    let run_id = uuid::Uuid::now_v7().to_string();
    let response = ChatStartResponse {
        run_id: run_id.clone(),
    };
    tauri::async_runtime::spawn(async move {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
        let event_app = app.clone();
        let event_run_id = run_id.clone();
        let conversation_id = request.conversation_id.clone();
        let forwarder = tauri::async_runtime::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let _ = event_app.emit(
                    CHAT_EVENT_NAME,
                    ChatEventDto {
                        run_id: event_run_id.clone(),
                        conversation_id: conversation_id.clone(),
                        event,
                    },
                );
            }
        });
        let service = app.state::<CarrotService>();
        if let Err(error) = service
            .stream_chat(
                run_id.clone(),
                request.conversation_id.clone(),
                request.text,
                request.attachment_ids,
                sender,
            )
            .await
        {
            let _ = app.emit(
                CHAT_EVENT_NAME,
                ChatEventDto {
                    run_id,
                    conversation_id: request.conversation_id,
                    event: ProviderEvent::Failed {
                        message: error.to_string(),
                    },
                },
            );
        }
        let _ = forwarder.await;
    });
    Ok(response)
}

#[tauri::command]
#[specta::specta]
pub async fn chat_cancel(
    service: State<'_, CarrotService>,
    run_id: String,
) -> Result<(), AppError> {
    service.cancel_chat(&run_id).await
}
