use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::cancellation::RunCancellation;
use crate::domain::provider::ProviderProfile;
use crate::domain::run::{
    NewRun, NewRunItem, NewToolExecution, PlanDraft, RunPhase, RunStatus, RunTransition,
    ToolExecutionResult,
};
use crate::domain::settings::RunStrategy;
use crate::domain::storage::RunStore;
use crate::providers::runtime::{
    LlmProvider, MessageContent, MessageRole, ProviderEvent, ProviderInputItem, ProviderMessage,
    ProviderRequest,
};
use crate::tools::{ToolCapabilities, ToolError, ToolRegistry, ToolRisk};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Store(#[from] crate::domain::storage::StoreError),
    #[error("provider failed: {0}")]
    Provider(String),
    #[error("run exceeded its {0} model-step budget")]
    Budget(u16),
    #[error("run was cancelled")]
    Cancelled,
    #[error("run was paused")]
    Paused,
}

pub struct RuntimeInput {
    pub run_id: String,
    pub conversation_id: String,
    pub strategy: RunStrategy,
    pub provider_profile: ProviderProfile,
    pub model: String,
    pub runtime_instance_id: String,
    pub replaces_run_id: Option<String>,
    pub user_message: ProviderMessage,
    pub max_model_steps: u16,
    pub request_timeout: Duration,
}

pub struct AgentRuntime {
    store: Arc<dyn RunStore>,
    tools: ToolRegistry,
}

impl AgentRuntime {
    pub fn new(store: Arc<dyn RunStore>, tools: ToolRegistry) -> Self {
        Self { store, tools }
    }

    pub async fn run(
        &self,
        provider: Arc<dyn LlmProvider>,
        input: RuntimeInput,
        events: mpsc::Sender<ProviderEvent>,
        cancellation: RunCancellation,
    ) -> Result<(), RuntimeError> {
        let user_content = serde_json::to_value(&input.user_message)
            .map_err(|error| RuntimeError::Provider(error.to_string()))?;
        self.store
            .start(NewRun {
                id: input.run_id.clone(),
                conversation_id: input.conversation_id.clone(),
                strategy: input.strategy,
                provider_profile: input.provider_profile.clone(),
                model: input.model.clone(),
                runtime_instance_id: input.runtime_instance_id.clone(),
                replaces_run_id: input.replaces_run_id.clone(),
                user_content,
            })
            .await?;

        let result = self
            .run_started(provider, &input, events.clone(), cancellation.clone())
            .await;
        let result = match result {
            Err(RuntimeError::Cancelled) if cancellation.is_pause_requested() => {
                Err(RuntimeError::Paused)
            }
            result => result,
        };
        if let Err(error) = &result {
            let (status, event_kind) = match error {
                RuntimeError::Cancelled => (RunStatus::Cancelled, "run_cancelled"),
                RuntimeError::Paused => (RunStatus::Paused, "run_paused"),
                _ => (RunStatus::Failed, "run_failed"),
            };
            let _ = self
                .store
                .transition(
                    &input.run_id,
                    RunTransition {
                        status,
                        phase: RunPhase::None,
                        event_kind: event_kind.to_owned(),
                        payload: serde_json::json!({"message": error.to_string()}),
                        stop_reason: Some(error.to_string()),
                    },
                )
                .await;
            let provider_event = match status {
                RunStatus::Cancelled => ProviderEvent::Cancelled,
                RunStatus::Paused => ProviderEvent::Paused,
                _ => ProviderEvent::Failed {
                    message: error.to_string(),
                },
            };
            let _ = events.send(provider_event).await;
            return Ok(());
        }
        result
    }

