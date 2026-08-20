use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::domain::mcp::McpToolDescriptor;
use crate::domain::mcp::McpToolPolicy;
use crate::providers::runtime::ToolDefinition;
use crate::tools::{
    AgentTool, ToolCapabilities, ToolError, ToolExecutionContext, ToolIdentity, ToolRisk,
};

use super::manager::McpClientManager;

pub struct McpToolAdapter {
    descriptor: McpToolDescriptor,
    manager: Arc<McpClientManager>,
    policy: McpToolPolicy,
}

impl McpToolAdapter {
    pub fn new(
        descriptor: McpToolDescriptor,
        manager: Arc<McpClientManager>,
        policy: McpToolPolicy,
    ) -> Self {
        Self {
            descriptor,
            manager,
            policy,
        }
    }
}

#[async_trait]
impl AgentTool for McpToolAdapter {
    fn definition(&self) -> ToolDefinition {
        let policy = match self.policy.risk {
            ToolRisk::ReadOnly => "Read-only",
            ToolRisk::LocalWrite => "Approval-required local write",
            ToolRisk::ExternalSideEffect => "Approval-required external side effect",
            ToolRisk::Dangerous => "Approval-required dangerous operation",
        };
        ToolDefinition {
            name: self.descriptor.alias.clone(),
            description: format!(
                "{policy} MCP tool from {}. {}",
                self.descriptor.server_id, self.descriptor.description
            ),
            parameters: self.descriptor.input_schema.clone(),
            strict: true,
        }
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            risk: self.policy.risk,
            idempotent: self.policy.idempotent,
            cancellable: true,
            reconcile: self.policy.reconcile,
        }
    }

    fn identity(&self) -> ToolIdentity {
        ToolIdentity {
            source_kind: "mcp".to_owned(),
            source_server_id: Some(self.descriptor.server_id.clone()),
            remote_tool_name: Some(self.descriptor.remote_name.clone()),
            schema_hash: self.descriptor.schema_hash.clone(),
        }
    }

    async fn approval_preview(&self, arguments: &Value) -> Result<Option<String>, ToolError> {
        self.manager
            .preview_file_change(
                &self.descriptor.server_id,
                &self.descriptor.remote_name,
                arguments,
            )
            .await
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))
    }

    async fn execute(
        &self,
        arguments: Value,
        _context: ToolExecutionContext,
        cancellation: CancellationToken,
    ) -> Result<Value, ToolError> {
        let result = self
            .manager
            .call_tool(
                &self.descriptor.server_id,
                &self.descriptor.remote_name,
                arguments,
                cancellation,
            )
            .await;
        let value = match result {
            Ok(value) => value,
            Err(error)
                if self.policy.risk != ToolRisk::ReadOnly
                    && matches!(
                        error,
                        super::manager::McpError::Cancelled
                            | super::manager::McpError::Timeout
                            | super::manager::McpError::Protocol(_)
                    ) =>
            {
                return Err(ToolError::OutcomeUnknown(error.to_string()));
            }
            Err(error) => return Err(ToolError::Execution(error.to_string())),
        };
        if let Some(schema) = &self.descriptor.output_schema {
            let structured = value.get("structuredContent").ok_or_else(|| {
                ToolError::Execution("MCP result omitted required structured content".to_owned())
            })?;
            let validator = jsonschema::validator_for(schema)
                .map_err(|error| ToolError::Execution(format!("invalid output schema: {error}")))?;
            if let Err(error) = validator.validate(structured) {
                return Err(ToolError::Execution(format!(
                    "MCP output failed schema validation: {error}"
                )));
            }
        }
        Ok(value)
    }
}
