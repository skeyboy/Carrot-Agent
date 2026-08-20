use async_trait::async_trait;
use diesel::dsl::max;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use uuid::Uuid;

use crate::domain::run::{
    AgentRun, ChatSnapshot, CommitResult, LeaseRecovery, NewRun, NewRunItem, NewToolExecution,
    PendingInput, PendingInputIntent, PlanDraft, RecoveryResolution, RunEvent, RunItem, RunPhase,
    RunStatus, RunTransition, ToolApproval, ToolExecution, ToolExecutionResult,
};
use crate::domain::storage::{RunStore, StoreError};

use super::database::Database;
use super::models::{
    NewPendingInputRow, NewPlanRow, NewPlanStepRow, NewRunEventRow, NewRunItemRow, NewRunRow,
    NewRunSnapshotRow, NewToolApprovalRow, NewToolExecutionRow, PendingInputRow, RunChangeset,
    RunEventRow, RunItemRow, RunRow, ToolApprovalRow, ToolExecutionRow,
};
use super::now_ms;
use super::schema::{
    items, pending_inputs, plan_steps, plans, run_events, run_snapshots, runs, tool_approvals,
    tool_executions,
};

#[derive(Clone)]
pub struct SqliteRunStore {
    database: Database,
}

impl SqliteRunStore {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    async fn load_run_row(&self, run_id: &str) -> Result<Option<RunRow>, StoreError> {
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        runs::table
            .filter(runs::id.eq(run_id))
            .select(RunRow::as_select())
            .first(&mut connection)
            .await
            .optional()
            .map_err(query_error)
    }
}

#[async_trait]
impl RunStore for SqliteRunStore {
    async fn start(&self, input: NewRun) -> Result<CommitResult, StoreError> {
        let now = now_ms()?;
        let provider_snapshot_json =
            serde_json::to_string(&input.provider_profile).map_err(invalid_serialization)?;
        let tool_catalog_snapshot_json =
            serde_json::to_string(&input.tool_catalog_snapshot).map_err(invalid_serialization)?;
        let user_content_json =
            serde_json::to_string(&input.user_content).map_err(invalid_serialization)?;
        let started_payload = serde_json::json!({
            "status": "running",
            "phase": "routing",
            "strategy": input.strategy,
        });
        let started_payload_json =
            serde_json::to_string(&started_payload).map_err(invalid_serialization)?;
        let user_payload = serde_json::json!({"itemSeq": 1});
        let user_payload_json =
            serde_json::to_string(&user_payload).map_err(invalid_serialization)?;
        let item_id = Uuid::now_v7().to_string();
        let first_event_id = Uuid::now_v7().to_string();
        let second_event_id = Uuid::now_v7().to_string();
        let parent_event_id = Uuid::now_v7().to_string();
        let run_id = input.id.clone();
        let run_row = NewRunRow {
            id: &input.id,
            conversation_id: &input.conversation_id,
            parent_run_id: input
                .parent_run_id
                .as_deref()
                .or(input.replaces_run_id.as_deref()),
            status: "running",
            phase: "routing",
            strategy: input.strategy.as_str(),
            provider_profile_id: &input.provider_profile.id,
            provider_snapshot_json: &provider_snapshot_json,
            model: &input.model,
            version: 1,
            last_event_seq: 2,
            runtime_instance_id: Some(&input.runtime_instance_id),
            lease_expires_at_ms: Some(now + 30_000),
            stop_reason: None,
            created_at_ms: now,
            updated_at_ms: now,
            completed_at_ms: None,
            tool_catalog_snapshot_json: &tool_catalog_snapshot_json,
        };
        let item_row = NewRunItemRow {
            id: &item_id,
            run_id: &input.id,
            seq: 1,
            kind: "message",
            role: Some("user"),
            status: "committed",
            content_json: &user_content_json,
            provider_item_id: None,
            call_id: None,
            created_at_ms: now,
        };
        let first_event = NewRunEventRow {
            id: &first_event_id,
            run_id: &input.id,
            seq: 1,
            kind: "run_started",
            payload_json: &started_payload_json,
            persisted_at_ms: now,
        };
        let second_event = NewRunEventRow {
            id: &second_event_id,
            run_id: &input.id,
            seq: 2,
            kind: "user_message_committed",
            payload_json: &user_payload_json,
            persisted_at_ms: now,
        };
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                if let Some(replaced_run_id) = input.replaces_run_id.as_deref() {
                    let parent = runs::table
                        .filter(runs::id.eq(replaced_run_id))
                        .filter(runs::conversation_id.eq(&input.conversation_id))
                        .filter(runs::status.eq_any(["paused", "cancelled"]))
                        .select(RunRow::as_select())
                        .first::<RunRow>(connection)
                        .await
                        .optional()?;
                    let Some(parent) = parent else {
                        return Err(diesel::result::Error::RollbackTransaction);
                    };
                    diesel::update(items::table.filter(items::run_id.eq(replaced_run_id)))
                        .set(items::status.eq("superseded"))
                        .execute(connection)
                        .await?;
                    commit_parent_branch_state(
                        connection,
                        &parent,
                        &parent_event_id,
                        "cancelled",
                        "run_replaced_from_inbox",
                        &input.id,
                        now,
                    )
                    .await?;
                } else if let Some(parent_run_id) = input.parent_run_id.as_deref() {
                    let parent = runs::table
                        .filter(runs::id.eq(parent_run_id))
                        .filter(runs::conversation_id.eq(&input.conversation_id))
                        .filter(runs::status.eq("paused"))
                        .select(RunRow::as_select())
                        .first::<RunRow>(connection)
                        .await?;
                    commit_parent_branch_state(
                        connection,
                        &parent,
                        &parent_event_id,
                        "suspended",
                        "run_forked_from_inbox",
                        &input.id,
                        now,
                    )
                    .await?;
                }
                if let Some(pending_input_id) = input.source_pending_input_id.as_deref() {
                    let pending = pending_inputs::table
                        .filter(pending_inputs::id.eq(pending_input_id))
                        .filter(pending_inputs::status.eq("pending"))
                        .filter(
                            pending_inputs::run_id.eq(input
                                .parent_run_id
                                .as_deref()
                                .or(input.replaces_run_id.as_deref())
                                .unwrap_or_default()),
                        )
                        .select(PendingInputRow::as_select())
                        .first::<PendingInputRow>(connection)
                        .await?;
                    if pending.content_json != user_content_json || pending.intent == "append" {
                        return Err(diesel::result::Error::RollbackTransaction);
                    }
                }
                diesel::insert_into(runs::table)
                    .values(run_row)
                    .execute(connection)
                    .await?;
                diesel::insert_into(items::table)
                    .values(item_row)
                    .execute(connection)
                    .await?;
                diesel::insert_into(run_events::table)
                    .values(first_event)
                    .execute(connection)
                    .await?;
                if let Some(pending_input_id) = input.source_pending_input_id.as_deref() {
                    diesel::update(
                        pending_inputs::table.filter(pending_inputs::id.eq(pending_input_id)),
                    )
                    .set((
                        pending_inputs::status.eq("consumed"),
                        pending_inputs::item_id.eq(Some(&item_id)),
                        pending_inputs::child_run_id.eq(Some(&input.id)),
                        pending_inputs::consumed_at_ms.eq(Some(now)),
                    ))
                    .execute(connection)
                    .await?;
                }
                diesel::insert_into(run_events::table)
                    .values(second_event)
                    .execute(connection)
                    .await?;
                let state_json = serde_json::json!({
                    "status": "running",
                    "phase": "routing",
                    "version": 1,
                    "runtimeInstanceId": input.runtime_instance_id,
                })
                .to_string();
                diesel::insert_into(run_snapshots::table)
                    .values(NewRunSnapshotRow {
                        run_id: &input.id,
                        event_high_water_seq: 2,
                        state_json: &state_json,
                        updated_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                Ok(())
            })
            .await
            .map_err(query_error)?;

        Ok(CommitResult {
            item: Some(RunItem {
                id: item_id,
                run_id: run_id.clone(),
                seq: 1,
                kind: "message".to_owned(),
                role: Some("user".to_owned()),
                status: "committed".to_owned(),
                content: input.user_content,
                provider_item_id: None,
                call_id: None,
                created_at_ms: now,
            }),
            event: RunEvent {
                id: second_event_id,
                run_id,
                seq: 2,
                kind: "user_message_committed".to_owned(),
                payload: user_payload,
                persisted_at_ms: now,
            },
        })
    }

