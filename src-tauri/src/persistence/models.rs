use diesel::{AsChangeset, Insertable, Queryable, Selectable};

use crate::domain::attachment::AttachmentDescriptor;
use crate::domain::conversation::Conversation;
use crate::domain::provider::ProviderProfile;
use crate::domain::run::{
    AgentRun, PendingInput, PendingInputIntent, RunEvent, RunItem, RunPhase, RunStatus,
    ToolApproval, ToolExecution,
};
use crate::domain::settings::RunStrategy;
use crate::domain::storage::StoreError;

use super::schema::{
    attachments, conversations, items, pending_inputs, plan_steps, plans, run_events,
    run_snapshots, runs, tool_approvals, tool_executions,
};

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = pending_inputs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PendingInputRow {
    pub id: String,
    pub run_id: String,
    pub item_id: Option<String>,
    pub intent: String,
    pub status: String,
    pub content_json: String,
    pub created_at_ms: i64,
    pub consumed_at_ms: Option<i64>,
    pub child_run_id: Option<String>,
}

impl TryFrom<PendingInputRow> for PendingInput {
    type Error = StoreError;

    fn try_from(row: PendingInputRow) -> Result<Self, Self::Error> {
        let intent = match row.intent.as_str() {
            "append" => PendingInputIntent::Append,
            "fork" => PendingInputIntent::Fork,
            "cancel_and_replace" => PendingInputIntent::CancelAndReplace,
            _ => {
                return Err(StoreError::InvalidData {
                    message: format!("pending input {} has invalid intent", row.id),
                });
            }
        };
        Ok(Self {
            id: row.id,
            run_id: row.run_id,
            item_id: row.item_id,
            intent,
            status: row.status,
            content: serde_json::from_str(&row.content_json).map_err(|error| {
                StoreError::InvalidData {
                    message: format!("pending input content is invalid: {error}"),
                }
            })?,
            created_at_ms: row.created_at_ms,
            consumed_at_ms: row.consumed_at_ms,
            child_run_id: row.child_run_id,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = pending_inputs)]
pub struct NewPendingInputRow<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub item_id: Option<&'a str>,
    pub intent: &'a str,
    pub status: &'a str,
    pub content_json: &'a str,
    pub created_at_ms: i64,
    pub consumed_at_ms: Option<i64>,
    pub child_run_id: Option<&'a str>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = runs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RunRow {
    pub id: String,
    pub conversation_id: String,
    pub parent_run_id: Option<String>,
    pub status: String,
    pub phase: String,
    pub strategy: String,
    pub provider_profile_id: String,
    pub provider_snapshot_json: String,
    pub model: String,
    pub version: i64,
    pub last_event_seq: i64,
    pub runtime_instance_id: Option<String>,
    pub lease_expires_at_ms: Option<i64>,
    pub stop_reason: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub completed_at_ms: Option<i64>,
}

impl TryFrom<RunRow> for AgentRun {
    type Error = StoreError;

