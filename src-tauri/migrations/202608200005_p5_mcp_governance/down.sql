ALTER TABLE tool_executions DROP COLUMN tool_policy_snapshot_json;
ALTER TABLE tool_executions DROP COLUMN tool_definition_snapshot_json;
ALTER TABLE tool_executions DROP COLUMN tool_schema_hash;
ALTER TABLE tool_executions DROP COLUMN remote_tool_name;
ALTER TABLE tool_executions DROP COLUMN source_server_id;
ALTER TABLE tool_executions DROP COLUMN source_kind;
ALTER TABLE runs DROP COLUMN tool_catalog_snapshot_json;
