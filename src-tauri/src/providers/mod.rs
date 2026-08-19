//! LLM provider adapters, starting with OpenAI Responses in Phase 2.

mod config;
mod model_catalog;
mod openai_responses;
pub(crate) mod runtime;

pub use config::{ProviderConfigError, ProviderConfigLoader};
pub use model_catalog::OpenAiModelCatalog;
pub use openai_responses::OpenAiResponsesProvider;
pub use runtime::{LlmProvider, ProviderEvent, ProviderRequest};
