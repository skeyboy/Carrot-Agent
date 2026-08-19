use std::collections::HashMap;

use async_trait::async_trait;
use futures_util::StreamExt;
use openai_oxide::types::chat::{
    ChatCompletionChunk, ChatCompletionMessageParam, ChatCompletionRequest, ContentPart,
    FinishReason, FunctionCall, FunctionDef, ImageDetail as OpenAiImageDetail, ImageUrl, Tool,
    ToolCall, ToolChoice, UserContent,
};
use openai_oxide::{ClientConfig, OpenAI};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::runtime::{
    ImageDetail, LlmProvider, MessageContent, MessageRole, ProviderError, ProviderEvent,
    ProviderInputItem, ProviderRequest,
};

pub struct OpenAiChatProvider {
    client: OpenAI,
}

impl OpenAiChatProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: OpenAI::with_config(ClientConfig::new(api_key).base_url(base_url)),
        }
    }

    pub(crate) fn build_request(
        request: ProviderRequest,
    ) -> Result<ChatCompletionRequest, ProviderError> {
        if request.model.trim().is_empty() {
            return Err(ProviderError::InvalidRequest(
                "model cannot be blank".to_owned(),
            ));
        }
        let mut messages = Vec::new();
        let mut pending_calls = Vec::new();
        for item in request.input {
            match item {
                ProviderInputItem::ToolCall {
                    call_id,
                    name,
                    arguments,
                } => pending_calls.push(ToolCall {
                    id: call_id,
                    type_: "function".to_owned(),
                    function: FunctionCall {
                        name,
                        arguments: serde_json::to_string(&arguments)
                            .map_err(|error| ProviderError::InvalidRequest(error.to_string()))?,
                    },
                }),
                item => {
                    flush_tool_calls(&mut messages, &mut pending_calls);
                    messages.push(map_input(item)?);
                }
            }
        }
        flush_tool_calls(&mut messages, &mut pending_calls);

        let mut output = ChatCompletionRequest::new(request.model, messages);
        output.store = Some(request.store);
        output.parallel_tool_calls = Some(true);
        if !request.tools.is_empty() {
            output.tools = Some(
                request
                    .tools
                    .into_iter()
                    .map(|tool| Tool {
                        type_: "function".to_owned(),
                        function: FunctionDef {
                            name: tool.name,
                            description: Some(tool.description),
                            parameters: Some(tool.parameters),
                            strict: Some(tool.strict),
                        },
                    })
                    .collect(),
            );
            output.tool_choice = Some(ToolChoice::Mode("auto".to_owned()));
        }
        Ok(output)
    }
}

fn flush_tool_calls(
    messages: &mut Vec<ChatCompletionMessageParam>,
    pending_calls: &mut Vec<ToolCall>,
) {
    if !pending_calls.is_empty() {
        messages.push(ChatCompletionMessageParam::Assistant {
            content: None,
            name: None,
            tool_calls: Some(std::mem::take(pending_calls)),
            refusal: None,
        });
    }
}

fn map_input(item: ProviderInputItem) -> Result<ChatCompletionMessageParam, ProviderError> {
    match item {
        ProviderInputItem::Message { message } => {
            let mut text = String::new();
            let mut parts = Vec::new();
            for content in message.content {
                match content {
                    MessageContent::Text { text: value } => {
                        text.push_str(&value);
                        parts.push(ContentPart::Text { text: value });
                    }
                    MessageContent::ImageDataUrl { data_url, detail } => {
                        parts.push(ContentPart::ImageUrl {
                            image_url: ImageUrl {
                                url: data_url,
                                detail: Some(match detail {
                                    ImageDetail::Auto => OpenAiImageDetail::Auto,
                                    ImageDetail::Low => OpenAiImageDetail::Low,
                                    ImageDetail::High => OpenAiImageDetail::High,
                                }),
                            },
                        });
                    }
                }
            }
            match message.role {
                MessageRole::System => Ok(ChatCompletionMessageParam::System {
                    content: text,
                    name: None,
                }),
                MessageRole::Developer => Ok(ChatCompletionMessageParam::Developer {
                    content: text,
                    name: None,
                }),
                MessageRole::User => Ok(ChatCompletionMessageParam::User {
                    content: if parts
                        .iter()
                        .all(|part| matches!(part, ContentPart::Text { .. }))
                    {
                        UserContent::Text(text)
                    } else {
                        UserContent::Parts(parts)
                    },
                    name: None,
                }),
                MessageRole::Assistant => Ok(ChatCompletionMessageParam::Assistant {
                    content: Some(text),
                    name: None,
                    tool_calls: None,
                    refusal: None,
                }),
            }
        }
        ProviderInputItem::ToolOutput { call_id, output } => Ok(ChatCompletionMessageParam::Tool {
            content: serde_json::to_string(&output)
                .map_err(|error| ProviderError::InvalidRequest(error.to_string()))?,
            tool_call_id: call_id,
        }),
        ProviderInputItem::ToolCall { .. } => Err(ProviderError::InvalidRequest(
            "tool calls must be grouped before message conversion".to_owned(),
        )),
    }
}

