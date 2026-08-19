PRAGMA foreign_keys = ON;

ALTER TABLE pending_inputs ADD COLUMN child_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL;

ALTER TABLE tool_executions ADD COLUMN idempotency_key TEXT;
ALTER TABLE tool_executions ADD COLUMN reconciliation_status TEXT NOT NULL DEFAULT 'not_required'
    CHECK (reconciliation_status IN ('not_required', 'pending', 'resolved_succeeded', 'resolved_failed', 'abandoned'));
ALTER TABLE tool_executions ADD COLUMN reconciliation_note TEXT;

CREATE UNIQUE INDEX tool_executions_idempotency_idx
    ON tool_executions(tool_name, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE TABLE tool_approvals (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    tool_execution_id TEXT NOT NULL REFERENCES tool_executions(id) ON DELETE CASCADE,
    call_id TEXT NOT NULL,
    arguments_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied')),
    requested_at_ms BIGINT NOT NULL,
    resolved_at_ms BIGINT,
    UNIQUE (tool_execution_id),
    UNIQUE (run_id, call_id, arguments_hash)
);

CREATE INDEX tool_approvals_run_status_idx
    ON tool_approvals(run_id, status, requested_at_ms);
