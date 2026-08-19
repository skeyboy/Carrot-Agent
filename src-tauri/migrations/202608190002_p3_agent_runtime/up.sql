PRAGMA foreign_keys = ON;

CREATE TABLE tool_executions (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    call_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'prepared', 'executing', 'succeeded', 'failed', 'cancelled'
    )),
    risk TEXT NOT NULL CHECK (risk IN (
        'read_only', 'local_write', 'external_side_effect', 'dangerous'
    )),
    arguments_json TEXT NOT NULL,
    arguments_hash TEXT NOT NULL,
    output_json TEXT,
    error_message TEXT,
    retryable BOOLEAN NOT NULL DEFAULT FALSE,
    prepared_at_ms BIGINT NOT NULL,
    started_at_ms BIGINT,
    completed_at_ms BIGINT,
    UNIQUE (run_id, call_id)
);

CREATE INDEX tool_executions_run_status_idx
    ON tool_executions(run_id, status, prepared_at_ms);

CREATE TABLE plans (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    revision BIGINT NOT NULL CHECK (revision > 0),
    goal TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'superseded', 'failed')),
    created_at_ms BIGINT NOT NULL,
    UNIQUE (run_id, revision)
);

CREATE TABLE plan_steps (
    id TEXT PRIMARY KEY NOT NULL,
    plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    ordinal BIGINT NOT NULL CHECK (ordinal > 0),
    title TEXT NOT NULL,
    acceptance TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'skipped')),
    attempt BIGINT NOT NULL DEFAULT 0 CHECK (attempt >= 0),
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    UNIQUE (plan_id, ordinal)
);

CREATE TABLE run_snapshots (
    run_id TEXT PRIMARY KEY NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    event_high_water_seq BIGINT NOT NULL CHECK (event_high_water_seq >= 0),
    state_json TEXT NOT NULL,
    updated_at_ms BIGINT NOT NULL
);