    async fn run_started(
        &self,
        provider: Arc<dyn LlmProvider>,
        input: &RuntimeInput,
        events: mpsc::Sender<ProviderEvent>,
        cancellation: RunCancellation,
    ) -> Result<(), RuntimeError> {
        if input.strategy != RunStrategy::Fast {
            self.store
                .create_plan(
                    &input.run_id,
                    PlanDraft {
                        goal: message_text(&input.user_message),
                        steps: vec![(
                            "Resolve the user request".to_owned(),
                            "Return a grounded final response".to_owned(),
                        )],
                    },
                )
                .await?;
        }

        let mut provider_input = self.replay_input(&input.conversation_id).await?;
        let tools = if input.provider_profile.capabilities.tools {
            self.tools.definitions()
        } else {
            Vec::new()
        };

        for step in 1..=input.max_model_steps {
            if cancellation.is_cancelled() {
                return Err(RuntimeError::Cancelled);
            }
            self.store
                .transition(
                    &input.run_id,
                    RunTransition {
                        status: RunStatus::Running,
                        phase: RunPhase::ModelStream,
                        event_kind: "model_request_started".to_owned(),
                        payload: serde_json::json!({"step": step}),
                        stop_reason: None,
                    },
                )
                .await?;

            let outcome = stream_once(
                provider.clone(),
                ProviderRequest {
                    model: input.model.clone(),
                    input: provider_input.clone(),
                    tools: tools.clone(),
                    previous_response_id: None,
                    store: input.provider_profile.store_responses,
                },
                events.clone(),
                cancellation.token().child_token(),
                input.request_timeout,
            )
            .await?;

            if !outcome.text.is_empty() {
                let message = ProviderMessage {
                    role: MessageRole::Assistant,
                    content: vec![MessageContent::Text {
                        text: outcome.text.clone(),
                    }],
                };
                self.store
                    .commit_item(
                        &input.run_id,
                        NewRunItem {
                            kind: "message".to_owned(),
                            role: Some("assistant".to_owned()),
                            content: serde_json::to_value(&message)
                                .map_err(|error| RuntimeError::Provider(error.to_string()))?,
                            call_id: None,
                            status: "committed".to_owned(),
                        },
                        "assistant_message_committed",
                        serde_json::json!({"step": step}),
                    )
                    .await?;
                provider_input.push(ProviderInputItem::Message { message });
            }

            if outcome.tool_calls.is_empty() {
                if input.strategy == RunStrategy::Quality && !outcome.text.is_empty() {
                    self.store
                        .transition(
                            &input.run_id,
                            RunTransition {
                                status: RunStatus::Running,
                                phase: RunPhase::Reflecting,
                                event_kind: "reflection_completed".to_owned(),
                                payload: serde_json::json!({
                                    "bounded": true,
                                    "decision": "candidate accepted",
                                }),
                                stop_reason: None,
                            },
                        )
                        .await?;
                    self.store
                        .commit_item(
                            &input.run_id,
                            NewRunItem {
                                kind: "reflection".to_owned(),
                                role: None,
                                content: serde_json::json!({
                                    "decision": "candidate accepted",
                                    "bounded": true,
                                }),
                                call_id: None,
                                status: "committed".to_owned(),
                            },
                            "reflection_summary_committed",
                            serde_json::json!({"bounded": true}),
                        )
                        .await?;
                }
                self.store
                    .transition(
                        &input.run_id,
                        RunTransition {
                            status: RunStatus::Completed,
                            phase: RunPhase::None,
                            event_kind: "run_completed".to_owned(),
                            payload: serde_json::json!({"modelSteps": step}),
                            stop_reason: Some("completed".to_owned()),
                        },
                    )
                    .await?;
                if let Some(completed) = outcome.completed {
                    events
                        .send(completed)
                        .await
                        .map_err(|_| RuntimeError::Provider("event receiver closed".to_owned()))?;
                }
                return Ok(());
            }

            for call in outcome.tool_calls {
                let capabilities =
                    self.tools
                        .capabilities(&call.name)
                        .unwrap_or(ToolCapabilities {
                            risk: ToolRisk::ReadOnly,
                            idempotent: false,
                            cancellable: true,
                            reconcile: false,
                        });
                let serialized = serde_json::to_vec(&call.arguments)
                    .map_err(|error| RuntimeError::Provider(error.to_string()))?;
                let execution_id = Uuid::now_v7().to_string();
                self.store
                    .prepare_tool(
                        &input.run_id,
                        NewToolExecution {
                            id: execution_id.clone(),
                            call_id: call.call_id.clone(),
                            tool_name: call.name.clone(),
                            risk: capabilities.risk.as_str().to_owned(),
                            arguments: call.arguments.clone(),
                            arguments_hash: format!("{:x}", Sha256::digest(serialized)),
                            retryable: capabilities.idempotent && capabilities.reconcile,
                        },
                    )
                    .await?;
                self.store
                    .mark_tool_executing(&input.run_id, &execution_id)
                    .await?;
                let tool_token = if capabilities.cancellable {
                    cancellation.token().child_token()
                } else {
                    CancellationToken::new()
                };
                let execution = self
                    .tools
                    .execute(&call.name, call.arguments.clone(), tool_token);
                let result = if capabilities.cancellable {
                    match tokio::time::timeout(
                        input.request_timeout.min(Duration::from_secs(30)),
                        execution,
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => Err(ToolError::Execution("tool timed out".to_owned())),
                    }
                } else {
                    execution.await
                };
                let (output, error_message, cancelled) = match result {
                    Ok(output) => (output, None, false),
                    Err(error) => (
                        serde_json::json!({"ok": false, "error": error.to_string()}),
                        Some(error.to_string()),
                        matches!(error, crate::tools::ToolError::Cancelled),
                    ),
                };
                self.store
                    .finish_tool(
                        &input.run_id,
                        &execution_id,
                        &call.call_id,
                        ToolExecutionResult {
                            output: Some(output.clone()),
                            error_message,
                            cancelled,
                        },
                    )
                    .await?;
                provider_input.push(ProviderInputItem::ToolCall {
                    call_id: call.call_id.clone(),
                    name: call.name,
                    arguments: call.arguments,
                });
                provider_input.push(ProviderInputItem::ToolOutput {
                    call_id: call.call_id,
                    output,
                });
            }
        }
        Err(RuntimeError::Budget(input.max_model_steps))
    }

