use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::application::CarrotService;
use crate::domain::run::{
    ChatSnapshot, PendingInputIntent, RecoveryResolution, RunPhase, RunStatus,
};
use crate::error::AppError;
use crate::providers::ProviderEvent;

pub const CHAT_EVENT_NAME: &str = "carrot://chat-event";

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatStartRequest {
    pub conversation_id: String,
    pub text: String,
    pub attachment_ids: Vec<String>,
    pub replaces_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatStartResponse {
    pub run_id: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatResumeRequest {
    pub run_id: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatInputRequest {
    pub run_id: String,
    pub intent: PendingInputIntent,
    pub text: String,
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatBranchRequest {
    pub pending_input_id: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolApprovalRequest {
    pub run_id: String,
    pub conversation_id: String,
    pub tool_execution_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecoveryRequest {
    pub run_id: String,
    pub tool_execution_id: String,
    pub resolution: RecoveryResolution,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatEventDto {
    pub run_id: String,
    pub conversation_id: String,
    pub event: ProviderEvent,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatSnapshotDto {
    pub conversation_id: String,
    pub active_run: Option<ActiveRunDto>,
    pub items: Vec<RunItemDto>,
    pub events: Vec<RunEventDto>,
    pub tool_executions: Vec<ToolExecutionDto>,
    pub pending_inputs: Vec<PendingInputDto>,
    pub approvals: Vec<ToolApprovalDto>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunDto {
    pub id: String,
    pub status: RunStatus,
    pub phase: RunPhase,
    pub last_event_seq: String,
    pub stop_reason: Option<String>,
    pub can_resume: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingInputDto {
    pub id: String,
    pub run_id: String,
    pub intent: PendingInputIntent,
    pub status: String,
    pub text: String,
    pub has_attachments: bool,
    pub child_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolApprovalDto {
    pub id: String,
    pub run_id: String,
    pub tool_execution_id: String,
    pub status: String,
    pub requested_at_ms: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunItemDto {
    pub id: String,
    pub run_id: String,
    pub seq: String,
    pub kind: String,
    pub role: Option<String>,
    pub content_json: String,
    pub call_id: Option<String>,
    pub created_at_ms: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunEventDto {
    pub run_id: String,
    pub seq: String,
    pub kind: String,
    pub payload_json: String,
    pub persisted_at_ms: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionDto {
    pub id: String,
    pub run_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub status: String,
    pub risk: String,
    pub arguments_json: String,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub idempotency_key: Option<String>,
    pub reconciliation_status: String,
    pub reconciliation_note: Option<String>,
}

impl From<ChatSnapshot> for ChatSnapshotDto {
    fn from(snapshot: ChatSnapshot) -> Self {
        Self {
            conversation_id: snapshot.conversation_id,
            active_run: snapshot.active_run.map(|run| {
                let can_resume = matches!(
                    run.status,
                    RunStatus::Paused | RunStatus::Suspended | RunStatus::Interrupted
                );
                ActiveRunDto {
                    id: run.id,
                    status: run.status,
                    phase: run.phase,
                    last_event_seq: run.last_event_seq.to_string(),
                    stop_reason: run.stop_reason,
                    can_resume,
                }
            }),
            items: snapshot
                .items
                .into_iter()
                .map(|item| RunItemDto {
                    id: item.id,
                    run_id: item.run_id,
                    seq: item.seq.to_string(),
                    kind: item.kind,
                    role: item.role,
                    content_json: item.content.to_string(),
                    call_id: item.call_id,
                    created_at_ms: item.created_at_ms.to_string(),
                })
                .collect(),
            events: snapshot
                .events
                .into_iter()
                .map(|event| RunEventDto {
                    run_id: event.run_id,
                    seq: event.seq.to_string(),
                    kind: event.kind,
                    payload_json: event.payload.to_string(),
                    persisted_at_ms: event.persisted_at_ms.to_string(),
                })
                .collect(),
            tool_executions: snapshot
                .tool_executions
                .into_iter()
                .map(|tool| ToolExecutionDto {
                    id: tool.id,
                    run_id: tool.run_id,
                    call_id: tool.call_id,
                    tool_name: tool.tool_name,
                    status: tool.status,
                    risk: tool.risk,
                    arguments_json: tool.arguments.to_string(),
                    output_json: tool.output.map(|output| output.to_string()),
                    error_message: tool.error_message,
                    idempotency_key: tool.idempotency_key,
                    reconciliation_status: tool.reconciliation_status,
                    reconciliation_note: tool.reconciliation_note,
                })
                .collect(),
            pending_inputs: snapshot
                .pending_inputs
                .into_iter()
                .map(|input| PendingInputDto {
                    id: input.id,
                    run_id: input.run_id,
                    intent: input.intent,
                    status: input.status,
                    text: message_text(&input.content),
                    has_attachments: message_has_attachments(&input.content),
                    child_run_id: input.child_run_id,
                })
                .collect(),
            approvals: snapshot
                .approvals
                .into_iter()
                .map(|approval| ToolApprovalDto {
                    id: approval.id,
                    run_id: approval.run_id,
                    tool_execution_id: approval.tool_execution_id,
                    status: approval.status,
                    requested_at_ms: approval.requested_at_ms.to_string(),
                })
                .collect(),
        }
    }
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
                request.replaces_run_id,
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

#[tauri::command]
#[specta::specta]
pub async fn chat_pause(service: State<'_, CarrotService>, run_id: String) -> Result<(), AppError> {
    service.pause_chat(&run_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_input(
    service: State<'_, CarrotService>,
    request: ChatInputRequest,
) -> Result<PendingInputDto, AppError> {
    service
        .enqueue_chat_input(
            &request.run_id,
            request.intent,
            request.text,
            request.attachment_ids,
        )
        .await
        .map(|input| PendingInputDto {
            id: input.id,
            run_id: input.run_id,
            intent: input.intent,
            status: input.status,
            text: message_text(&input.content),
            has_attachments: message_has_attachments(&input.content),
            child_run_id: input.child_run_id,
        })
}

#[tauri::command]
#[specta::specta]
pub async fn chat_branch(
    app: AppHandle,
    _service: State<'_, CarrotService>,
    request: ChatBranchRequest,
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
            .branch_chat(
                run_id.clone(),
                &request.pending_input_id,
                &request.conversation_id,
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
pub async fn chat_tool_approval(
    app: AppHandle,
    service: State<'_, CarrotService>,
    request: ToolApprovalRequest,
) -> Result<ChatStartResponse, AppError> {
    service
        .resolve_tool_approval(
            &request.run_id,
            &request.tool_execution_id,
            request.approved,
        )
        .await?;
    let response = ChatStartResponse {
        run_id: request.run_id.clone(),
    };
    spawn_resume(
        app,
        ChatResumeRequest {
            run_id: request.run_id,
            conversation_id: request.conversation_id,
        },
    );
    Ok(response)
}

#[tauri::command]
#[specta::specta]
pub async fn chat_tool_recovery(
    service: State<'_, CarrotService>,
    request: ToolRecoveryRequest,
) -> Result<(), AppError> {
    service
        .resolve_tool_recovery(
            &request.run_id,
            &request.tool_execution_id,
            request.resolution,
            request.note,
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_resume(
    app: AppHandle,
    _service: State<'_, CarrotService>,
    request: ChatResumeRequest,
) -> Result<ChatStartResponse, AppError> {
    let run_id = request.run_id.clone();
    let response = ChatStartResponse {
        run_id: run_id.clone(),
    };
    spawn_resume(app, request);
    Ok(response)
}

fn spawn_resume(app: AppHandle, request: ChatResumeRequest) {
    tauri::async_runtime::spawn(async move {
        let run_id = request.run_id.clone();
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
            .resume_chat(run_id.clone(), &request.conversation_id, sender)
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
}

#[tauri::command]
#[specta::specta]
pub async fn chat_snapshot(
    service: State<'_, CarrotService>,
    conversation_id: String,
) -> Result<ChatSnapshotDto, AppError> {
    service
        .chat_snapshot(&conversation_id)
        .await
        .map(ChatSnapshotDto::from)
}

fn message_text(value: &serde_json::Value) -> String {
    value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| {
            (part.get("type")?.as_str()? == "text")
                .then(|| part.get("text")?.as_str().map(ToOwned::to_owned))
                .flatten()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn message_has_attachments(value: &serde_json::Value) -> bool {
    value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|parts| {
            parts.iter().any(|part| {
                part.get("type").and_then(serde_json::Value::as_str) == Some("image_data_url")
            })
        })
}