    async fn transition(
        &self,
        run_id: &str,
        transition: RunTransition,
    ) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let payload_json =
            serde_json::to_string(&transition.payload).map_err(invalid_serialization)?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let event = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                if transition.status == RunStatus::Completed {
                    let pending_append = pending_inputs::table
                        .filter(pending_inputs::run_id.eq(run_id))
                        .filter(pending_inputs::status.eq("pending"))
                        .filter(pending_inputs::intent.eq("append"))
                        .select(pending_inputs::id)
                        .first::<String>(connection)
                        .await
                        .optional()?;
                    if pending_append.is_some() {
                        return Err(diesel::result::Error::RollbackTransaction);
                    }
                }
                let next_seq = row.last_event_seq + 1;
                let next_version = row.version + 1;
                let completed_at = transition.status.is_terminal().then_some(now);
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: next_seq,
                        kind: &transition.event_kind,
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let updated = diesel::update(
                    runs::table.filter(runs::id.eq(run_id).and(runs::version.eq(row.version))),
                )
                .set(RunChangeset {
                    status: transition.status.as_str(),
                    phase: transition.phase.as_str(),
                    version: next_version,
                    last_event_seq: next_seq,
                    stop_reason: transition.stop_reason.as_deref(),
                    lease_expires_at_ms: Some(if transition.status.is_terminal() {
                        now
                    } else {
                        now + 30_000
                    }),
                    updated_at_ms: now,
                    completed_at_ms: completed_at,
                })
                .execute(connection)
                .await?;
                if updated != 1 {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                if transition.status.is_terminal() {
                    let plan_status =
                        if transition.status == crate::domain::run::RunStatus::Completed {
                            "completed"
                        } else {
                            "failed"
                        };
                    diesel::update(
                        plans::table
                            .filter(plans::run_id.eq(run_id))
                            .filter(plans::status.eq("active")),
                    )
                    .set(plans::status.eq(plan_status))
                    .execute(connection)
                    .await?;
                }
                if matches!(transition.status, RunStatus::Failed | RunStatus::Cancelled) {
                    diesel::update(
                        items::table
                            .filter(items::run_id.eq(run_id))
                            .filter(items::status.eq("committed")),
                    )
                    .set(items::status.eq("abandoned"))
                    .execute(connection)
                    .await?;
                }
                upsert_run_snapshot(
                    connection,
                    &row,
                    next_seq,
                    transition.status.as_str(),
                    transition.phase.as_str(),
                    now,
                )
                .await?;
                Ok(RunEvent {
                    id: event_id.clone(),
                    run_id: run_id.to_owned(),
                    seq: next_seq,
                    kind: transition.event_kind.clone(),
                    payload: transition.payload.clone(),
                    persisted_at_ms: now,
                })
            })
            .await
            .map_err(transaction_error)?;
        Ok(event)
    }

    async fn commit_item(
        &self,
        run_id: &str,
        item: NewRunItem,
        event_kind: &str,
        event_payload: serde_json::Value,
    ) -> Result<CommitResult, StoreError> {
        let now = now_ms()?;
        let item_id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        let content_json = serde_json::to_string(&item.content).map_err(invalid_serialization)?;
        let payload_json = serde_json::to_string(&event_payload).map_err(invalid_serialization)?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let item_seq = items::table
                    .filter(items::run_id.eq(run_id))
                    .select(max(items::seq))
                    .first::<Option<i64>>(connection)
                    .await?
                    .unwrap_or(0)
                    + 1;
                let event_seq = row.last_event_seq + 1;
                diesel::insert_into(items::table)
                    .values(NewRunItemRow {
                        id: &item_id,
                        run_id,
                        seq: item_seq,
                        kind: &item.kind,
                        role: item.role.as_deref(),
                        status: &item.status,
                        content_json: &content_json,
                        provider_item_id: None,
                        call_id: item.call_id.as_deref(),
                        created_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: event_kind,
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_watermark(connection, &row, event_seq, now).await?;
                Ok((item_seq, event_seq))
            })
            .await
            .map(|(item_seq, event_seq)| CommitResult {
                item: Some(RunItem {
                    id: item_id,
                    run_id: run_id.to_owned(),
                    seq: item_seq,
                    kind: item.kind,
                    role: item.role,
                    status: item.status,
                    content: item.content,
                    provider_item_id: None,
                    call_id: item.call_id,
                    created_at_ms: now,
                }),
                event: RunEvent {
                    id: event_id,
                    run_id: run_id.to_owned(),
                    seq: event_seq,
                    kind: event_kind.to_owned(),
                    payload: event_payload,
                    persisted_at_ms: now,
                },
            })
            .map_err(transaction_error)
    }

    async fn prepare_tool(
        &self,
        run_id: &str,
        execution: NewToolExecution,
    ) -> Result<CommitResult, StoreError> {
        self.prepare_tool_transaction(run_id, execution).await
    }

    async fn mark_tool_executing(
        &self,
        run_id: &str,
        execution_id: &str,
    ) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({"toolExecutionId": execution_id});
        let payload_json = serde_json::to_string(&payload).map_err(invalid_serialization)?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let event_seq = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let updated = diesel::update(
                    tool_executions::table.filter(
                        tool_executions::id
                            .eq(execution_id)
                            .and(tool_executions::run_id.eq(run_id))
                            .and(tool_executions::status.eq("prepared")),
                    ),
                )
                .set((
                    tool_executions::status.eq("executing"),
                    tool_executions::started_at_ms.eq(Some(now)),
                ))
                .execute(connection)
                .await?;
                if updated != 1 {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                let event_seq = row.last_event_seq + 1;
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: "tool_execution_started",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let status = active_status(&row);
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    status,
                    "tool_execute",
                    None,
                    now,
                )
                .await?;
                Ok(event_seq)
            })
            .await
            .map_err(transaction_error)?;
        Ok(RunEvent {
            id: event_id,
            run_id: run_id.to_owned(),
            seq: event_seq,
            kind: "tool_execution_started".to_owned(),
            payload,
            persisted_at_ms: now,
        })
    }

    async fn finish_tool(
        &self,
        run_id: &str,
        execution_id: &str,
        call_id: &str,
        result: ToolExecutionResult,
    ) -> Result<CommitResult, StoreError> {
        self.finish_tool_transaction(run_id, execution_id, call_id, result)
            .await
    }

    async fn mark_tool_outcome_unknown(
        &self,
        run_id: &str,
        execution_id: &str,
        reason: &str,
    ) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "toolExecutionId": execution_id,
            "reason": reason,
        });
        let payload_json = payload.to_string();
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let event_seq = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let updated = diesel::update(
                    tool_executions::table
                        .filter(tool_executions::id.eq(execution_id))
                        .filter(tool_executions::run_id.eq(run_id))
                        .filter(tool_executions::status.eq("executing")),
                )
                .set((
                    tool_executions::reconciliation_status.eq("pending"),
                    tool_executions::reconciliation_note.eq(Some(reason)),
                    tool_executions::error_message.eq(Some(reason)),
                ))
                .execute(connection)
                .await?;
                if updated != 1 {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                let event_seq = row.last_event_seq + 1;
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: "tool_reconciliation_required",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    "recovery_required",
                    "none",
                    Some(reason),
                    now,
                )
                .await?;
                Ok(event_seq)
            })
            .await
            .map_err(transaction_error)?;
        Ok(RunEvent {
            id: event_id,
            run_id: run_id.to_owned(),
            seq: event_seq,
            kind: "tool_reconciliation_required".to_owned(),
            payload,
            persisted_at_ms: now,
        })
    }

    async fn create_plan(&self, run_id: &str, plan: PlanDraft) -> Result<RunEvent, StoreError> {
        self.create_plan_transaction(run_id, plan).await
    }

    async fn conversation_items(&self, conversation_id: &str) -> Result<Vec<RunItem>, StoreError> {
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let run_ids = runs::table
            .filter(runs::conversation_id.eq(conversation_id))
            .order((runs::created_at_ms.asc(), runs::id.asc()))
            .select(runs::id)
            .load::<String>(&mut connection)
            .await
            .map_err(query_error)?;
        let mut output = Vec::new();
        for run_id in run_ids {
            let rows = items::table
                .filter(items::run_id.eq(run_id))
                .filter(items::status.eq("committed"))
                .order(items::seq.asc())
                .select(RunItemRow::as_select())
                .load::<RunItemRow>(&mut connection)
                .await
                .map_err(query_error)?;
            output.extend(
                rows.into_iter()
                    .map(RunItem::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        Ok(output)
    }

    async fn snapshot(&self, conversation_id: &str) -> Result<ChatSnapshot, StoreError> {
        let items = self.conversation_items(conversation_id).await?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let run_rows = runs::table
            .filter(runs::conversation_id.eq(conversation_id))
            .order((runs::created_at_ms.asc(), runs::id.asc()))
            .select(RunRow::as_select())
            .load::<RunRow>(&mut connection)
            .await
            .map_err(query_error)?;
        let mut events_output = Vec::new();
        let mut tools_output = Vec::new();
        let mut latest_active_run = None;
        for row in run_rows {
            let run_id = row.id.clone();
            let run = AgentRun::try_from(row)?;
            if !run.status.is_terminal() {
                latest_active_run = Some(run);
            }
            events_output.extend(
                run_events::table
                    .filter(run_events::run_id.eq(&run_id))
                    .order(run_events::seq.asc())
                    .select(RunEventRow::as_select())
                    .load::<RunEventRow>(&mut connection)
                    .await
                    .map_err(query_error)?
                    .into_iter()
                    .map(RunEvent::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            tools_output.extend(
                tool_executions::table
                    .filter(tool_executions::run_id.eq(&run_id))
                    .order(tool_executions::prepared_at_ms.asc())
                    .select(ToolExecutionRow::as_select())
                    .load::<ToolExecutionRow>(&mut connection)
                    .await
                    .map_err(query_error)?
                    .into_iter()
                    .map(ToolExecution::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        Ok(ChatSnapshot {
            conversation_id: conversation_id.to_owned(),
            active_run: latest_active_run,
            items,
            events: events_output,
            tool_executions: tools_output,
            pending_inputs: pending_inputs::table
                .inner_join(runs::table.on(runs::id.eq(pending_inputs::run_id)))
                .filter(runs::conversation_id.eq(conversation_id))
                .order(pending_inputs::created_at_ms.asc())
                .select(PendingInputRow::as_select())
                .load::<PendingInputRow>(&mut connection)
                .await
                .map_err(query_error)?
                .into_iter()
                .map(PendingInput::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            approvals: tool_approvals::table
                .inner_join(runs::table.on(runs::id.eq(tool_approvals::run_id)))
                .filter(runs::conversation_id.eq(conversation_id))
                .order(tool_approvals::requested_at_ms.asc())
                .select(ToolApprovalRow::as_select())
                .load::<ToolApprovalRow>(&mut connection)
                .await
                .map_err(query_error)?
                .into_iter()
                .map(ToolApproval::from)
                .collect(),
        })
    }

    async fn get_run(&self, run_id: &str) -> Result<Option<AgentRun>, StoreError> {
        self.load_run_row(run_id)
            .await?
            .map(AgentRun::try_from)
            .transpose()
    }

    async fn request_pause(&self, run_id: &str) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                if row.status != "running" {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                let next_seq = row.last_event_seq + 1;
                let payload = serde_json::json!({"requestedAtEventSeq": row.last_event_seq});
                let payload_json = payload.to_string();
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: next_seq,
                        kind: "run_pause_requested",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_state(
                    connection,
                    &row,
                    next_seq,
                    "pause_requested",
                    &row.phase,
                    Some("pause requested"),
                    now,
                )
                .await?;
                Ok(RunEvent {
                    id: event_id.clone(),
                    run_id: run_id.to_owned(),
                    seq: next_seq,
                    kind: "run_pause_requested".to_owned(),
                    payload,
                    persisted_at_ms: now,
                })
            })
            .await
            .map_err(transaction_error)
    }

    async fn claim_resume(
        &self,
        run_id: &str,
        runtime_instance_id: &str,
    ) -> Result<AgentRun, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                if !matches!(row.status.as_str(), "paused" | "suspended" | "interrupted") {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                let unsafe_tools = tool_executions::table
                    .filter(tool_executions::run_id.eq(run_id))
                    .filter(tool_executions::status.eq_any(["prepared", "executing"]))
                    .filter(tool_executions::risk.eq_any(["external_side_effect", "dangerous"]))
                    .select((tool_executions::id, tool_executions::status))
                    .load::<(String, String)>(connection)
                    .await?;
                for (execution_id, status) in unsafe_tools {
                    let approved = tool_approvals::table
                        .filter(tool_approvals::tool_execution_id.eq(&execution_id))
                        .filter(tool_approvals::status.eq("approved"))
                        .select(tool_approvals::id)
                        .first::<String>(connection)
                        .await
                        .optional()?
                        .is_some();
                    if status == "executing" || !approved {
                        return Err(diesel::result::Error::RollbackTransaction);
                    }
                }
                let next_seq = row.last_event_seq + 1;
                let payload = serde_json::json!({
                    "runtimeInstanceId": runtime_instance_id,
                    "fromStatus": row.status,
                });
                let payload_json = payload.to_string();
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: next_seq,
                        kind: "run_resumed",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let updated = diesel::update(
                    runs::table.filter(runs::id.eq(run_id).and(runs::version.eq(row.version))),
                )
                .set((
                    runs::status.eq("running"),
                    runs::phase.eq("routing"),
                    runs::version.eq(row.version + 1),
                    runs::last_event_seq.eq(next_seq),
                    runs::runtime_instance_id.eq(Some(runtime_instance_id)),
                    runs::lease_expires_at_ms.eq(Some(now + 30_000)),
                    runs::stop_reason.eq::<Option<&str>>(None),
                    runs::updated_at_ms.eq(now),
                    runs::completed_at_ms.eq::<Option<i64>>(None),
                ))
                .execute(connection)
                .await?;
                if updated != 1 {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                upsert_run_snapshot(connection, &row, next_seq, "running", "routing", now).await?;
                runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await
            })
            .await
            .map_err(transaction_error)
            .and_then(AgentRun::try_from)
    }

    async fn enqueue_input(
        &self,
        run_id: &str,
        intent: PendingInputIntent,
        content: serde_json::Value,
    ) -> Result<PendingInput, StoreError> {
        let now = now_ms()?;
        let id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        let content_json = serde_json::to_string(&content).map_err(invalid_serialization)?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let event_seq = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                if row.status != "running"
                    && row.status != "pause_requested"
                    && row.status != "paused"
                {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                diesel::insert_into(pending_inputs::table)
                    .values(NewPendingInputRow {
                        id: &id,
                        run_id,
                        item_id: None,
                        intent: intent.as_str(),
                        status: "pending",
                        content_json: &content_json,
                        created_at_ms: now,
                        consumed_at_ms: None,
                        child_run_id: None,
                    })
                    .execute(connection)
                    .await?;
                let event_seq = row.last_event_seq + 1;
                let payload = serde_json::json!({"pendingInputId": id, "intent": intent});
                let payload_json = payload.to_string();
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: "pending_input_enqueued",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_watermark(connection, &row, event_seq, now).await?;
                Ok(event_seq)
            })
            .await
            .map_err(transaction_error)?;
        let _ = event_seq;
        Ok(PendingInput {
            id,
            run_id: run_id.to_owned(),
            item_id: None,
            intent,
            status: "pending".to_owned(),
            content,
            created_at_ms: now,
            consumed_at_ms: None,
            child_run_id: None,
        })
    }

    async fn consume_append_inputs(&self, run_id: &str) -> Result<Vec<RunItem>, StoreError> {
        self.consume_append_inputs_transaction(run_id).await
    }

    async fn recover_expired_leases(
        &self,
        runtime_instance_id: &str,
    ) -> Result<Vec<LeaseRecovery>, StoreError> {
        let now = now_ms()?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let stale = runs::table
            .filter(runs::status.eq_any([
                "running",
                "pause_requested",
                "suspended",
                "waiting_for_approval",
            ]))
            .filter(
                runs::lease_expires_at_ms
                    .is_null()
                    .or(runs::lease_expires_at_ms.lt(now)),
            )
            .filter(
                runs::runtime_instance_id
                    .is_null()
                    .or(runs::runtime_instance_id.ne(runtime_instance_id)),
            )
            .select(RunRow::as_select())
            .load::<RunRow>(&mut connection)
            .await
            .map_err(query_error)?;
        drop(connection);
        let mut recovered = Vec::new();
        for row in stale {
            let mut connection = self.database.connection().await.map_err(unavailable)?;
            let unfinished = tool_executions::table
                .filter(tool_executions::run_id.eq(&row.id))
                .filter(tool_executions::status.eq_any(["prepared", "executing"]))
                .select(ToolExecutionRow::as_select())
                .load::<ToolExecutionRow>(&mut connection)
                .await
                .map_err(query_error)?;
            drop(connection);
            let unknown_side_effect = unfinished.iter().any(|tool| {
                tool.status == "executing"
                    && matches!(tool.risk.as_str(), "external_side_effect" | "dangerous")
            });
            if unknown_side_effect {
                let reason = "tool side effect outcome is unknown; review before continuing";
                let mut connection = self.database.connection().await.map_err(unavailable)?;
                diesel::update(
                    tool_executions::table
                        .filter(tool_executions::run_id.eq(&row.id))
                        .filter(tool_executions::status.eq("executing"))
                        .filter(
                            tool_executions::risk.eq_any(["external_side_effect", "dangerous"]),
                        ),
                )
                .set((
                    tool_executions::reconciliation_status.eq("pending"),
                    tool_executions::reconciliation_note.eq(Some(reason)),
                ))
                .execute(&mut connection)
                .await
                .map_err(query_error)?;
                drop(connection);
                self.transition(
                    &row.id,
                    RunTransition {
                        status: RunStatus::RecoveryRequired,
                        phase: RunPhase::None,
                        event_kind: "tool_reconciliation_required".to_owned(),
                        payload: serde_json::json!({"reason": reason}),
                        stop_reason: Some(reason.to_owned()),
                    },
                )
                .await?;
                recovered.push(LeaseRecovery {
                    run_id: row.id,
                    status: RunStatus::RecoveryRequired,
                    reason: reason.to_owned(),
                });
                continue;
            }
            for tool in unfinished {
                self.finish_tool(
                    &row.id,
                    &tool.id,
                    &tool.call_id,
                    ToolExecutionResult {
                        output: Some(serde_json::json!({
                            "ok": false,
                            "interrupted": true,
                            "message": "tool outcome reconciled after lease expiry",
                        })),
                        error_message: Some("tool interrupted before a durable outcome".to_owned()),
                        cancelled: true,
                    },
                )
                .await?;
            }
            let status = if row.status == "pause_requested" {
                RunStatus::Paused
            } else {
                RunStatus::Interrupted
            };
            let reason = if status == RunStatus::Paused {
                "pause completed during application recovery"
            } else {
                "previous runtime lease expired"
            };
            self.transition(
                &row.id,
                RunTransition {
                    status,
                    phase: RunPhase::None,
                    event_kind: "run_lease_recovered".to_owned(),
                    payload: serde_json::json!({"reason": reason}),
                    stop_reason: Some(reason.to_owned()),
                },
            )
            .await?;
            recovered.push(LeaseRecovery {
                run_id: row.id,
                status,
                reason: reason.to_owned(),
            });
        }
        Ok(recovered)
    }

    async fn renew_lease(
        &self,
        run_id: &str,
        runtime_instance_id: &str,
    ) -> Result<bool, StoreError> {
        let now = now_ms()?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        diesel::update(
            runs::table
                .filter(runs::id.eq(run_id))
                .filter(runs::runtime_instance_id.eq(runtime_instance_id))
                .filter(runs::status.eq_any([
                    "running",
                    "pause_requested",
                    "suspended",
                    "waiting_for_approval",
                ])),
        )
        .set((
            runs::lease_expires_at_ms.eq(Some(now + 30_000)),
            runs::updated_at_ms.eq(now),
        ))
        .execute(&mut connection)
        .await
        .map(|updated| updated == 1)
        .map_err(query_error)
    }

    async fn request_tool_approval(
        &self,
        run_id: &str,
        execution_id: &str,
    ) -> Result<ToolApproval, StoreError> {
        let now = now_ms()?;
        let approval_id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let execution = tool_executions::table
                    .filter(tool_executions::id.eq(execution_id))
                    .filter(tool_executions::run_id.eq(run_id))
                    .filter(tool_executions::status.eq("prepared"))
                    .select(ToolExecutionRow::as_select())
                    .first::<ToolExecutionRow>(connection)
                    .await?;
                let next_seq = row.last_event_seq + 1;
                diesel::insert_into(tool_approvals::table)
                    .values(NewToolApprovalRow {
                        id: &approval_id,
                        run_id,
                        tool_execution_id: execution_id,
                        call_id: &execution.call_id,
                        arguments_hash: &execution.arguments_hash,
                        status: "pending",
                        requested_at_ms: now,
                        resolved_at_ms: None,
                    })
                    .execute(connection)
                    .await?;
                let payload = serde_json::json!({
                    "approvalId": approval_id,
                    "toolExecutionId": execution_id,
                    "callId": execution.call_id,
                    "toolName": execution.tool_name,
                    "risk": execution.risk,
                    "argumentsHash": execution.arguments_hash,
                });
                let payload_json = payload.to_string();
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: next_seq,
                        kind: "tool_approval_requested",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_state(
                    connection,
                    &row,
                    next_seq,
                    "waiting_for_approval",
                    "tool_prepare",
                    Some("tool approval required"),
                    now,
                )
                .await?;
                Ok(ToolApproval {
                    id: approval_id.clone(),
                    run_id: run_id.to_owned(),
                    tool_execution_id: execution_id.to_owned(),
                    call_id: execution.call_id,
                    arguments_hash: execution.arguments_hash,
                    status: "pending".to_owned(),
                    requested_at_ms: now,
                    resolved_at_ms: None,
                })
            })
            .await
            .map_err(transaction_error)
    }

    async fn resolve_tool_approval(
        &self,
        run_id: &str,
        execution_id: &str,
        approved: bool,
    ) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let status = if approved { "approved" } else { "denied" };
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .filter(runs::status.eq("waiting_for_approval"))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let updated = diesel::update(
                    tool_approvals::table
                        .filter(tool_approvals::run_id.eq(run_id))
                        .filter(tool_approvals::tool_execution_id.eq(execution_id))
                        .filter(tool_approvals::status.eq("pending")),
                )
                .set((
                    tool_approvals::status.eq(status),
                    tool_approvals::resolved_at_ms.eq(Some(now)),
                ))
                .execute(connection)
                .await?;
                if updated != 1 {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                if !approved {
                    let output = serde_json::json!({
                        "ok": false,
                        "error": "tool execution denied by user",
                    });
                    let output_json = output.to_string();
                    diesel::update(
                        tool_executions::table
                            .filter(tool_executions::id.eq(execution_id))
                            .filter(tool_executions::status.eq("prepared")),
                    )
                    .set((
                        tool_executions::status.eq("cancelled"),
                        tool_executions::output_json.eq(Some(&output_json)),
                        tool_executions::error_message.eq(Some("tool execution denied by user")),
                        tool_executions::completed_at_ms.eq(Some(now)),
                    ))
                    .execute(connection)
                    .await?;
                    let execution = tool_executions::table
                        .filter(tool_executions::id.eq(execution_id))
                        .select(ToolExecutionRow::as_select())
                        .first::<ToolExecutionRow>(connection)
                        .await?;
                    let item_id = Uuid::now_v7().to_string();
                    let item_seq = next_item_seq(connection, run_id).await?;
                    diesel::insert_into(items::table)
                        .values(NewRunItemRow {
                            id: &item_id,
                            run_id,
                            seq: item_seq,
                            kind: "function_call_output",
                            role: Some("tool"),
                            status: "committed",
                            content_json: &output_json,
                            provider_item_id: None,
                            call_id: Some(&execution.call_id),
                            created_at_ms: now,
                        })
                        .execute(connection)
                        .await?;
                }
                let next_seq = row.last_event_seq + 1;
                let payload = serde_json::json!({
                    "toolExecutionId": execution_id,
                    "decision": status,
                });
                let payload_json = payload.to_string();
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: next_seq,
                        kind: "tool_approval_resolved",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_state(
                    connection,
                    &row,
                    next_seq,
                    if approved { "paused" } else { "interrupted" },
                    "none",
                    Some(if approved {
                        "approved tool ready to resume"
                    } else {
                        "tool execution denied"
                    }),
                    now,
                )
                .await?;
                Ok(RunEvent {
                    id: event_id.clone(),
                    run_id: run_id.to_owned(),
                    seq: next_seq,
                    kind: "tool_approval_resolved".to_owned(),
                    payload,
                    persisted_at_ms: now,
                })
            })
            .await
            .map_err(transaction_error)
    }

    async fn pending_tool_executions(
        &self,
        run_id: &str,
    ) -> Result<Vec<ToolExecution>, StoreError> {
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        tool_executions::table
            .inner_join(
                tool_approvals::table.on(tool_approvals::tool_execution_id.eq(tool_executions::id)),
            )
            .filter(tool_executions::run_id.eq(run_id))
            .filter(tool_executions::status.eq("prepared"))
            .filter(tool_approvals::status.eq("approved"))
            .order(tool_executions::prepared_at_ms.asc())
            .select(ToolExecutionRow::as_select())
            .load::<ToolExecutionRow>(&mut connection)
            .await
            .map_err(query_error)?
            .into_iter()
            .map(ToolExecution::try_from)
            .collect()
    }

    async fn resolve_recovery(
        &self,
        run_id: &str,
        execution_id: &str,
        resolution: RecoveryResolution,
        note: Option<String>,
    ) -> Result<RunEvent, StoreError> {
        self.resolve_recovery_transaction(run_id, execution_id, resolution, note)
            .await
    }

    async fn get_pending_input(&self, input_id: &str) -> Result<Option<PendingInput>, StoreError> {
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        pending_inputs::table
            .filter(pending_inputs::id.eq(input_id))
            .select(PendingInputRow::as_select())
            .first::<PendingInputRow>(&mut connection)
            .await
            .optional()
            .map_err(query_error)?
            .map(PendingInput::try_from)
            .transpose()
    }
}