#[derive(Default)]
struct ChatAccumulator {
    response_id: Option<String>,
    tool_calls: HashMap<i32, (String, String, String)>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

impl ChatAccumulator {
    fn consume(&mut self, chunk: ChatCompletionChunk) -> Result<Vec<ProviderEvent>, ProviderError> {
        let mut events = Vec::new();
        if self.response_id.is_none() {
            self.response_id = Some(chunk.id.clone());
            events.push(ProviderEvent::Started {
                response_id: chunk.id.clone(),
            });
        }
        if let Some(usage) = chunk.usage {
            self.input_tokens = usage.prompt_tokens;
            self.output_tokens = usage.completion_tokens;
        }
        for choice in chunk.choices {
            if let Some(delta) = choice.delta.content
                && !delta.is_empty()
            {
                events.push(ProviderEvent::TextDelta { delta });
            }
            for call in choice.delta.tool_calls.unwrap_or_default() {
                let entry = self.tool_calls.entry(call.index).or_default();
                if let Some(id) = call.id {
                    entry.0 = id;
                }
                if let Some(function) = call.function {
                    if let Some(name) = function.name {
                        entry.1.push_str(&name);
                    }
                    if let Some(arguments) = function.arguments {
                        entry.2.push_str(&arguments);
                    }
                }
            }
            if matches!(choice.finish_reason, Some(FinishReason::ToolCalls)) {
                events.extend(self.take_tool_calls()?);
            }
        }
        Ok(events)
    }

    fn take_tool_calls(&mut self) -> Result<Vec<ProviderEvent>, ProviderError> {
        let mut calls = self.tool_calls.drain().collect::<Vec<_>>();
        calls.sort_by_key(|(index, _)| *index);
        calls
            .into_iter()
            .map(|(_, (call_id, name, arguments))| {
                if call_id.is_empty() || name.is_empty() {
                    return Err(ProviderError::Request(
                        "provider returned an incomplete tool call".to_owned(),
                    ));
                }
                let arguments = serde_json::from_str(&arguments).map_err(|error| {
                    ProviderError::Request(format!("invalid tool arguments: {error}"))
                })?;
                Ok(ProviderEvent::ToolCall {
                    call_id,
                    name,
                    arguments,
                })
            })
            .collect()
    }