    fn try_from(row: RunRow) -> Result<Self, Self::Error> {
        let status = RunStatus::parse(&row.status).ok_or_else(|| StoreError::InvalidData {
            message: format!("run {} has invalid status", row.id),
        })?;
        let phase = RunPhase::parse(&row.phase).ok_or_else(|| StoreError::InvalidData {
            message: format!("run {} has invalid phase", row.id),
        })?;
        let strategy =
            RunStrategy::parse(&row.strategy).ok_or_else(|| StoreError::InvalidData {
                message: format!("run {} has invalid strategy", row.id),
            })?;
        let provider_snapshot = serde_json::from_str::<ProviderProfile>(
            &row.provider_snapshot_json,
        )
        .map_err(|error| StoreError::InvalidData {
            message: format!("run {} provider snapshot is invalid: {error}", row.id),
        })?;
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            parent_run_id: row.parent_run_id,
            status,
            phase,
            strategy,
            provider_profile_id: row.provider_profile_id,
            provider_snapshot,
            model: row.model,
            version: row.version,
            last_event_seq: row.last_event_seq,
            runtime_instance_id: row.runtime_instance_id,
            lease_expires_at_ms: row.lease_expires_at_ms,
            stop_reason: row.stop_reason,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
            completed_at_ms: row.completed_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = runs)]
pub struct NewRunRow<'a> {
    pub id: &'a str,
    pub conversation_id: &'a str,
    pub parent_run_id: Option<&'a str>,
    pub status: &'a str,
    pub phase: &'a str,
    pub strategy: &'a str,
    pub provider_profile_id: &'a str,
    pub provider_snapshot_json: &'a str,
    pub model: &'a str,
    pub version: i64,
    pub last_event_seq: i64,
    pub runtime_instance_id: Option<&'a str>,
    pub lease_expires_at_ms: Option<i64>,
    pub stop_reason: Option<&'a str>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub completed_at_ms: Option<i64>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = tool_approvals)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ToolApprovalRow {
    pub id: String,
    pub run_id: String,
    pub tool_execution_id: String,
    pub call_id: String,
    pub arguments_hash: String,
    pub status: String,
    pub requested_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

impl From<ToolApprovalRow> for ToolApproval {
    fn from(row: ToolApprovalRow) -> Self {
        Self {
            id: row.id,
            run_id: row.run_id,
            tool_execution_id: row.tool_execution_id,
            call_id: row.call_id,
            arguments_hash: row.arguments_hash,
            status: row.status,
            requested_at_ms: row.requested_at_ms,
            resolved_at_ms: row.resolved_at_ms,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = tool_approvals)]
pub struct NewToolApprovalRow<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub tool_execution_id: &'a str,
    pub call_id: &'a str,
    pub arguments_hash: &'a str,
    pub status: &'a str,
    pub requested_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

#[derive(AsChangeset)]
#[diesel(table_name = runs)]
pub struct RunChangeset<'a> {
    pub status: &'a str,
    pub phase: &'a str,
    pub version: i64,
    pub last_event_seq: i64,
    pub stop_reason: Option<&'a str>,
    pub lease_expires_at_ms: Option<i64>,
    pub updated_at_ms: i64,
    pub completed_at_ms: Option<i64>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RunItemRow {
    pub id: String,
    pub run_id: String,
    pub seq: i64,
    pub kind: String,
    pub role: Option<String>,
    pub status: String,
    pub content_json: String,
    pub provider_item_id: Option<String>,
    pub call_id: Option<String>,
    pub created_at_ms: i64,
}

impl TryFrom<RunItemRow> for RunItem {
    type Error = StoreError;

    fn try_from(row: RunItemRow) -> Result<Self, Self::Error> {
        let content =
            serde_json::from_str(&row.content_json).map_err(|error| StoreError::InvalidData {
                message: format!("item {} content is invalid: {error}", row.id),
            })?;
        Ok(Self {
            id: row.id,
            run_id: row.run_id,
            seq: row.seq,
            kind: row.kind,
            role: row.role,
            status: row.status,
            content,
            provider_item_id: row.provider_item_id,
            call_id: row.call_id,
            created_at_ms: row.created_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = items)]
pub struct NewRunItemRow<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub seq: i64,
    pub kind: &'a str,
    pub role: Option<&'a str>,
    pub status: &'a str,
    pub content_json: &'a str,
    pub provider_item_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub created_at_ms: i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = run_events)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RunEventRow {
    pub id: String,
    pub run_id: String,
    pub seq: i64,
    pub kind: String,
    pub payload_json: String,
    pub persisted_at_ms: i64,
}

impl TryFrom<RunEventRow> for RunEvent {
    type Error = StoreError;