    async fn replay_input(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ProviderInputItem>, RuntimeError> {
        let mut output = Vec::new();
        for item in self.store.conversation_items(conversation_id).await? {
            match item.kind.as_str() {
                "message" => {
                    let message = serde_json::from_value(item.content)
                        .map_err(|error| RuntimeError::Provider(error.to_string()))?;
                    output.push(ProviderInputItem::Message { message });
                }
                "function_call" => output.push(ProviderInputItem::ToolCall {
                    call_id: item.call_id.unwrap_or_default(),
                    name: item.content["name"].as_str().unwrap_or_default().to_owned(),
                    arguments: item.content["arguments"].clone(),
                }),
                "function_call_output" => output.push(ProviderInputItem::ToolOutput {
                    call_id: item.call_id.unwrap_or_default(),
                    output: item.content,
                }),
                _ => {}
            }
        }
        Ok(output)
    }
}

struct ModelOutcome {
    text: String,
    tool_calls: Vec<ToolCall>,
    completed: Option<ProviderEvent>,
}

struct ToolCall {
    call_id: String,
    name: String,
    arguments: serde_json::Value,
}

async fn stream_once(
    provider: Arc<dyn LlmProvider>,
    request: ProviderRequest,
    outward: mpsc::Sender<ProviderEvent>,
    cancellation: CancellationToken,
    timeout: Duration,
) -> Result<ModelOutcome, RuntimeError> {
    let (sender, mut receiver) = mpsc::channel(64);
    let provider_token = cancellation.clone();
    let task = tokio::spawn(async move { provider.stream(request, sender, provider_token).await });
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut completed = None;
    let receive = async {
        while let Some(event) = receiver.recv().await {
            match &event {
                ProviderEvent::TextDelta { delta } => text.push_str(delta),
                ProviderEvent::ToolCall {
                    call_id,
                    name,
                    arguments,
                } => tool_calls.push(ToolCall {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                }),
                ProviderEvent::Failed { message } => {
                    return Err(RuntimeError::Provider(message.clone()));
                }
                ProviderEvent::Cancelled => return Err(RuntimeError::Cancelled),
                ProviderEvent::Completed { .. } => completed = Some(event.clone()),
                _ => {}
            }
            if !matches!(event, ProviderEvent::Completed { .. }) {
                outward
                    .send(event)
                    .await
                    .map_err(|_| RuntimeError::Provider("event receiver closed".to_owned()))?;
            }
        }
        task.await
            .map_err(|error| RuntimeError::Provider(error.to_string()))?
            .map_err(|error| RuntimeError::Provider(error.to_string()))?;
        Ok(ModelOutcome {
            text,
            tool_calls,
            completed,
        })
    };
    tokio::select! {
        _ = cancellation.cancelled() => Err(RuntimeError::Cancelled),
        result = tokio::time::timeout(timeout, receive) => match result {
            Ok(result) => result,
            Err(_) => {
                cancellation.cancel();
                Err(RuntimeError::Provider("request timed out".to_owned()))
            }
        }
    }
}

fn message_text(message: &ProviderMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|part| match part {
            MessageContent::Text { text } => Some(text.as_str()),
            MessageContent::ImageDataUrl { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::{AgentRuntime, RuntimeInput};
    use crate::domain::conversation::NewConversation;
    use crate::domain::provider::{
        ProviderCapabilities, ProviderKind, ProviderProfile, ProviderProtocol,
    };
    use crate::domain::run::RunStatus;
    use crate::domain::settings::RunStrategy;
    use crate::domain::storage::{ConversationStore, RunStore};
    use crate::persistence::{Database, SqliteConversationStore, SqliteRunStore};
    use crate::providers::runtime::{
        LlmProvider, MessageContent, MessageRole, ProviderError, ProviderEvent, ProviderMessage,
        ProviderRequest,
    };
    use crate::tools::ToolRegistry;

    struct ToolCallingProvider {
        calls: AtomicUsize,
    }

    struct BlockingProvider;

    #[async_trait]
    impl LlmProvider for BlockingProvider {
        async fn stream(
            &self,
            _request: ProviderRequest,
            events: mpsc::Sender<ProviderEvent>,
            cancellation: CancellationToken,
        ) -> Result<(), ProviderError> {
            events
                .send(ProviderEvent::Started {
                    response_id: "blocking".to_owned(),
                })
                .await
                .unwrap();
            cancellation.cancelled().await;
            let _ = events.send(ProviderEvent::Cancelled).await;
            Ok(())
        }
    }

    struct ImmediateProvider;

    #[async_trait]
    impl LlmProvider for ImmediateProvider {
        async fn stream(
            &self,
            _request: ProviderRequest,
            events: mpsc::Sender<ProviderEvent>,
            _cancellation: CancellationToken,
        ) -> Result<(), ProviderError> {
            events
                .send(ProviderEvent::Started {
                    response_id: "replacement".to_owned(),
                })
                .await
                .unwrap();
            events
                .send(ProviderEvent::TextDelta {
                    delta: "Edited answer".to_owned(),
                })
                .await
                .unwrap();
            events
                .send(ProviderEvent::Completed {
                    response_id: "replacement".to_owned(),
                    input_tokens: None,
                    output_tokens: None,
                })
                .await
                .unwrap();
            Ok(())
        }
    }

    #[async_trait]
    impl LlmProvider for ToolCallingProvider {
        async fn stream(
            &self,
            request: ProviderRequest,
            events: mpsc::Sender<ProviderEvent>,
            _cancellation: CancellationToken,
        ) -> Result<(), ProviderError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            events
                .send(ProviderEvent::Started {
                    response_id: format!("response-{call}"),
                })
                .await
                .unwrap();
            if call == 0 {
                assert!(!request.tools.is_empty());
                events
                    .send(ProviderEvent::ToolCall {
                        call_id: "call-1".to_owned(),
                        name: "calculator".to_owned(),
                        arguments: serde_json::json!({
                            "operation": "multiply",
                            "left": 6,
                            "right": 7
                        }),
                    })
                    .await
                    .unwrap();
            } else {
                assert!(request.input.iter().any(|item| matches!(
                    item,
                    crate::providers::runtime::ProviderInputItem::ToolOutput { call_id, .. }
                        if call_id == "call-1"
                )));
                events
                    .send(ProviderEvent::TextDelta {
                        delta: "The result is 42.".to_owned(),
                    })
                    .await
                    .unwrap();
            }
            events
                .send(ProviderEvent::Completed {
                    response_id: format!("response-{call}"),
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                })
                .await
                .unwrap();
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn persists_react_tool_loop_with_contiguous_events() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::connect(&temp.path().join("runtime.sqlite3"))
            .await
            .unwrap();
        let conversations = SqliteConversationStore::new(database.clone());
        let conversation = conversations
            .create(NewConversation {
                title: "Runtime test".to_owned(),
                default_provider_profile_id: "local".to_owned(),
                default_model: "fake".to_owned(),
            })
            .await
            .unwrap();
        let store: Arc<dyn RunStore> = Arc::new(SqliteRunStore::new(database));
        let runtime = AgentRuntime::new(store.clone(), ToolRegistry::built_in());
        let provider = Arc::new(ToolCallingProvider {
            calls: AtomicUsize::new(0),
        });
        let (sender, mut receiver) = mpsc::channel(32);

        runtime
            .run(
                provider,
                RuntimeInput {
                    run_id: "run-1".to_owned(),
                    conversation_id: conversation.id.clone(),
                    strategy: RunStrategy::Auto,
                    provider_profile: local_profile(),
                    model: "fake".to_owned(),
                    runtime_instance_id: "test-runtime".to_owned(),
                    replaces_run_id: None,
                    user_message: ProviderMessage {
                        role: MessageRole::User,
                        content: vec![MessageContent::Text {
                            text: "What is 6 times 7?".to_owned(),
                        }],
                    },
                    max_model_steps: 4,
                    request_timeout: Duration::from_secs(2),
                },
                sender,
                crate::agent::cancellation::RunCancellation::detached(),
            )
            .await
            .unwrap();
        let mut streamed = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            streamed.push(event);
        }
        assert_eq!(
            streamed
                .iter()
                .filter(|event| matches!(event, ProviderEvent::Completed { .. }))
                .count(),
            1
        );

        let snapshot = store.snapshot(&conversation.id).await.unwrap();
        assert_eq!(snapshot.active_run, None);
        assert_eq!(snapshot.items.len(), 4);
        assert_eq!(snapshot.tool_executions.len(), 1);
        assert_eq!(snapshot.tool_executions[0].status, "succeeded");
        assert_eq!(
            snapshot.tool_executions[0].output.as_ref().unwrap()["result"],
            42.0
        );
        assert_eq!(
            snapshot
                .events
                .iter()
                .map(|event| event.seq)
                .collect::<Vec<_>>(),
            (1..=snapshot.events.len() as i64).collect::<Vec<_>>()
        );
        assert_eq!(
            store.get_run("run-1").await.unwrap().unwrap().status,
            RunStatus::Completed
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pauses_and_atomically_replaces_the_unfinished_input() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::connect(&temp.path().join("pause.sqlite3"))
            .await
            .unwrap();
        let conversations = SqliteConversationStore::new(database.clone());
        let conversation = conversations
            .create(NewConversation {
                title: "Pause test".to_owned(),
                default_provider_profile_id: "local".to_owned(),
                default_model: "fake".to_owned(),
            })
            .await
            .unwrap();
        let store: Arc<dyn RunStore> = Arc::new(SqliteRunStore::new(database));
        let cancellation_tree = Arc::new(crate::agent::cancellation::CancellationTree::default());
        let control = cancellation_tree.begin_run("paused-run".to_owned()).await;
        let runtime = AgentRuntime::new(store.clone(), ToolRegistry::built_in());
        let (sender, mut receiver) = mpsc::channel(16);
        let conversation_id = conversation.id.clone();
        let paused_task = tokio::spawn(async move {
            runtime
                .run(
                    Arc::new(BlockingProvider),
                    runtime_input("paused-run", &conversation_id, "Original input", None),
                    sender,
                    control,
                )
                .await
        });
        assert!(matches!(
            receiver.recv().await,
            Some(ProviderEvent::Started { .. })
        ));
        assert!(cancellation_tree.pause_run("paused-run").await);
        paused_task.await.unwrap().unwrap();
        let mut observed_pause = false;
        while let Ok(event) = receiver.try_recv() {
            observed_pause |= matches!(event, ProviderEvent::Paused);
        }
        assert!(observed_pause);
        assert_eq!(
            store.get_run("paused-run").await.unwrap().unwrap().status,
            RunStatus::Paused
        );

        let replacement_runtime = AgentRuntime::new(store.clone(), ToolRegistry::built_in());
        let (replacement_sender, mut replacement_receiver) = mpsc::channel(16);
        replacement_runtime
            .run(
                Arc::new(ImmediateProvider),
                runtime_input(
                    "replacement-run",
                    &conversation.id,
                    "Edited input",
                    Some("paused-run"),
                ),
                replacement_sender,
                crate::agent::cancellation::RunCancellation::detached(),
            )
            .await
            .unwrap();
        while replacement_receiver.try_recv().is_ok() {}

        let items = store.conversation_items(&conversation.id).await.unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.run_id == "replacement-run"));
        assert_eq!(message_text_from_item(&items[0].content), "Edited input");
        assert_eq!(message_text_from_item(&items[1].content), "Edited answer");
    }

    fn runtime_input(
        run_id: &str,
        conversation_id: &str,
        text: &str,
        replaces_run_id: Option<&str>,
    ) -> RuntimeInput {
        RuntimeInput {
            run_id: run_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            strategy: RunStrategy::Fast,
            provider_profile: local_profile(),
            model: "fake".to_owned(),
            runtime_instance_id: "test-runtime".to_owned(),
            replaces_run_id: replaces_run_id.map(str::to_owned),
            user_message: ProviderMessage {
                role: MessageRole::User,
                content: vec![MessageContent::Text {
                    text: text.to_owned(),
                }],
            },
            max_model_steps: 2,
            request_timeout: Duration::from_secs(2),
        }
    }

    fn message_text_from_item(content: &serde_json::Value) -> &str {
        content["content"][0]["text"].as_str().unwrap()
    }

    fn local_profile() -> ProviderProfile {
        ProviderProfile {
            id: "local".to_owned(),
            label: "Local".to_owned(),
            kind: ProviderKind::OpenaiCompatible,
            protocol: ProviderProtocol::ChatCompletions,
            base_url: "http://127.0.0.1:11434/v1".to_owned(),
            default_model: "fake".to_owned(),
            available_models: vec!["fake".to_owned()],
            enabled_models: vec!["fake".to_owned()],
            models_synced_at_ms: None,
            credential_ref: "local-key".to_owned(),
            store_responses: true,
            capabilities: ProviderCapabilities {
                tools: true,
                images: false,
                files: false,
            },
        }
    }
}