    fn complete(mut self) -> Result<Vec<ProviderEvent>, ProviderError> {
        let mut events = self.take_tool_calls()?;
        events.push(ProviderEvent::Completed {
            response_id: self.response_id.unwrap_or_default(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        });
        Ok(events)
    }
}

#[async_trait]
impl LlmProvider for OpenAiChatProvider {
    async fn stream(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancellation: CancellationToken,
    ) -> Result<(), ProviderError> {
        let request = Self::build_request(request)?;
        let chat = self.client.chat();
        let completions = chat.completions();
        let mut stream = tokio::select! {
            _ = cancellation.cancelled() => {
                events.send(ProviderEvent::Cancelled).await.map_err(|_| ProviderError::ReceiverClosed)?;
                return Ok(());
            }
            result = completions.create_stream(request) => {
                result.map_err(|error| ProviderError::Request(error.to_string()))?
            }
        };
        let mut accumulator = ChatAccumulator::default();
        loop {
            let next = tokio::select! {
                _ = cancellation.cancelled() => {
                    events.send(ProviderEvent::Cancelled).await.map_err(|_| ProviderError::ReceiverClosed)?;
                    return Ok(());
                }
                next = stream.next() => next,
            };
            let Some(chunk) = next else {
                break;
            };
            let chunk = chunk.map_err(|error| ProviderError::Request(error.to_string()))?;
            for event in accumulator.consume(chunk)? {
                events
                    .send(event)
                    .await
                    .map_err(|_| ProviderError::ReceiverClosed)?;
            }
        }
        for event in accumulator.complete()? {
            events
                .send(event)
                .await
                .map_err(|_| ProviderError::ReceiverClosed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use openai_oxide::types::chat::ChatCompletionChunk;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::{ChatAccumulator, OpenAiChatProvider};
    use crate::providers::runtime::LlmProvider;
    use crate::providers::runtime::{
        MessageContent, MessageRole, ProviderEvent, ProviderInputItem, ProviderMessage,
        ProviderRequest, ToolDefinition,
    };

    #[test]
    fn maps_replayable_tool_context_to_chat_messages() {
        let request = OpenAiChatProvider::build_request(ProviderRequest {
            model: "local-model".to_owned(),
            input: vec![
                ProviderInputItem::Message {
                    message: ProviderMessage {
                        role: MessageRole::User,
                        content: vec![MessageContent::Text {
                            text: "Calculate".to_owned(),
                        }],
                    },
                },
                ProviderInputItem::ToolCall {
                    call_id: "call-1".to_owned(),
                    name: "calculator".to_owned(),
                    arguments: serde_json::json!({"left": 2, "right": 3}),
                },
                ProviderInputItem::ToolOutput {
                    call_id: "call-1".to_owned(),
                    output: serde_json::json!({"result": 5}),
                },
            ],
            tools: vec![ToolDefinition {
                name: "calculator".to_owned(),
                description: "Calculate".to_owned(),
                parameters: serde_json::json!({"type": "object"}),
                strict: true,
            }],
            previous_response_id: None,
            store: true,
            reasoning_summary: false,
        })
        .unwrap();
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["messages"][1]["tool_calls"][0]["id"], "call-1");
        assert_eq!(json["messages"][2]["tool_call_id"], "call-1");
        assert_eq!(json["tools"][0]["function"]["strict"], true);
    }

    #[test]
    fn assembles_streamed_tool_call_arguments() {
        let chunks = [
            r#"{"id":"chat-1","object":"chat.completion.chunk","created":1,"model":"test","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"calculator","arguments":"{\"left\":"}}]},"finish_reason":null}]}"#,
            r#"{"id":"chat-1","object":"chat.completion.chunk","created":1,"model":"test","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"2}"}}]},"finish_reason":"tool_calls"}]}"#,
        ];
        let mut accumulator = ChatAccumulator::default();
        let events = chunks
            .into_iter()
            .flat_map(|json| {
                accumulator
                    .consume(serde_json::from_str::<ChatCompletionChunk>(json).unwrap())
                    .unwrap()
            })
            .collect::<Vec<_>>();

        assert!(events.iter().any(|event| matches!(
            event,
            ProviderEvent::ToolCall { call_id, name, arguments }
                if call_id == "call-1" && name == "calculator" && arguments["left"] == 2
        )));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires http://127.0.0.1:11434/v1"]
    async fn streams_from_local_openai_compatible_service() {
        let provider = OpenAiChatProvider::new(
            "local-provider".to_owned(),
            "http://127.0.0.1:11434/v1".to_owned(),
        );
        let request = ProviderRequest {
            model: "phi4-mini:latest".to_owned(),
            input: vec![ProviderInputItem::Message {
                message: ProviderMessage {
                    role: MessageRole::User,
                    content: vec![MessageContent::Text {
                        text: "Reply with exactly LOCAL_ADAPTER_OK".to_owned(),
                    }],
                },
            }],
            tools: Vec::new(),
            previous_response_id: None,
            store: false,
            reasoning_summary: false,
        };
        let (sender, mut receiver) = mpsc::channel(64);
        let task = tokio::spawn(async move {
            provider
                .stream(request, sender, CancellationToken::new())
                .await
        });
        let mut text = String::new();
        while let Some(event) = receiver.recv().await {
            if let ProviderEvent::TextDelta { delta } = event {
                text.push_str(&delta);
            }
        }
        tokio::time::timeout(std::time::Duration::from_secs(60), task)
            .await
            .expect("local service timed out")
            .expect("provider task panicked")
            .expect("local service request failed");
        assert!(
            text.replace(char::is_whitespace, "")
                .contains("LOCAL_ADAPTER_OK")
        );
    }
}
