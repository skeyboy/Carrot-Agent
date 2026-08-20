use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use specta::Type;
use tokio_util::sync::CancellationToken;

use crate::providers::runtime::ToolDefinition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
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

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "read_only" => Self::ReadOnly,
            "local_write" => Self::LocalWrite,
            "external_side_effect" => Self::ExternalSideEffect,
            "dangerous" => Self::Dangerous,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    pub risk: ToolRisk,
    pub idempotent: bool,
    pub cancellable: bool,
    pub reconcile: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolIdentity {
    pub source_kind: String,
    pub source_server_id: Option<String>,
    pub remote_tool_name: Option<String>,
    pub schema_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolSnapshot {
    pub definition: ToolDefinition,
    pub capabilities: ToolCapabilities,
    pub identity: ToolIdentity,
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
    #[error("tool outcome is unknown: {0}")]
    OutcomeUnknown(String),
}

#[async_trait]
pub trait AgentTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn capabilities(&self) -> ToolCapabilities;
    fn identity(&self) -> ToolIdentity {
        let definition = self.definition();
        ToolIdentity {
            source_kind: "built_in".to_owned(),
            source_server_id: None,
            remote_tool_name: None,
            schema_hash: schema_hash(&definition.parameters),
        }
    }
    fn business_idempotency_key(&self, _arguments: &Value) -> Option<String> {
        None
    }
    async fn approval_preview(&self, _arguments: &Value) -> Result<Option<String>, ToolError> {
        Ok(None)
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

    pub fn extend(&self, additions: Vec<Arc<dyn AgentTool>>) -> Self {
        let mut tools = self.tools.as_ref().clone();
        for tool in additions {
            tools.insert(tool.definition().name, tool);
        }
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

    pub async fn approval_preview(
        &self,
        name: &str,
        arguments: &Value,
    ) -> Result<Option<String>, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::Unknown(name.to_owned()))?;
        tool.approval_preview(arguments).await
    }

    pub fn snapshot(&self) -> Vec<ToolSnapshot> {
        let mut snapshot = self
            .tools
            .values()
            .map(|tool| ToolSnapshot {
                definition: tool.definition(),
                capabilities: tool.capabilities(),
                identity: tool.identity(),
            })
            .collect::<Vec<_>>();
        snapshot.sort_by(|left, right| left.definition.name.cmp(&right.definition.name));
        snapshot
    }

    pub fn identity(&self, name: &str) -> Result<ToolIdentity, ToolError> {
        self.tools
            .get(name)
            .map(|tool| tool.identity())
            .ok_or_else(|| ToolError::Unknown(name.to_owned()))
    }

    pub fn retain_snapshot(&self, snapshot: &[ToolSnapshot]) -> Self {
        let expected = snapshot
            .iter()
            .map(|item| (item.definition.name.as_str(), item))
            .collect::<HashMap<_, _>>();
        let tools = self
            .tools
            .iter()
            .filter_map(|(name, tool)| {
                let item = expected.get(name.as_str())?;
                (tool.identity() == item.identity && tool.definition() == item.definition)
                    .then(|| (name.clone(), tool.clone()))
            })
            .collect();
        Self {
            tools: Arc::new(tools),
        }
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
        validate_arguments(&tool.definition().parameters, &arguments)?;
        tool.execute(arguments, context, cancellation).await
    }
}

pub fn schema_hash(schema: &Value) -> String {
    use sha2::{Digest, Sha256};
    let encoded = serde_json::to_vec(schema).unwrap_or_default();
    format!("{:x}", Sha256::digest(encoded))
}

fn validate_arguments(schema: &Value, arguments: &Value) -> Result<(), ToolError> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| ToolError::InvalidArguments(format!("tool schema is invalid: {error}")))?;
    if let Err(error) = validator.validate(arguments) {
        return Err(ToolError::InvalidArguments(format!(
            "{} at {}",
            error,
            error.instance_path()
        )));
    }
    Ok(())
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
