use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::providers::runtime::ToolDefinition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ToolRisk {
    ReadOnly,
    LocalWrite,
    ExternalSideEffect,
    Dangerous,
}

impl ToolRisk {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::LocalWrite => "local_write",
            Self::ExternalSideEffect => "external_side_effect",
            Self::Dangerous => "dangerous",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolCapabilities {
    pub risk: ToolRisk,
    pub idempotent: bool,
    pub cancellable: bool,
    pub reconcile: bool,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub idempotency_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unknown tool '{0}'")]
    Unknown(String),
    #[error("invalid tool arguments: {0}")]
    InvalidArguments(String),
    #[error("tool execution failed: {0}")]
    Execution(String),
    #[error("tool execution was cancelled")]
    Cancelled,
}

#[async_trait]
pub trait AgentTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn capabilities(&self) -> ToolCapabilities;
    fn business_idempotency_key(&self, _arguments: &Value) -> Option<String> {
        None
    }
    async fn execute(
        &self,
        arguments: Value,
        context: ToolExecutionContext,
        cancellation: CancellationToken,
    ) -> Result<Value, ToolError>;
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: Arc<HashMap<String, Arc<dyn AgentTool>>>,
}

impl ToolRegistry {
    pub fn built_in() -> Self {
        Self::new(vec![Arc::new(CurrentTimeTool), Arc::new(CalculatorTool)])
    }

    pub fn new(tools: Vec<Arc<dyn AgentTool>>) -> Self {
        let tools = tools
            .into_iter()
            .map(|tool| (tool.definition().name, tool))
            .collect();
        Self {
            tools: Arc::new(tools),
        }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut definitions = self
            .tools
            .values()
            .map(|tool| tool.definition())
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.name.cmp(&right.name));
        definitions
    }

    pub fn capabilities(&self, name: &str) -> Result<ToolCapabilities, ToolError> {
        self.tools
            .get(name)
            .map(|tool| tool.capabilities())
            .ok_or_else(|| ToolError::Unknown(name.to_owned()))
    }

    pub fn idempotency_key(
        &self,
        name: &str,
        arguments: &Value,
        fallback: String,
    ) -> Result<String, ToolError> {
        self.tools
            .get(name)
            .map(|tool| tool.business_idempotency_key(arguments).unwrap_or(fallback))
            .ok_or_else(|| ToolError::Unknown(name.to_owned()))
    }

    pub async fn execute(
        &self,
        name: &str,
        arguments: Value,
        context: ToolExecutionContext,
        cancellation: CancellationToken,
    ) -> Result<Value, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::Unknown(name.to_owned()))?;
        tool.execute(arguments, context, cancellation).await
    }
}

struct CurrentTimeTool;

#[async_trait]
impl AgentTool for CurrentTimeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_current_time".to_owned(),
            description: "Get the current Unix timestamp in milliseconds.".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            strict: true,
        }
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            risk: ToolRisk::ReadOnly,
            idempotent: true,
            cancellable: true,
            reconcile: true,
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        context: ToolExecutionContext,
        cancellation: CancellationToken,
    ) -> Result<Value, ToolError> {
        debug_assert!(!context.idempotency_key.is_empty());
        if cancellation.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        if arguments.as_object().is_none_or(|value| !value.is_empty()) {
            return Err(ToolError::InvalidArguments(
                "get_current_time accepts no arguments".to_owned(),
            ));
        }
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .as_millis();
        Ok(json!({"unixTimestampMs": timestamp_ms.to_string()}))
    }
}

struct CalculatorTool;

#[async_trait]
impl AgentTool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calculator".to_owned(),
            description: "Perform one basic arithmetic operation on two finite numbers.".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "operation": {"type": "string", "enum": ["add", "subtract", "multiply", "divide"]},
                    "left": {"type": "number"},
                    "right": {"type": "number"}
                },
                "required": ["operation", "left", "right"],
                "additionalProperties": false
            }),
            strict: true,
        }
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            risk: ToolRisk::ReadOnly,
            idempotent: true,
            cancellable: true,
            reconcile: true,
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        context: ToolExecutionContext,
        cancellation: CancellationToken,
    ) -> Result<Value, ToolError> {
        debug_assert!(!context.idempotency_key.is_empty());
        if cancellation.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let object = arguments
            .as_object()
            .ok_or_else(|| ToolError::InvalidArguments("expected an object".to_owned()))?;
        if object.len() != 3 {
            return Err(ToolError::InvalidArguments(
                "expected only operation, left, and right".to_owned(),
            ));
        }
        let operation = object
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("operation is required".to_owned()))?;
        let left = finite_number(object.get("left"), "left")?;
        let right = finite_number(object.get("right"), "right")?;
        let result = match operation {
            "add" => left + right,
            "subtract" => left - right,
            "multiply" => left * right,
            "divide" if right == 0.0 => {
                return Err(ToolError::InvalidArguments(
                    "right cannot be zero for division".to_owned(),
                ));
            }
            "divide" => left / right,
            _ => {
                return Err(ToolError::InvalidArguments(
                    "operation is not supported".to_owned(),
                ));
            }
        };
        Ok(json!({"result": result}))
    }
}

fn finite_number(value: Option<&Value>, name: &str) -> Result<f64, ToolError> {
    value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| ToolError::InvalidArguments(format!("{name} must be a finite number")))
}

#[cfg(test)]
mod tests {
    use super::{ToolExecutionContext, ToolRegistry};
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn built_in_calculator_validates_and_executes() {
        let registry = ToolRegistry::built_in();
        let result = registry
            .execute(
                "calculator",
                serde_json::json!({"operation": "multiply", "left": 6, "right": 7}),
                ToolExecutionContext {
                    idempotency_key: "test-calculation".to_owned(),
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result["result"], 42.0);
        assert!(registry.definitions().iter().all(|tool| tool.strict));
        assert_eq!(
            registry
                .idempotency_key(
                    "calculator",
                    &serde_json::json!({}),
                    "execution-1".to_owned()
                )
                .unwrap(),
            "execution-1"
        );
    }
}
