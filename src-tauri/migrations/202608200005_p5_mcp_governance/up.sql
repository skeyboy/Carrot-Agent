ALTER TABLE runs ADD COLUMN tool_catalog_snapshot_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE tool_executions ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'built_in';
ALTER TABLE tool_executions ADD COLUMN source_server_id TEXT;
ALTER TABLE tool_executions ADD COLUMN remote_tool_name TEXT;
ALTER TABLE tool_executions ADD COLUMN tool_schema_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE tool_executions ADD COLUMN tool_definition_snapshot_json TEXT NOT NULL
    DEFAULT '{"name":"","description":"","parameters":{},"strict":false}';
ALTER TABLE tool_executions ADD COLUMN tool_policy_snapshot_json TEXT NOT NULL
    DEFAULT '{"risk":"read_only","idempotent":false,"cancellable":true,"reconcile":false}';
