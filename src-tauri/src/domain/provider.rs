use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenaiResponses,
    OpenaiCompatible,
}

impl ProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiResponses => "openai_responses",
            Self::OpenaiCompatible => "openai_compatible",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    Responses,
    ChatCompletions,
}

impl ProviderProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub tools: bool,
    pub images: bool,
    pub files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ProviderProfile {
    pub id: String,
    pub label: String,
    pub kind: ProviderKind,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub default_model: String,
    pub credential_ref: String,
    pub store_responses: bool,
    pub capabilities: ProviderCapabilities,
}
