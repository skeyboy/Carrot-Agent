//! LLM provider adapters, starting with OpenAI Responses in Phase 2.

mod config;
mod openai_responses;
pub(crate) mod runtime;

pub use config::{ProviderConfigError, ProviderConfigLoader};
pub use openai_responses::OpenAiResponsesProvider;
pub use runtime::{LlmProvider, ProviderEvent, ProviderRequest};
