PRAGMA foreign_keys = ON;

CREATE TABLE provider_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL CHECK (length(trim(label)) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('openai_responses', 'openai_compatible')),
    protocol TEXT NOT NULL CHECK (protocol IN ('responses', 'chat_completions')),
    base_url TEXT NOT NULL,
    default_model TEXT NOT NULL CHECK (length(trim(default_model)) > 0),
    credential_ref TEXT NOT NULL CHECK (length(trim(credential_ref)) > 0),
    store_responses BOOLEAN NOT NULL DEFAULT TRUE,
    supports_tools BOOLEAN NOT NULL DEFAULT TRUE,
    supports_images BOOLEAN NOT NULL DEFAULT TRUE,
    supports_files BOOLEAN NOT NULL DEFAULT FALSE,
    config_json TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL
);

CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    default_provider_profile_id TEXT NOT NULL,
    default_model TEXT NOT NULL CHECK (length(trim(default_model)) > 0),
    archived BOOLEAN NOT NULL DEFAULT FALSE,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL
);

CREATE INDEX conversations_updated_at_idx
    ON conversations(archived, updated_at_ms DESC);

CREATE TABLE runs (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    parent_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK (status IN (
        'queued', 'running', 'pause_requested', 'paused', 'suspended',
        'waiting_for_approval', 'completed', 'failed', 'cancelled',
        'interrupted', 'recovery_required'
    )),
    phase TEXT NOT NULL CHECK (phase IN (
        'routing', 'planning', 'model_stream', 'tool_prepare', 'tool_execute',
        'observation_commit', 'reflecting', 'finalizing', 'none'
    )),
    strategy TEXT NOT NULL CHECK (strategy IN ('fast', 'auto', 'quality')),
    provider_profile_id TEXT NOT NULL,
    provider_snapshot_json TEXT NOT NULL,
    model TEXT NOT NULL CHECK (length(trim(model)) > 0),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    last_event_seq BIGINT NOT NULL DEFAULT 0 CHECK (last_event_seq >= 0),
    runtime_instance_id TEXT,
    lease_expires_at_ms BIGINT,
    stop_reason TEXT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    completed_at_ms BIGINT
);

CREATE INDEX runs_conversation_created_idx
    ON runs(conversation_id, created_at_ms DESC);

CREATE UNIQUE INDEX runs_one_active_per_conversation_idx
    ON runs(conversation_id)
    WHERE status IN ('queued', 'running', 'pause_requested', 'waiting_for_approval');

CREATE TABLE items (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    seq BIGINT NOT NULL CHECK (seq > 0),
    kind TEXT NOT NULL CHECK (kind IN (
        'message', 'function_call', 'function_call_output', 'reasoning_summary',
        'plan', 'reflection', 'provider_output'
    )),
    role TEXT CHECK (role IS NULL OR role IN ('user', 'assistant', 'system', 'tool')),
    status TEXT NOT NULL CHECK (status IN ('committed', 'superseded', 'abandoned')),
    content_json TEXT NOT NULL,
    provider_item_id TEXT,
    call_id TEXT,
    created_at_ms BIGINT NOT NULL,
    UNIQUE (run_id, seq)
);

CREATE INDEX items_run_call_idx ON items(run_id, call_id);

CREATE TABLE run_events (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    seq BIGINT NOT NULL CHECK (seq > 0),
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    persisted_at_ms BIGINT NOT NULL,
    UNIQUE (run_id, seq)
);

CREATE TABLE pending_inputs (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    item_id TEXT REFERENCES items(id) ON DELETE SET NULL,
    intent TEXT NOT NULL CHECK (intent IN ('append', 'fork', 'cancel_and_replace')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'consumed', 'cancelled')),
    content_json TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    consumed_at_ms BIGINT
);

CREATE INDEX pending_inputs_run_status_idx
    ON pending_inputs(run_id, status, created_at_ms);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    item_id TEXT REFERENCES items(id) ON DELETE SET NULL,
    file_name TEXT NOT NULL,
    media_type TEXT NOT NULL,
    byte_length BIGINT NOT NULL CHECK (byte_length >= 0),
    content_hash TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ready', 'deleted', 'failed')),
    created_at_ms BIGINT NOT NULL
);

CREATE INDEX attachments_conversation_idx ON attachments(conversation_id, created_at_ms);
