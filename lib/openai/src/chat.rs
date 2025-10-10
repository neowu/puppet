use std::sync::Arc;
use std::sync::Mutex;

use framework::exception;
use framework::exception::Exception;
use framework::http::EventSource;
use framework::http::HeaderName;
use framework::http::HttpClient;
use framework::http::HttpMethod::POST;
use framework::http::HttpRequest;
use framework::json;
use framework::json::from_json;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::debug;

use crate::api_key;
use crate::chat_api::ChatCompletionChoice;
use crate::chat_api::ChatRequest;
use crate::chat_api::ChatRequestMessage;
use crate::chat_api::ChatResponse;
use crate::chat_api::ChatResponseMessage;
use crate::chat_api::ChatStreamResponse;
use crate::chat_api::FunctionCall;
use crate::chat_api::ResponseFormat;
use crate::chat_api::Role;
use crate::chat_api::StreamOptions;
use crate::chat_api::Tool;
use crate::chat_api::ToolCall;
use crate::chat_api::Usage;
use crate::function::FunctionPayload;
use crate::function::FunctionStore;

pub struct Chat {
    http_client: HttpClient,
    function_store: Arc<FunctionStore>,
    pub config: ChatConfig,
}

#[derive(Default, Debug, Clone)]
pub struct ChatConfig {
    url: String,
    model: String,
    api_key: String,

    pub system_message: Option<String>,
    pub top_p: Option<f32>,
    pub temperature: Option<f32>,
    pub response_format: Option<ResponseFormat>,
    pub max_tokens: Option<i32>,
}

impl Chat {
    pub fn new(url: String, api_key: String, model: String, function_store: FunctionStore) -> Self {
        Chat {
            http_client: HttpClient::default(),
            config: ChatConfig {
                url,
                model,
                api_key,
                ..ChatConfig::default()
            },
            function_store: Arc::new(function_store),
        }
    }

    pub async fn generate(
        &self,
        messages: Arc<Mutex<Vec<ChatRequestMessage>>>,
        prediction: Option<String>,
    ) -> Result<String, Exception> {
        let mut prediction_value = prediction;
        let tools = self.function_store.definitions();
        loop {
            let http_request = request(
                &self.config,
                Arc::clone(&messages),
                tools.clone(),
                false,
                prediction_value,
            )?;
            let http_response = self.http_client.execute(http_request).await?;
            if http_response.status != 200 {
                return Err(exception!(
                    message = format!("failed to call openai api, status={}", http_response.status)
                ));
            }
            let response: ChatResponse = from_json(&http_response.body)?;
            debug!(
                "usage, prompt_tokens={}, completion_tokens={}",
                response.usage.prompt_tokens, response.usage.completion_tokens
            );

            let result =
                process_chat_response(response, Arc::clone(&messages), Arc::clone(&self.function_store)).unwrap();
            if let Some(content) = result {
                return Ok(content);
            }
            prediction_value = None; // prediction only used once without function call
        }
    }

    pub async fn generate_stream(
        &self,
        messages: Arc<Mutex<Vec<ChatRequestMessage>>>,
    ) -> Result<impl Stream<Item = String>, Exception> {
        let (tx, rx) = mpsc::channel(64);

        let tools = self.function_store.definitions();
        let function_store = Arc::clone(&self.function_store);

        let http_client = self.http_client.clone();
        let config = self.config.clone();
        tokio::spawn(async move {
            loop {
                let http_request = request(&config, Arc::clone(&messages), tools.clone(), true, None)?;
                let event_source = http_client.sse(http_request).await?;
                let response = read_sse_response(event_source, &tx).await?;
                debug!(
                    "usage, prompt_tokens={}, completion_tokens={}",
                    response.usage.prompt_tokens, response.usage.completion_tokens
                );
                let result = process_chat_response(response, Arc::clone(&messages), Arc::clone(&function_store))?;
                if result.is_some() {
                    return Ok::<_, Exception>(());
                }
            }
        });
        Ok(ReceiverStream::new(rx))
    }
}

