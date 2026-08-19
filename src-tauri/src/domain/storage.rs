#![allow(dead_code)]

use async_trait::async_trait;

use super::conversation::{Conversation, ConversationChanges, NewConversation};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("stored record changed concurrently")]
    Conflict,
    #[error("stored data is invalid: {message}")]
    InvalidData { message: String },
    #[error("storage is unavailable: {message}")]
    Unavailable { message: String },
}

#[async_trait]
pub trait ConversationStore: Send + Sync {
    async fn list(&self) -> Result<Vec<Conversation>, StoreError>;

    async fn get(&self, id: &str) -> Result<Option<Conversation>, StoreError>;

    async fn create(&self, input: NewConversation) -> Result<Conversation, StoreError>;

    async fn update(
        &self,
        id: &str,
        changes: ConversationChanges,
    ) -> Result<Option<Conversation>, StoreError>;

    async fn delete(&self, id: &str, expected_version: i64) -> Result<bool, StoreError>;
}
