use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRequest {
    pub model: String,
    pub input: Vec<ProviderInputItem>,
    pub tools: Vec<ToolDefinition>,
    pub previous_response_id: Option<String>,
    pub store: bool,
    pub reasoning_summary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderInputItem {
    Message {
        message: ProviderMessage,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolOutput {
        call_id: String,
        output: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMessage {
    pub role: MessageRole,
    pub content: Vec<MessageContent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    Developer,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text {
        text: String,
    },
    ImageDataUrl {
        data_url: String,
        detail: ImageDetail,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub strict: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    Started {
        response_id: String,
    },
    TextDelta {
        delta: String,
    },
    ReasoningDelta {
        delta: String,
    },
    ReasoningCompleted {
        duration_ms: i64,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments: serde_json::Value,
    },
    Completed {
        response_id: String,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    },
    Failed {
        message: String,
    },
    Cancelled,
    Paused,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider request is invalid: {0}")]
    InvalidRequest(String),
    #[error("provider request failed: {0}")]
    Request(String),
    #[error("provider event receiver closed")]
    ReceiverClosed,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancellation: CancellationToken,
    ) -> Result<(), ProviderError>;
}