fn request(
    config: &ChatConfig,
    messages: Arc<Mutex<Vec<ChatRequestMessage>>>,
    tools: Option<Vec<Tool>>,
    stream: bool,
    prediction: Option<String>,
) -> Result<HttpRequest, Exception> {
    let request_messages = request_messages(messages, config);
    let request = ChatRequest {
        model: config.model.clone(),
        messages: request_messages,
        temperature: config.temperature.unwrap_or(1.0),
        top_p: config.top_p.unwrap_or(1.0),
        stream,
        stream_options: stream.then_some(StreamOptions { include_usage: true }),
        stop: None,
        max_completion_tokens: None,
        presence_penalty: 0.0,
        frequency_penalty: 0.0,
        tool_choice: tools.is_some().then_some("auto"),
        tools,
        response_format: config.response_format.clone(),
        prediction: prediction.map(|content| crate::chat_api::Prediction {
            r#type: "content",
            content,
        }),
    };
    let mut http_request = HttpRequest::new(POST, &config.url);
    http_request.body(json::to_json(&request)?, "application/json");
    let api_key = api_key(&config.api_key)?;
    http_request.headers.insert(HeaderName::from_static("api-key"), api_key);
    Ok(http_request)
}

// call function if needed, or return generated content
fn process_chat_response(
    response: ChatResponse,
    messages: Arc<Mutex<Vec<ChatRequestMessage>>>,
    function_store: Arc<FunctionStore>,
) -> Result<Option<String>, Exception> {
    let mut messages = messages.lock().unwrap();
    let message = response.choices.into_iter().next().unwrap();
    if let Some(calls) = message.message.tool_calls {
        let mut functions = Vec::with_capacity(calls.len());
        for call in calls.iter() {
            functions.push(FunctionPayload {
                id: call.id.to_string(),
                name: call.function.name.to_string(),
                value: json::from_json(&call.function.arguments)?,
            })
        }

        messages.push(ChatRequestMessage::new_function_call(calls));
        let results = function_store.call(functions)?;

        for result in results {
            messages.push(ChatRequestMessage::new_function_response(
                result.id,
                json::to_json(&result.value)?,
            ));
        }
        Ok(None)
    } else {
        let content = message.message.content.clone().unwrap();
        messages.push(ChatRequestMessage::new_message(Role::Assistant, content.clone()));
        Ok(Some(content))
    }
}

fn request_messages(messages: Arc<Mutex<Vec<ChatRequestMessage>>>, config: &ChatConfig) -> Vec<ChatRequestMessage> {
    let messages = messages.lock().unwrap();
    if let Some(ref system_message) = config.system_message {
        let mut request_messages = Vec::with_capacity(messages.len() + 1);
        request_messages.push(ChatRequestMessage::new_message(Role::System, system_message.clone()));
        request_messages.extend(messages.clone());
        request_messages
    } else {
        messages.clone()
    }
}

async fn read_sse_response(
    mut event_source: EventSource,
    tx: &mpsc::Sender<String>,
) -> Result<ChatResponse, Exception> {
    let mut response = ChatResponse {
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: ChatResponseMessage {
                content: None,
                tool_calls: None,
            },
            finish_reason: String::new(),
        }],
        usage: Usage::default(),
    };

    // only support one choice, n=1
    let choice = response.choices.first_mut().unwrap();

    while let Some(event) = event_source.next().await {
        let event = event?;

        let stream_response: ChatStreamResponse = json::from_json(&event.data)?;

        if let Some(stream_choice) = stream_response.choices.into_iter().next() {
            choice.index = stream_choice.index;

            if let Some(stream_calls) = stream_choice.delta.tool_calls {
                if choice.message.tool_calls.is_none() {
                    choice.message.tool_calls = Some(vec![]);
                }

                // stream tool call only return single element
                let stream_call = stream_calls.into_iter().next().unwrap();
                if let Some(name) = stream_call.function.name {
                    choice.message.tool_calls.as_mut().unwrap().push(ToolCall {
                        id: stream_call.id.unwrap(),
                        r#type: "function".to_string(),
                        function: FunctionCall {
                            name,
                            arguments: String::new(),
                        },
                    });
                }
                let tool_call = choice
                    .message
                    .tool_calls
                    .as_mut()
                    .unwrap()
                    .get_mut(stream_call.index as usize)
                    .unwrap();
                tool_call.function.arguments.push_str(&stream_call.function.arguments);
            } else if let Some(content) = stream_choice.delta.content {
                choice.append_content(&content);
                tx.send(content).await?;
            }

            if let Some(finish_reason) = stream_choice.finish_reason {
                choice.finish_reason = finish_reason;
                if choice.finish_reason == "stop" {
                    // chatgpt doesn't return '\n' at end of message
                    tx.send("\n".to_string()).await?;
                }
            }
        }

        if let Some(usage) = stream_response.usage {
            response.usage = usage;
        }
    }
    Ok(response)
}
