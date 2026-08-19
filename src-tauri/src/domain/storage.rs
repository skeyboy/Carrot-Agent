#![allow(dead_code)]

use async_trait::async_trait;

use super::conversation::{Conversation, ConversationChanges, NewConversation};
use super::run::{
    AgentRun, ChatSnapshot, CommitResult, NewRun, NewRunItem, NewToolExecution, PlanDraft,
    RunEvent, RunItem, RunTransition, ToolExecutionResult,
};

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

#[async_trait]
pub trait RunStore: Send + Sync {
    async fn start(&self, input: NewRun) -> Result<CommitResult, StoreError>;

    async fn transition(
        &self,
        run_id: &str,
        transition: RunTransition,
    ) -> Result<RunEvent, StoreError>;

    async fn commit_item(
        &self,
        run_id: &str,
        item: NewRunItem,
        event_kind: &str,
        event_payload: serde_json::Value,
    ) -> Result<CommitResult, StoreError>;

    async fn prepare_tool(
        &self,
        run_id: &str,
        execution: NewToolExecution,
    ) -> Result<CommitResult, StoreError>;

    async fn mark_tool_executing(
        &self,
        run_id: &str,
        execution_id: &str,
    ) -> Result<RunEvent, StoreError>;

    async fn finish_tool(
        &self,
        run_id: &str,
        execution_id: &str,
        call_id: &str,
        result: ToolExecutionResult,
    ) -> Result<CommitResult, StoreError>;

    async fn create_plan(&self, run_id: &str, plan: PlanDraft) -> Result<RunEvent, StoreError>;

    async fn conversation_items(&self, conversation_id: &str) -> Result<Vec<RunItem>, StoreError>;

    async fn snapshot(&self, conversation_id: &str) -> Result<ChatSnapshot, StoreError>;

    async fn get_run(&self, run_id: &str) -> Result<Option<AgentRun>, StoreError>;
}