impl SqliteRunStore {
    async fn resolve_recovery_transaction(
        &self,
        run_id: &str,
        execution_id: &str,
        resolution: RecoveryResolution,
        note: Option<String>,
    ) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let event_id = Uuid::now_v7().to_string();
        let item_id = Uuid::now_v7().to_string();
        let resolution_value = resolution.as_str();
        let tool_status = match resolution {
            RecoveryResolution::MarkSucceeded => "succeeded",
            RecoveryResolution::MarkFailed => "failed",
            RecoveryResolution::Abandon => "cancelled",
        };
        let run_status = if resolution == RecoveryResolution::Abandon {
            RunStatus::Cancelled
        } else {
            RunStatus::Interrupted
        };
        let output = serde_json::json!({
            "reconciled": true,
            "resolution": resolution,
            "note": note.clone(),
        });
        let output_json = output.to_string();
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .filter(runs::status.eq("recovery_required"))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let execution = tool_executions::table
                    .filter(tool_executions::id.eq(execution_id))
                    .filter(tool_executions::run_id.eq(run_id))
                    .filter(tool_executions::reconciliation_status.eq("pending"))
                    .select(ToolExecutionRow::as_select())
                    .first::<ToolExecutionRow>(connection)
                    .await?;
                diesel::update(tool_executions::table.filter(tool_executions::id.eq(execution_id)))
                    .set((
                        tool_executions::status.eq(tool_status),
                        tool_executions::output_json.eq(Some(&output_json)),
                        tool_executions::error_message.eq(if tool_status == "failed" {
                            Some("marked failed during reconciliation")
                        } else {
                            None
                        }),
                        tool_executions::completed_at_ms.eq(Some(now)),
                        tool_executions::reconciliation_status.eq(resolution_value),
                        tool_executions::reconciliation_note.eq(note.as_deref()),
                    ))
                    .execute(connection)
                    .await?;
                let item_seq = next_item_seq(connection, run_id).await?;
                diesel::insert_into(items::table)
                    .values(NewRunItemRow {
                        id: &item_id,
                        run_id,
                        seq: item_seq,
                        kind: "function_call_output",
                        role: Some("tool"),
                        status: "committed",
                        content_json: &output_json,
                        provider_item_id: None,
                        call_id: Some(&execution.call_id),
                        created_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let next_seq = row.last_event_seq + 1;
                let payload = serde_json::json!({
                    "toolExecutionId": execution_id,
                    "resolution": resolution,
                    "itemId": item_id,
                });
                let payload_json = payload.to_string();
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: next_seq,
                        kind: "tool_reconciliation_resolved",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                update_run_state(
                    connection,
                    &row,
                    next_seq,
                    run_status.as_str(),
                    "none",
                    Some("tool reconciliation resolved"),
                    now,
                )
                .await?;
                Ok(RunEvent {
                    id: event_id.clone(),
                    run_id: run_id.to_owned(),
                    seq: next_seq,
                    kind: "tool_reconciliation_resolved".to_owned(),
                    payload,
                    persisted_at_ms: now,
                })
            })
            .await
            .map_err(transaction_error)
    }

    async fn consume_append_inputs_transaction(
        &self,
        run_id: &str,
    ) -> Result<Vec<RunItem>, StoreError> {
        let now = now_ms()?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let mut row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let inputs = pending_inputs::table
                    .filter(pending_inputs::run_id.eq(run_id))
                    .filter(pending_inputs::status.eq("pending"))
                    .filter(pending_inputs::intent.eq("append"))
                    .order(pending_inputs::created_at_ms.asc())
                    .select(PendingInputRow::as_select())
                    .load::<PendingInputRow>(connection)
                    .await?;
                let mut next_seq = next_item_seq(connection, run_id).await?;
                let mut output = Vec::new();
                for input in inputs {
                    let item_id = Uuid::now_v7().to_string();
                    let event_id = Uuid::now_v7().to_string();
                    let event_seq = row.last_event_seq + 1;
                    diesel::insert_into(items::table)
                        .values(NewRunItemRow {
                            id: &item_id,
                            run_id,
                            seq: next_seq,
                            kind: "message",
                            role: Some("user"),
                            status: "committed",
                            content_json: &input.content_json,
                            provider_item_id: None,
                            call_id: None,
                            created_at_ms: now,
                        })
                        .execute(connection)
                        .await?;
                    diesel::update(pending_inputs::table.filter(pending_inputs::id.eq(&input.id)))
                        .set((
                            pending_inputs::status.eq("consumed"),
                            pending_inputs::item_id.eq(Some(&item_id)),
                            pending_inputs::consumed_at_ms.eq(Some(now)),
                        ))
                        .execute(connection)
                        .await?;
                    let payload = serde_json::json!({
                        "pendingInputId": input.id,
                        "itemId": item_id,
                        "itemSeq": next_seq,
                    });
                    let payload_json = payload.to_string();
                    diesel::insert_into(run_events::table)
                        .values(NewRunEventRow {
                            id: &event_id,
                            run_id,
                            seq: event_seq,
                            kind: "pending_input_consumed",
                            payload_json: &payload_json,
                            persisted_at_ms: now,
                        })
                        .execute(connection)
                        .await?;
                    update_run_watermark(connection, &row, event_seq, now).await?;
                    output.push(RunItem {
                        id: item_id,
                        run_id: run_id.to_owned(),
                        seq: next_seq,
                        kind: "message".to_owned(),
                        role: Some("user".to_owned()),
                        status: "committed".to_owned(),
                        content: serde_json::from_str(&input.content_json)
                            .map_err(|_| diesel::result::Error::RollbackTransaction)?,
                        provider_item_id: None,
                        call_id: None,
                        created_at_ms: now,
                    });
                    row.version += 1;
                    row.last_event_seq = event_seq;
                    row.updated_at_ms = now;
                    next_seq += 1;
                }
                Ok(output)
            })
            .await
            .map_err(transaction_error)
    }

    async fn prepare_tool_transaction(
        &self,
        run_id: &str,
        execution: NewToolExecution,
    ) -> Result<CommitResult, StoreError> {
        let now = now_ms()?;
        let item_id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        let arguments_json =
            serde_json::to_string(&execution.arguments).map_err(invalid_serialization)?;
        let definition_snapshot_json =
            serde_json::to_string(&execution.definition_snapshot).map_err(invalid_serialization)?;
        let policy_snapshot_json =
            serde_json::to_string(&execution.policy_snapshot).map_err(invalid_serialization)?;
        let payload = serde_json::json!({
            "toolExecutionId": execution.id,
            "callId": execution.call_id,
            "name": execution.tool_name,
            "arguments": execution.arguments,
        });
        let payload_json = serde_json::to_string(&payload).map_err(invalid_serialization)?;
        let item_content = serde_json::json!({
            "name": execution.tool_name,
            "arguments": execution.arguments,
        });
        let item_content_json =
            serde_json::to_string(&item_content).map_err(invalid_serialization)?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let (item_seq, event_seq) = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let item_seq = next_item_seq(connection, run_id).await?;
                let event_seq = row.last_event_seq + 1;
                diesel::insert_into(tool_executions::table)
                    .values(NewToolExecutionRow {
                        id: &execution.id,
                        run_id,
                        call_id: &execution.call_id,
                        tool_name: &execution.tool_name,
                        status: "prepared",
                        risk: &execution.risk,
                        arguments_json: &arguments_json,
                        arguments_hash: &execution.arguments_hash,
                        approval_preview: execution.approval_preview.as_deref(),
                        output_json: None,
                        error_message: None,
                        retryable: execution.retryable,
                        prepared_at_ms: now,
                        started_at_ms: None,
                        completed_at_ms: None,
                        idempotency_key: execution.idempotency_key.as_deref(),
                        reconciliation_status: "not_required",
                        reconciliation_note: None,
                        source_kind: &execution.identity.source_kind,
                        source_server_id: execution.identity.source_server_id.as_deref(),
                        remote_tool_name: execution.identity.remote_tool_name.as_deref(),
                        tool_schema_hash: &execution.identity.schema_hash,
                        tool_definition_snapshot_json: &definition_snapshot_json,
                        tool_policy_snapshot_json: &policy_snapshot_json,
                    })
                    .execute(connection)
                    .await?;
                diesel::insert_into(items::table)
                    .values(NewRunItemRow {
                        id: &item_id,
                        run_id,
                        seq: item_seq,
                        kind: "function_call",
                        role: Some("assistant"),
                        status: "committed",
                        content_json: &item_content_json,
                        provider_item_id: None,
                        call_id: Some(&execution.call_id),
                        created_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: "tool_call_prepared",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let status = active_status(&row);
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    status,
                    "tool_prepare",
                    None,
                    now,
                )
                .await?;
                Ok((item_seq, event_seq))
            })
            .await
            .map_err(transaction_error)?;
        Ok(CommitResult {
            item: Some(RunItem {
                id: item_id,
                run_id: run_id.to_owned(),
                seq: item_seq,
                kind: "function_call".to_owned(),
                role: Some("assistant".to_owned()),
                status: "committed".to_owned(),
                content: item_content,
                provider_item_id: None,
                call_id: Some(execution.call_id),
                created_at_ms: now,
            }),
            event: RunEvent {
                id: event_id,
                run_id: run_id.to_owned(),
                seq: event_seq,
                kind: "tool_call_prepared".to_owned(),
                payload,
                persisted_at_ms: now,
            },
        })
    }

    async fn finish_tool_transaction(
        &self,
        run_id: &str,
        execution_id: &str,
        call_id: &str,
        result: ToolExecutionResult,
    ) -> Result<CommitResult, StoreError> {
        let now = now_ms()?;
        let item_id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        let status = if result.cancelled {
            "cancelled"
        } else if result.error_message.is_some() {
            "failed"
        } else {
            "succeeded"
        };
        let output = result.output.unwrap_or_else(|| {
            serde_json::json!({"error": result.error_message.clone().unwrap_or_else(|| "cancelled".to_owned())})
        });
        let output_json = serde_json::to_string(&output).map_err(invalid_serialization)?;
        let payload = serde_json::json!({
            "toolExecutionId": execution_id,
            "callId": call_id,
            "status": status,
            "output": output,
        });
        let payload_json = serde_json::to_string(&payload).map_err(invalid_serialization)?;
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let (item_seq, event_seq) = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let updated = diesel::update(
                    tool_executions::table.filter(
                        tool_executions::id
                            .eq(execution_id)
                            .and(tool_executions::run_id.eq(run_id)),
                    ),
                )
                .set((
                    tool_executions::status.eq(status),
                    tool_executions::output_json.eq(Some(&output_json)),
                    tool_executions::error_message.eq(result.error_message.as_deref()),
                    tool_executions::completed_at_ms.eq(Some(now)),
                ))
                .execute(connection)
                .await?;
                if updated != 1 {
                    return Err(diesel::result::Error::RollbackTransaction);
                }
                let item_seq = next_item_seq(connection, run_id).await?;
                let event_seq = row.last_event_seq + 1;
                diesel::insert_into(items::table)
                    .values(NewRunItemRow {
                        id: &item_id,
                        run_id,
                        seq: item_seq,
                        kind: "function_call_output",
                        role: Some("tool"),
                        status: "committed",
                        content_json: &output_json,
                        provider_item_id: None,
                        call_id: Some(call_id),
                        created_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: "tool_output_committed",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let status = active_status(&row);
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    status,
                    "observation_commit",
                    None,
                    now,
                )
                .await?;
                Ok((item_seq, event_seq))
            })
            .await
            .map_err(transaction_error)?;
        Ok(CommitResult {
            item: Some(RunItem {
                id: item_id,
                run_id: run_id.to_owned(),
                seq: item_seq,
                kind: "function_call_output".to_owned(),
                role: Some("tool".to_owned()),
                status: "committed".to_owned(),
                content: output,
                provider_item_id: None,
                call_id: Some(call_id.to_owned()),
                created_at_ms: now,
            }),
            event: RunEvent {
                id: event_id,
                run_id: run_id.to_owned(),
                seq: event_seq,
                kind: "tool_output_committed".to_owned(),
                payload,
                persisted_at_ms: now,
            },
        })
    }

    async fn create_plan_transaction(
        &self,
        run_id: &str,
        plan: PlanDraft,
    ) -> Result<RunEvent, StoreError> {
        let now = now_ms()?;
        let plan_id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        let step_ids = plan
            .steps
            .iter()
            .map(|_| Uuid::now_v7().to_string())
            .collect::<Vec<_>>();
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let (event_seq, payload) = connection
            .transaction::<_, diesel::result::Error, _>(async |connection| {
                let row = runs::table
                    .filter(runs::id.eq(run_id))
                    .select(RunRow::as_select())
                    .first::<RunRow>(connection)
                    .await?;
                let event_seq = row.last_event_seq + 1;
                let revision = plans::table
                    .filter(plans::run_id.eq(run_id))
                    .select(max(plans::revision))
                    .first::<Option<i64>>(connection)
                    .await?
                    .unwrap_or(0)
                    + 1;
                diesel::update(
                    plans::table
                        .filter(plans::run_id.eq(run_id))
                        .filter(plans::status.eq("active")),
                )
                .set(plans::status.eq("superseded"))
                .execute(connection)
                .await?;
                let payload = serde_json::json!({
                    "planId": plan_id,
                    "revision": revision,
                    "goal": plan.goal,
                    "steps": plan.steps.iter().enumerate().map(|(index, (title, acceptance))| serde_json::json!({
                        "ordinal": index + 1,
                        "title": title,
                        "acceptance": acceptance,
                    })).collect::<Vec<_>>(),
                });
                let payload_json = payload.to_string();
                diesel::insert_into(plans::table)
                    .values(NewPlanRow {
                        id: &plan_id,
                        run_id,
                        revision,
                        goal: &plan.goal,
                        status: "active",
                        created_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                for (index, ((title, acceptance), step_id)) in
                    plan.steps.iter().zip(step_ids.iter()).enumerate()
                {
                    diesel::insert_into(plan_steps::table)
                        .values(NewPlanStepRow {
                            id: step_id,
                            plan_id: &plan_id,
                            ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
                            title,
                            acceptance,
                            status: "pending",
                            attempt: 0,
                            created_at_ms: now,
                            updated_at_ms: now,
                        })
                        .execute(connection)
                        .await?;
                }
                diesel::insert_into(run_events::table)
                    .values(NewRunEventRow {
                        id: &event_id,
                        run_id,
                        seq: event_seq,
                        kind: "plan_created",
                        payload_json: &payload_json,
                        persisted_at_ms: now,
                    })
                    .execute(connection)
                    .await?;
                let status = active_status(&row);
                update_run_state(connection, &row, event_seq, status, "planning", None, now)
                    .await?;
                Ok((event_seq, payload))
            })
            .await
            .map_err(transaction_error)?;
        Ok(RunEvent {
            id: event_id,
            run_id: run_id.to_owned(),
            seq: event_seq,
            kind: "plan_created".to_owned(),
            payload,
            persisted_at_ms: now,
        })
    }
}

async fn next_item_seq(
    connection: &mut super::database::DbConnection,
    run_id: &str,
) -> Result<i64, diesel::result::Error> {
    Ok(items::table
        .filter(items::run_id.eq(run_id))
        .select(max(items::seq))
        .first::<Option<i64>>(connection)
        .await?
        .unwrap_or(0)
        + 1)
}

async fn commit_parent_branch_state(
    connection: &mut super::database::DbConnection,
    row: &RunRow,
    event_id: &str,
    status: &str,
    event_kind: &str,
    child_run_id: &str,
    now: i64,
) -> Result<(), diesel::result::Error> {
    let event_seq = row.last_event_seq + 1;
    let payload = serde_json::json!({"childRunId": child_run_id});
    let payload_json = payload.to_string();
    diesel::insert_into(run_events::table)
        .values(NewRunEventRow {
            id: event_id,
            run_id: &row.id,
            seq: event_seq,
            kind: event_kind,
            payload_json: &payload_json,
            persisted_at_ms: now,
        })
        .execute(connection)
        .await?;
    update_run_state(
        connection,
        row,
        event_seq,
        status,
        "none",
        Some(event_kind),
        now,
    )
    .await
}

fn active_status(row: &RunRow) -> &str {
    if row.status == "pause_requested" {
        "pause_requested"
    } else {
        "running"
    }
}

async fn update_run_watermark(
    connection: &mut super::database::DbConnection,
    row: &RunRow,
    event_seq: i64,
    now: i64,
) -> Result<(), diesel::result::Error> {
    update_run_state(
        connection,
        row,
        event_seq,
        &row.status,
        &row.phase,
        row.stop_reason.as_deref(),
        now,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_run_state(
    connection: &mut super::database::DbConnection,
    row: &RunRow,
    event_seq: i64,
    status: &str,
    phase: &str,
    stop_reason: Option<&str>,
    now: i64,
) -> Result<(), diesel::result::Error> {
    let updated =
        diesel::update(runs::table.filter(runs::id.eq(&row.id).and(runs::version.eq(row.version))))
            .set(RunChangeset {
                status,
                phase,
                version: row.version + 1,
                last_event_seq: event_seq,
                stop_reason,
                lease_expires_at_ms: Some(now + 30_000),
                updated_at_ms: now,
                completed_at_ms: None,
            })
            .execute(connection)
            .await?;
    if updated != 1 {
        return Err(diesel::result::Error::RollbackTransaction);
    }
    upsert_run_snapshot(connection, row, event_seq, status, phase, now).await?;
    Ok(())
}

async fn upsert_run_snapshot(
    connection: &mut super::database::DbConnection,
    row: &RunRow,
    event_seq: i64,
    status: &str,
    phase: &str,
    now: i64,
) -> Result<(), diesel::result::Error> {
    let state_json = serde_json::json!({
        "status": status,
        "phase": phase,
        "version": row.version + 1,
        "runtimeInstanceId": row.runtime_instance_id,
    })
    .to_string();
    diesel::insert_into(run_snapshots::table)
        .values(NewRunSnapshotRow {
            run_id: &row.id,
            event_high_water_seq: event_seq,
            state_json: &state_json,
            updated_at_ms: now,
        })
        .on_conflict(run_snapshots::run_id)
        .do_update()
        .set((
            run_snapshots::event_high_water_seq.eq(event_seq),
            run_snapshots::state_json.eq(&state_json),
            run_snapshots::updated_at_ms.eq(now),
        ))
        .execute(connection)
        .await?;
    Ok(())
}

fn unavailable(error: super::DatabaseError) -> StoreError {
    StoreError::Unavailable {
        message: error.to_string(),
    }
}

fn query_error(error: diesel::result::Error) -> StoreError {
    StoreError::Unavailable {
        message: error.to_string(),
    }
}

fn transaction_error(error: diesel::result::Error) -> StoreError {
    if matches!(error, diesel::result::Error::RollbackTransaction) {
        StoreError::Conflict
    } else {
        query_error(error)
    }
}

fn invalid_serialization(error: serde_json::Error) -> StoreError {
    StoreError::InvalidData {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::conversation::NewConversation;
    use crate::domain::provider::{
        ProviderCapabilities, ProviderKind, ProviderProfile, ProviderProtocol,
    };
    use crate::domain::run::{
        NewRun, NewToolExecution, PendingInputIntent, RecoveryResolution, RunPhase, RunStatus,
        RunTransition,
    };
    use crate::domain::settings::RunStrategy;
    use crate::domain::storage::{ConversationStore, RunStore};
    use crate::persistence::{Database, SqliteConversationStore};

    use super::SqliteRunStore;

    #[tokio::test(flavor = "multi_thread")]
    async fn persists_pause_resume_and_consumes_append_once() {
        let (store, conversation_id) = setup("resume").await;
        store
            .start(new_run("run-1", &conversation_id))
            .await
            .unwrap();
        store
            .enqueue_input(
                "run-1",
                PendingInputIntent::Append,
                serde_json::json!({
                    "role": "user",
                    "content": [{"type": "text", "text": "Follow-up"}],
                }),
            )
            .await
            .unwrap();
        assert_eq!(store.consume_append_inputs("run-1").await.unwrap().len(), 1);
        assert!(
            store
                .consume_append_inputs("run-1")
                .await
                .unwrap()
                .is_empty()
        );
        store
            .enqueue_input(
                "run-1",
                PendingInputIntent::Append,
                serde_json::json!({
                    "role": "user",
                    "content": [{"type": "text", "text": "Late follow-up"}],
                }),
            )
            .await
            .unwrap();
        assert!(
            store
                .transition(
                    "run-1",
                    RunTransition {
                        status: RunStatus::Completed,
                        phase: RunPhase::None,
                        event_kind: "run_completed".to_owned(),
                        payload: serde_json::json!({}),
                        stop_reason: Some("completed".to_owned()),
                    },
                )
                .await
                .is_err()
        );
        assert_eq!(store.consume_append_inputs("run-1").await.unwrap().len(), 1);

        let pause_event = store.request_pause("run-1").await.unwrap();
        assert_eq!(pause_event.kind, "run_pause_requested");
        assert_eq!(
            store.get_run("run-1").await.unwrap().unwrap().status,
            RunStatus::PauseRequested
        );
        store
            .transition(
                "run-1",
                RunTransition {
                    status: RunStatus::Paused,
                    phase: RunPhase::None,
                    event_kind: "run_paused".to_owned(),
                    payload: serde_json::json!({}),
                    stop_reason: Some("paused".to_owned()),
                },
            )
            .await
            .unwrap();
        let resumed = store.claim_resume("run-1", "runtime-2").await.unwrap();
        assert_eq!(resumed.status, RunStatus::Running);
        assert_eq!(resumed.runtime_instance_id.as_deref(), Some("runtime-2"));
        assert!(store.claim_resume("run-1", "runtime-3").await.is_err());

        let snapshot = store.snapshot(&conversation_id).await.unwrap();
        assert_eq!(snapshot.pending_inputs.len(), 2);
        assert!(
            snapshot
                .pending_inputs
                .iter()
                .all(|input| input.status == "consumed")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn expired_external_side_effect_requires_manual_recovery() {
        let (store, conversation_id) = setup("reconcile").await;
        store
            .start(new_run("run-unsafe", &conversation_id))
            .await
            .unwrap();
        store
            .prepare_tool(
                "run-unsafe",
                NewToolExecution {
                    id: "execution-1".to_owned(),
                    call_id: "call-1".to_owned(),
                    tool_name: "send_message".to_owned(),
                    risk: "external_side_effect".to_owned(),
                    arguments: serde_json::json!({"message": "hello"}),
                    arguments_hash: "hash".to_owned(),
                    approval_preview: None,
                    retryable: false,
                    idempotency_key: Some("send_message:hash".to_owned()),
                    identity: crate::tools::ToolIdentity {
                        source_kind: "built_in".to_owned(),
                        source_server_id: None,
                        remote_tool_name: None,
                        schema_hash: "schema".to_owned(),
                    },
                    definition_snapshot: crate::providers::runtime::ToolDefinition {
                        name: "send_message".to_owned(),
                        description: "test".to_owned(),
                        parameters: serde_json::json!({"type": "object"}),
                        strict: true,
                    },
                    policy_snapshot: crate::tools::ToolCapabilities {
                        risk: crate::tools::ToolRisk::ExternalSideEffect,
                        idempotent: false,
                        cancellable: true,
                        reconcile: false,
                    },
                },
            )
            .await
            .unwrap();
        store
            .mark_tool_executing("run-unsafe", "execution-1")
            .await
            .unwrap();
        {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            let mut connection = store.database.connection().await.unwrap();
            diesel::update(crate::persistence::schema::runs::table)
                .set(crate::persistence::schema::runs::lease_expires_at_ms.eq(Some(0_i64)))
                .execute(&mut connection)
                .await
                .unwrap();
        }

        let recovered = store.recover_expired_leases("runtime-2").await.unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, RunStatus::RecoveryRequired);
        let run = store.get_run("run-unsafe").await.unwrap().unwrap();
        assert_eq!(run.status, RunStatus::RecoveryRequired);
        assert!(store.claim_resume("run-unsafe", "runtime-2").await.is_err());

        store
            .resolve_recovery(
                "run-unsafe",
                "execution-1",
                RecoveryResolution::MarkSucceeded,
                Some("verified against the upstream operation log".to_owned()),
            )
            .await
            .unwrap();
        let resumed = store.claim_resume("run-unsafe", "runtime-2").await.unwrap();
        assert_eq!(resumed.status, RunStatus::Running);
        let snapshot = store.snapshot(&conversation_id).await.unwrap();
        assert_eq!(snapshot.tool_executions[0].status, "succeeded");
        assert_eq!(
            snapshot.tool_executions[0].reconciliation_status,
            "resolved_succeeded"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn forced_exit_after_pause_request_recovers_as_paused() {
        let (store, conversation_id) = setup("forced-pause").await;
        store
            .start(new_run("run-sleep", &conversation_id))
            .await
            .unwrap();
        store.request_pause("run-sleep").await.unwrap();
        {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            let mut connection = store.database.connection().await.unwrap();
            diesel::update(crate::persistence::schema::runs::table)
                .set(crate::persistence::schema::runs::lease_expires_at_ms.eq(Some(0_i64)))
                .execute(&mut connection)
                .await
                .unwrap();
        }

        let recovered = store
            .recover_expired_leases("runtime-after-wake")
            .await
            .unwrap();
        assert_eq!(recovered[0].status, RunStatus::Paused);
        assert_eq!(
            store.get_run("run-sleep").await.unwrap().unwrap().status,
            RunStatus::Paused
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fork_consumes_attachment_inbox_and_preserves_parent() {
        let (store, conversation_id) = setup("fork").await;
        store
            .start(new_run("parent", &conversation_id))
            .await
            .unwrap();
        pause_run(&store, "parent").await;
        let content = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "Try another direction"},
                {"type": "image_data_url", "data_url": "data:image/png;base64,AA==", "detail": "auto"}
            ],
        });
        let pending = store
            .enqueue_input("parent", PendingInputIntent::Fork, content.clone())
            .await
            .unwrap();
        let mut child = new_run("child", &conversation_id);
        child.parent_run_id = Some("parent".to_owned());
        child.source_pending_input_id = Some(pending.id.clone());
        child.user_content = content;
        store.start(child).await.unwrap();

        assert_eq!(
            store.get_run("parent").await.unwrap().unwrap().status,
            RunStatus::Suspended
        );
        let consumed = store.get_pending_input(&pending.id).await.unwrap().unwrap();
        assert_eq!(consumed.status, "consumed");
        assert_eq!(consumed.child_run_id.as_deref(), Some("child"));
        let items = store.conversation_items(&conversation_id).await.unwrap();
        assert!(items.iter().any(|item| {
            item.run_id == "child" && item.content["content"][1]["type"] == "image_data_url"
        }));
        store
            .transition(
                "child",
                RunTransition {
                    status: RunStatus::Completed,
                    phase: RunPhase::None,
                    event_kind: "run_completed".to_owned(),
                    payload: serde_json::json!({}),
                    stop_reason: Some("completed".to_owned()),
                },
            )
            .await
            .unwrap();
        let snapshot = store.snapshot(&conversation_id).await.unwrap();
        assert_eq!(snapshot.active_run.unwrap().id, "parent");
        assert_eq!(
            store
                .claim_resume("parent", "runtime-2")
                .await
                .unwrap()
                .status,
            RunStatus::Running
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_and_replace_supersedes_parent_from_exact_inbox_record() {
        let (store, conversation_id) = setup("replace-inbox").await;
        store
            .start(new_run("parent", &conversation_id))
            .await
            .unwrap();
        pause_run(&store, "parent").await;
        let content = serde_json::json!({
            "role": "user",
            "content": [{"type": "text", "text": "Corrected request"}],
        });
        let pending = store
            .enqueue_input(
                "parent",
                PendingInputIntent::CancelAndReplace,
                content.clone(),
            )
            .await
            .unwrap();
        let mut replacement = new_run("replacement", &conversation_id);
        replacement.replaces_run_id = Some("parent".to_owned());
        replacement.source_pending_input_id = Some(pending.id.clone());
        replacement.user_content = content;
        store.start(replacement).await.unwrap();

        assert_eq!(
            store.get_run("parent").await.unwrap().unwrap().status,
            RunStatus::Cancelled
        );
        let items = store.conversation_items(&conversation_id).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].run_id, "replacement");
        assert_eq!(
            store
                .get_pending_input(&pending.id)
                .await
                .unwrap()
                .unwrap()
                .child_run_id
                .as_deref(),
            Some("replacement")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn approval_is_durable_and_releases_only_the_matching_tool() {
        let (store, conversation_id) = setup("approval").await;
        store
            .start(new_run("run-approval", &conversation_id))
            .await
            .unwrap();
        store
            .prepare_tool(
                "run-approval",
                NewToolExecution {
                    id: "write-1".to_owned(),
                    call_id: "call-write".to_owned(),
                    tool_name: "write_file".to_owned(),
                    risk: "dangerous".to_owned(),
                    arguments: serde_json::json!({"path": "report.txt"}),
                    arguments_hash: "write-hash".to_owned(),
                    approval_preview: Some(
                        "--- a/report.txt\n+++ b/report.txt\n+approved\n".to_owned(),
                    ),
                    retryable: true,
                    idempotency_key: Some("write_file:report-v1".to_owned()),
                    identity: crate::tools::ToolIdentity {
                        source_kind: "built_in".to_owned(),
                        source_server_id: None,
                        remote_tool_name: None,
                        schema_hash: "schema".to_owned(),
                    },
                    definition_snapshot: crate::providers::runtime::ToolDefinition {
                        name: "write_file".to_owned(),
                        description: "test".to_owned(),
                        parameters: serde_json::json!({"type": "object"}),
                        strict: true,
                    },
                    policy_snapshot: crate::tools::ToolCapabilities {
                        risk: crate::tools::ToolRisk::Dangerous,
                        idempotent: true,
                        cancellable: true,
                        reconcile: true,
                    },
                },
            )
            .await
            .unwrap();
        store
            .request_tool_approval("run-approval", "write-1")
            .await
            .unwrap();
        let snapshot = store.snapshot(&conversation_id).await.unwrap();
        assert_eq!(snapshot.approvals[0].status, "pending");
        assert!(
            snapshot.tool_executions[0]
                .approval_preview
                .as_deref()
                .is_some_and(|preview| preview.contains("+approved"))
        );
        assert_eq!(
            snapshot.active_run.unwrap().status,
            RunStatus::WaitingForApproval
        );

        store
            .resolve_tool_approval("run-approval", "write-1", true)
            .await
            .unwrap();
        store
            .claim_resume("run-approval", "runtime-2")
            .await
            .unwrap();
        let executable = store.pending_tool_executions("run-approval").await.unwrap();
        assert_eq!(executable.len(), 1);
        assert_eq!(
            executable[0].idempotency_key.as_deref(),
            Some("write_file:report-v1")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_multimodal_run_does_not_poison_later_context() {
        let (store, conversation_id) = setup("failed-image").await;
        let mut failed = new_run("run-image", &conversation_id);
        failed.user_content = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "Describe this image"},
                {"type": "image_data_url", "data_url": "data:image/png;base64,AA==", "detail": "auto"}
            ]
        });
        store.start(failed).await.unwrap();
        store
            .transition(
                "run-image",
                RunTransition {
                    status: RunStatus::Failed,
                    phase: RunPhase::None,
                    event_kind: "run_failed".to_owned(),
                    payload: serde_json::json!({"reason": "unsupported multimodal request"}),
                    stop_reason: Some("model does not support images".to_owned()),
                },
            )
            .await
            .unwrap();

        assert!(
            store
                .conversation_items(&conversation_id)
                .await
                .unwrap()
                .is_empty()
        );

        store
            .start(new_run("run-text", &conversation_id))
            .await
            .unwrap();
        let replayable = store.conversation_items(&conversation_id).await.unwrap();
        assert_eq!(replayable.len(), 1);
        assert_eq!(replayable[0].run_id, "run-text");
    }

    async fn pause_run(store: &SqliteRunStore, run_id: &str) {
        store.request_pause(run_id).await.unwrap();
        store
            .transition(
                run_id,
                RunTransition {
                    status: RunStatus::Paused,
                    phase: RunPhase::None,
                    event_kind: "run_paused".to_owned(),
                    payload: serde_json::json!({}),
                    stop_reason: Some("paused".to_owned()),
                },
            )
            .await
            .unwrap();
    }

    async fn setup(name: &str) -> (SqliteRunStore, String) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.keep().join(format!("{name}.sqlite3"));
        let database = Database::connect(&path).await.unwrap();
        let conversation = SqliteConversationStore::new(database.clone())
            .create(NewConversation {
                title: name.to_owned(),
                default_provider_profile_id: "local".to_owned(),
                default_model: "fake".to_owned(),
            })
            .await
            .unwrap();
        (SqliteRunStore::new(database), conversation.id)
    }

    fn new_run(id: &str, conversation_id: &str) -> NewRun {
        NewRun {
            id: id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            strategy: RunStrategy::Auto,
            provider_profile: ProviderProfile {
                id: "local".to_owned(),
                label: "Local".to_owned(),
                kind: ProviderKind::OpenaiCompatible,
                protocol: ProviderProtocol::ChatCompletions,
                base_url: "http://127.0.0.1:11434/v1".to_owned(),
                default_model: "fake".to_owned(),
                available_models: vec!["fake".to_owned()],
                enabled_models: vec!["fake".to_owned()],
                models_synced_at_ms: None,
                credential_ref: "local-api-key".to_owned(),
                store_responses: true,
                capabilities: ProviderCapabilities {
                    tools: true,
                    images: true,
                    files: false,
                },
            },
            model: "fake".to_owned(),
            runtime_instance_id: "runtime-1".to_owned(),
            replaces_run_id: None,
            parent_run_id: None,
            source_pending_input_id: None,
            user_content: serde_json::json!({
                "role": "user",
                "content": [{"type": "text", "text": "Hello"}],
            }),
            tool_catalog_snapshot: Vec::new(),
        }
    }
}
