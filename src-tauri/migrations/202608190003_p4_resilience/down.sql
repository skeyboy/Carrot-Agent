DROP INDEX IF EXISTS tool_approvals_run_status_idx;
DROP TABLE IF EXISTS tool_approvals;
DROP INDEX IF EXISTS tool_executions_idempotency_idx;
ALTER TABLE tool_executions DROP COLUMN reconciliation_note;
ALTER TABLE tool_executions DROP COLUMN reconciliation_status;
ALTER TABLE tool_executions DROP COLUMN idempotency_key;
ALTER TABLE pending_inputs DROP COLUMN child_run_id;
