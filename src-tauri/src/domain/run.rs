use serde::{Deserialize, Serialize};
use specta::Type;

use super::provider::ProviderProfile;
use super::settings::RunStrategy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    PauseRequested,
    Paused,
    Suspended,
    WaitingForApproval,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    RecoveryRequired,
}

impl RunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::PauseRequested => "pause_requested",
            Self::Paused => "paused",
            Self::Suspended => "suspended",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::RecoveryRequired => "recovery_required",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "pause_requested" => Self::PauseRequested,
            "paused" => Self::Paused,
            "suspended" => Self::Suspended,
            "waiting_for_approval" => Self::WaitingForApproval,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            "recovery_required" => Self::RecoveryRequired,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    Routing,
    Planning,
    ModelStream,
    ToolPrepare,
    ToolExecute,
    ObservationCommit,
    Reflecting,
    Finalizing,
    None,
}

impl RunPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Routing => "routing",
            Self::Planning => "planning",
            Self::ModelStream => "model_stream",
            Self::ToolPrepare => "tool_prepare",
            Self::ToolExecute => "tool_execute",
            Self::ObservationCommit => "observation_commit",
            Self::Reflecting => "reflecting",
            Self::Finalizing => "finalizing",
            Self::None => "none",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "routing" => Self::Routing,
            "planning" => Self::Planning,
            "model_stream" => Self::ModelStream,
            "tool_prepare" => Self::ToolPrepare,
            "tool_execute" => Self::ToolExecute,
            "observation_commit" => Self::ObservationCommit,
            "reflecting" => Self::Reflecting,
            "finalizing" => Self::Finalizing,
            "none" => Self::None,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    pub conversation_id: String,
    pub parent_run_id: Option<String>,
    pub status: RunStatus,
    pub phase: RunPhase,
    pub strategy: RunStrategy,
    pub provider_profile_id: String,
    pub provider_snapshot: ProviderProfile,
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

#[derive(Debug, Clone)]
pub struct NewRun {
    pub id: String,
    pub conversation_id: String,
    pub strategy: RunStrategy,
    pub provider_profile: ProviderProfile,
    pub model: String,
    pub runtime_instance_id: String,
    pub replaces_run_id: Option<String>,
    pub user_content: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunItem {
    pub id: String,
    pub run_id: String,
    pub seq: i64,
    pub kind: String,
    pub role: Option<String>,
    pub status: String,
    pub content: serde_json::Value,
    pub provider_item_id: Option<String>,
    pub call_id: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewRunItem {
    pub kind: String,
    pub role: Option<String>,
    pub content: serde_json::Value,
    pub call_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub id: String,
    pub run_id: String,
    pub seq: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub persisted_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RunTransition {
    pub status: RunStatus,
    pub phase: RunPhase,
    pub event_kind: String,
    pub payload: serde_json::Value,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    pub id: String,
    pub run_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub status: String,
    pub risk: String,
    pub arguments: serde_json::Value,
    pub arguments_hash: String,
    pub output: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub retryable: bool,
    pub prepared_at_ms: i64,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewToolExecution {
    pub id: String,
    pub call_id: String,
    pub tool_name: String,
    pub risk: String,
    pub arguments: serde_json::Value,
    pub arguments_hash: String,
    pub retryable: bool,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub output: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
pub struct PlanDraft {
    pub goal: String,
    pub steps: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatSnapshot {
    pub conversation_id: String,
    pub active_run: Option<AgentRun>,
    pub items: Vec<RunItem>,
    pub events: Vec<RunEvent>,
    pub tool_executions: Vec<ToolExecution>,
}

#[derive(Debug, Clone)]
pub struct CommitResult {
    #[allow(dead_code)]
    pub item: Option<RunItem>,
    #[allow(dead_code)]
    pub event: RunEvent,
}
