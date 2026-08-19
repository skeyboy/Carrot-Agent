use std::collections::HashMap;

use async_trait::async_trait;
use futures_util::StreamExt;
use openai_oxide::types::responses::{
    EasyInputContent, ImageDetail as OpenAiImageDetail, InputContent, InputImageContent,
    InputTextContent, OutputItem, ResponseCreateRequest, ResponseInput, ResponseInputItem,
    ResponseStreamEvent, ResponseTool, Role,
};
use openai_oxide::{ClientConfig, OpenAI};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::runtime::{
    ImageDetail, LlmProvider, MessageContent, MessageRole, ProviderError, ProviderEvent,
    ProviderRequest,
};

pub struct OpenAiResponsesProvider {
    client: OpenAI,
}

impl OpenAiResponsesProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: OpenAI::with_config(ClientConfig::new(api_key).base_url(base_url)),
        }
    }

    pub(crate) fn build_request(
        request: ProviderRequest,
    ) -> Result<ResponseCreateRequest, ProviderError> {
        if request.model.trim().is_empty() {
            return Err(ProviderError::InvalidRequest(
                "model cannot be blank".to_owned(),
            ));
        }

        let messages = request
            .messages
            .into_iter()
            .map(|message| {
                let content = message
                    .content
                    .into_iter()
                    .map(|content| match content {
                        MessageContent::Text { text } => {
                            InputContent::InputText(InputTextContent { text })
                        }
                        MessageContent::ImageDataUrl { data_url, detail } => {
                            InputContent::InputImage(InputImageContent {
                                detail: match detail {
                                    ImageDetail::Auto => OpenAiImageDetail::Auto,
                                    ImageDetail::Low => OpenAiImageDetail::Low,
                                    ImageDetail::High => OpenAiImageDetail::High,
                                },
                                file_id: None,
                                image_url: Some(data_url),
                            })
                        }
                    })
                    .collect::<Vec<_>>();
                let content = serde_json::to_value(EasyInputContent::ContentList(content))
                    .map_err(|error| ProviderError::InvalidRequest(error.to_string()))?;
                Ok(ResponseInputItem {
                    role: match message.role {
                        MessageRole::System => Role::System,
                        MessageRole::Developer => Role::Developer,
                        MessageRole::User => Role::User,
                        MessageRole::Assistant => Role::Assistant,
                    },
                    content,
                })
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;

        let mut output = ResponseCreateRequest::new(request.model);
        output.input = Some(ResponseInput::Messages(messages));
        output.previous_response_id = request.previous_response_id;
        output.store = Some(request.store);
        output.parallel_tool_calls = Some(true);
        if !request.tools.is_empty() {
            output.tools = Some(
                request
                    .tools
                    .into_iter()
                    .map(|tool| ResponseTool::Function {
                        name: tool.name,
                        description: Some(tool.description),
                        parameters: Some(tool.parameters),
                        strict: Some(tool.strict),
                    })
                    .collect(),
            );
        }
        Ok(output)
    }
}

#[async_trait]
impl LlmProvider for OpenAiResponsesProvider {
    async fn stream(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancellation: CancellationToken,
    ) -> Result<(), ProviderError> {
        let request = Self::build_request(request)?;
        let responses = self.client.responses();
        let mut stream = tokio::select! {
            _ = cancellation.cancelled() => {
                events.send(ProviderEvent::Cancelled).await.map_err(|_| ProviderError::ReceiverClosed)?;
                return Ok(());
            }
            result = responses.create_stream(request) => {
                result.map_err(|error| ProviderError::Request(error.to_string()))?
            }
        };
        let mut pending_calls: HashMap<i64, (String, String)> = HashMap::new();

        loop {
            let next = tokio::select! {
                _ = cancellation.cancelled() => {
                    events.send(ProviderEvent::Cancelled).await.map_err(|_| ProviderError::ReceiverClosed)?;
                    return Ok(());
                }
                next = stream.next() => next,
            };
            let Some(event) = next else {
                return Ok(());
            };
            let event = event.map_err(|error| ProviderError::Request(error.to_string()))?;
            let mapped = match event {
                ResponseStreamEvent::ResponseCreated { response } => Some(ProviderEvent::Started {
                    response_id: response.id,
                }),
                ResponseStreamEvent::ResponseOutputTextDelta(event) => {
                    Some(ProviderEvent::TextDelta { delta: event.delta })
                }
                ResponseStreamEvent::ResponseOutputItemAdded(event) => {
                    if let OutputItem::FunctionCall(call) = event.item {
                        pending_calls.insert(event.output_index, (call.call_id, call.name));
                    }
                    None
                }
                ResponseStreamEvent::ResponseFunctionCallArgumentsDone(event) => {
                    let (call_id, name) = pending_calls
                        .remove(&i64::from(event.output_index))
                        .unwrap_or_default();
                    let arguments = serde_json::from_str(&event.arguments).map_err(|error| {
                        ProviderError::Request(format!("invalid tool arguments: {error}"))
                    })?;
                    Some(ProviderEvent::ToolCall {
                        call_id,
                        name: event.name.unwrap_or(name),
                        arguments,
                    })
                }
                ResponseStreamEvent::ResponseCompleted(event) => {
                    let usage = event.response.usage;
                    Some(ProviderEvent::Completed {
                        response_id: event.response.id,
                        input_tokens: usage.as_ref().and_then(|usage| usage.input_tokens),
                        output_tokens: usage.as_ref().and_then(|usage| usage.output_tokens),
                    })
                }
                ResponseStreamEvent::ResponseFailed(event) => Some(ProviderEvent::Failed {
                    message: event
                        .response
                        .error
                        .map(|error| error.message)
                        .unwrap_or_else(|| "response failed".to_owned()),
                }),
                ResponseStreamEvent::ResponseIncomplete(_) => Some(ProviderEvent::Failed {
                    message: "response was incomplete".to_owned(),
                }),
                ResponseStreamEvent::ResponseError(event) => Some(ProviderEvent::Failed {
                    message: event.message,
                }),
                _ => None,
            };

            if let Some(event) = mapped {
                let terminal = matches!(
                    event,
                    ProviderEvent::Completed { .. } | ProviderEvent::Failed { .. }
                );
                events
                    .send(event)
                    .await
                    .map_err(|_| ProviderError::ReceiverClosed)?;
                if terminal {
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OpenAiResponsesProvider;
    use crate::providers::runtime::{
        ImageDetail, MessageContent, MessageRole, ProviderMessage, ProviderRequest, ToolDefinition,
    };

    #[test]
    fn maps_images_tools_and_store_to_responses_request() {
        let request = OpenAiResponsesProvider::build_request(ProviderRequest {
            model: "gpt-test".to_owned(),
            messages: vec![ProviderMessage {
                role: MessageRole::User,
                content: vec![
                    MessageContent::Text {
                        text: "Inspect".to_owned(),
                    },
                    MessageContent::ImageDataUrl {
                        data_url: "data:image/png;base64,AA==".to_owned(),
                        detail: ImageDetail::Auto,
                    },
                ],
            }],
            tools: vec![ToolDefinition {
                name: "lookup".to_owned(),
                description: "Lookup a record".to_owned(),
                parameters: serde_json::json!({"type": "object"}),
                strict: true,
            }],
            previous_response_id: None,
            store: true,
        })
        .unwrap();
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["store"], true);
        assert_eq!(json["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(json["tools"][0]["name"], "lookup");
    }
}
