#![allow(dead_code)]

use async_trait::async_trait;

use super::conversation::{Conversation, ConversationChanges, NewConversation};
use super::run::{
    AgentRun, ChatSnapshot, CommitResult, LeaseRecovery, NewRun, NewRunItem, NewToolExecution,
    PendingInput, PendingInputIntent, PlanDraft, RecoveryResolution, RunEvent, RunItem,
    RunTransition, ToolApproval, ToolExecution, ToolExecutionResult,
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

    async fn mark_tool_outcome_unknown(
        &self,
        run_id: &str,
        execution_id: &str,
        reason: &str,
    ) -> Result<RunEvent, StoreError>;

    async fn create_plan(&self, run_id: &str, plan: PlanDraft) -> Result<RunEvent, StoreError>;

    async fn conversation_items(&self, conversation_id: &str) -> Result<Vec<RunItem>, StoreError>;

    async fn snapshot(&self, conversation_id: &str) -> Result<ChatSnapshot, StoreError>;

    async fn get_run(&self, run_id: &str) -> Result<Option<AgentRun>, StoreError>;

    async fn request_pause(&self, run_id: &str) -> Result<RunEvent, StoreError>;

    async fn claim_resume(
        &self,
        run_id: &str,
        runtime_instance_id: &str,
    ) -> Result<AgentRun, StoreError>;

    async fn enqueue_input(
        &self,
        run_id: &str,
        intent: PendingInputIntent,
        content: serde_json::Value,
    ) -> Result<PendingInput, StoreError>;

    async fn consume_append_inputs(&self, run_id: &str) -> Result<Vec<RunItem>, StoreError>;

    async fn recover_expired_leases(
        &self,
        runtime_instance_id: &str,
    ) -> Result<Vec<LeaseRecovery>, StoreError>;

    async fn renew_lease(
        &self,
        run_id: &str,
        runtime_instance_id: &str,
    ) -> Result<bool, StoreError>;

    async fn request_tool_approval(
        &self,
        run_id: &str,
        execution_id: &str,
    ) -> Result<ToolApproval, StoreError>;

    async fn resolve_tool_approval(
        &self,
        run_id: &str,
        execution_id: &str,
        approved: bool,
    ) -> Result<RunEvent, StoreError>;

    async fn pending_tool_executions(&self, run_id: &str)
    -> Result<Vec<ToolExecution>, StoreError>;

    async fn resolve_recovery(
        &self,
        run_id: &str,
        execution_id: &str,
        resolution: RecoveryResolution,
        note: Option<String>,
    ) -> Result<RunEvent, StoreError>;

    async fn get_pending_input(&self, input_id: &str) -> Result<Option<PendingInput>, StoreError>;
}