    fn try_from(row: RunEventRow) -> Result<Self, Self::Error> {
        let payload =
            serde_json::from_str(&row.payload_json).map_err(|error| StoreError::InvalidData {
                message: format!("event {} payload is invalid: {error}", row.id),
            })?;
        Ok(Self {
            id: row.id,
            run_id: row.run_id,
            seq: row.seq,
            kind: row.kind,
            payload,
            persisted_at_ms: row.persisted_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = run_events)]
pub struct NewRunEventRow<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub seq: i64,
    pub kind: &'a str,
    pub payload_json: &'a str,
    pub persisted_at_ms: i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = tool_executions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ToolExecutionRow {
    pub id: String,
    pub run_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub status: String,
    pub risk: String,
    pub arguments_json: String,
    pub arguments_hash: String,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
    pub prepared_at_ms: i64,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub idempotency_key: Option<String>,
    pub reconciliation_status: String,
    pub reconciliation_note: Option<String>,
}

impl TryFrom<ToolExecutionRow> for ToolExecution {
    type Error = StoreError;

    fn try_from(row: ToolExecutionRow) -> Result<Self, Self::Error> {
        let arguments =
            serde_json::from_str(&row.arguments_json).map_err(|error| StoreError::InvalidData {
                message: format!("tool execution {} arguments are invalid: {error}", row.id),
            })?;
        let output = row
            .output_json
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| StoreError::InvalidData {
                message: format!("tool execution {} output is invalid: {error}", row.id),
            })?;
        Ok(Self {
            id: row.id,
            run_id: row.run_id,
            call_id: row.call_id,
            tool_name: row.tool_name,
            status: row.status,
            risk: row.risk,
            arguments,
            arguments_hash: row.arguments_hash,
            output,
            error_message: row.error_message,
            retryable: row.retryable,
            prepared_at_ms: row.prepared_at_ms,
            started_at_ms: row.started_at_ms,
            completed_at_ms: row.completed_at_ms,
            idempotency_key: row.idempotency_key,
            reconciliation_status: row.reconciliation_status,
            reconciliation_note: row.reconciliation_note,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = tool_executions)]
pub struct NewToolExecutionRow<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub call_id: &'a str,
    pub tool_name: &'a str,
    pub status: &'a str,
    pub risk: &'a str,
    pub arguments_json: &'a str,
    pub arguments_hash: &'a str,
    pub output_json: Option<&'a str>,
    pub error_message: Option<&'a str>,
    pub retryable: bool,
    pub prepared_at_ms: i64,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub idempotency_key: Option<&'a str>,
    pub reconciliation_status: &'a str,
    pub reconciliation_note: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = plans)]
pub struct NewPlanRow<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub revision: i64,
    pub goal: &'a str,
    pub status: &'a str,
    pub created_at_ms: i64,
}

#[derive(Insertable)]
#[diesel(table_name = plan_steps)]
pub struct NewPlanStepRow<'a> {
    pub id: &'a str,
    pub plan_id: &'a str,
    pub ordinal: i64,
    pub title: &'a str,
    pub acceptance: &'a str,
    pub status: &'a str,
    pub attempt: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Insertable)]
#[diesel(table_name = run_snapshots)]
pub struct NewRunSnapshotRow<'a> {
    pub run_id: &'a str,
    pub event_high_water_seq: i64,
    pub state_json: &'a str,
    pub updated_at_ms: i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = attachments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AttachmentRow {
    pub id: String,
    pub conversation_id: String,
    #[allow(dead_code)]
    pub item_id: Option<String>,
    pub file_name: String,
    pub media_type: String,
    pub byte_length: i64,
    pub content_hash: String,
    pub relative_path: String,
    pub status: String,
    pub created_at_ms: i64,
}

impl TryFrom<AttachmentRow> for AttachmentDescriptor {
    type Error = StoreError;

    fn try_from(row: AttachmentRow) -> Result<Self, Self::Error> {
        let byte_length = u64::try_from(row.byte_length).map_err(|_| StoreError::InvalidData {
            message: format!("attachment {} has an invalid byte length", row.id),
        })?;
        if row.id.trim().is_empty()
            || row.conversation_id.trim().is_empty()
            || row.file_name.trim().is_empty()
            || row.media_type != "image/png"
            || row.content_hash.trim().is_empty()
            || row.relative_path.trim().is_empty()
            || row.status != "ready"
            || row.created_at_ms < 0
        {
            return Err(StoreError::InvalidData {
                message: format!("attachment {} failed domain validation", row.id),
            });
        }
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            media_type: row.media_type,
            file_name: row.file_name,
            byte_length,
            content_hash: row.content_hash,
            relative_path: row.relative_path,
            created_at_ms: row.created_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = attachments)]
pub struct NewAttachmentRow<'a> {
    pub id: &'a str,
    pub conversation_id: &'a str,
    pub item_id: Option<&'a str>,
    pub file_name: &'a str,
    pub media_type: &'a str,
    pub byte_length: i64,
    pub content_hash: &'a str,
    pub relative_path: &'a str,
    pub status: &'a str,
    pub created_at_ms: i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub default_provider_profile_id: String,
    pub default_model: String,
    pub archived: bool,
    pub version: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl TryFrom<ConversationRow> for Conversation {
    type Error = StoreError;

    fn try_from(row: ConversationRow) -> Result<Self, Self::Error> {
        if row.id.trim().is_empty()
            || row.title.trim().is_empty()
            || row.default_provider_profile_id.trim().is_empty()
            || row.default_model.trim().is_empty()
            || row.version < 1
            || row.created_at_ms < 0
            || row.updated_at_ms < row.created_at_ms
        {
            return Err(StoreError::InvalidData {
                message: format!("conversation {} failed domain validation", row.id),
            });
        }

        Ok(Self {
            id: row.id,
            title: row.title,
            default_provider_profile_id: row.default_provider_profile_id,
            default_model: row.default_model,
            archived: row.archived,
            version: row.version,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = conversations)]
pub struct NewConversationRow<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub default_provider_profile_id: &'a str,
    pub default_model: &'a str,
    pub archived: bool,
    pub version: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(AsChangeset)]
#[diesel(table_name = conversations)]
pub struct ConversationChangeset<'a> {
    pub title: Option<&'a str>,
    pub default_provider_profile_id: Option<&'a str>,
    pub default_model: Option<&'a str>,
    pub version: i64,
    pub updated_at_ms: i64,
}
