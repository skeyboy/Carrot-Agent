use async_trait::async_trait;
use diesel::dsl::max;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use uuid::Uuid;

use crate::domain::run::{
    AgentRun, ChatSnapshot, CommitResult, NewRun, NewRunItem, NewToolExecution, PlanDraft,
    RunEvent, RunItem, RunTransition, ToolExecution, ToolExecutionResult,
};
use crate::domain::storage::{RunStore, StoreError};

use super::database::Database;
use super::models::{
    NewPlanRow, NewPlanStepRow, NewRunEventRow, NewRunItemRow, NewRunRow, NewRunSnapshotRow,
    NewToolExecutionRow, RunChangeset, RunEventRow, RunItemRow, RunRow, ToolExecutionRow,
};
use super::now_ms;
use super::schema::{items, plan_steps, plans, run_events, run_snapshots, runs, tool_executions};

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
        let run_id = input.id.clone();
        let run_row = NewRunRow {
            id: &input.id,
            conversation_id: &input.conversation_id,
            parent_run_id: input.replaces_run_id.as_deref(),
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
                    let replaceable = runs::table
                        .filter(runs::id.eq(replaced_run_id))
                        .filter(runs::conversation_id.eq(&input.conversation_id))
                        .filter(runs::status.eq_any(["paused", "cancelled"]))
                        .select(runs::id)
                        .first::<String>(connection)
                        .await
                        .optional()?;
                    if replaceable.is_none() {
                        return Err(diesel::result::Error::RollbackTransaction);
                    }
                    diesel::update(items::table.filter(items::run_id.eq(replaced_run_id)))
                        .set(items::status.eq("superseded"))
                        .execute(connection)
                        .await?;
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
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    "running",
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
        let mut latest_run = None;
        for row in run_rows {
            let run_id = row.id.clone();
            let run = AgentRun::try_from(row)?;
            latest_run = Some(run);
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
            active_run: latest_run.filter(|run| !run.status.is_terminal()),
            items,
            events: events_output,
            tool_executions: tools_output,
        })
    }

    async fn get_run(&self, run_id: &str) -> Result<Option<AgentRun>, StoreError> {
        self.load_run_row(run_id)
            .await?
            .map(AgentRun::try_from)
            .transpose()
    }
}

impl SqliteRunStore {
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
                        output_json: None,
                        error_message: None,
                        retryable: execution.retryable,
                        prepared_at_ms: now,
                        started_at_ms: None,
                        completed_at_ms: None,
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
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    "running",
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
                update_run_state(
                    connection,
                    &row,
                    event_seq,
                    "running",
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
                update_run_state(
                    connection, &row, event_seq, "running", "planning", None, now,
                )
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
