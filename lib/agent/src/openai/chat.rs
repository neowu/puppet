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
use framework::task;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;
use tracing::debug;

use crate::openai::chat_api::ChatCompletionChoice;
use crate::openai::chat_api::ChatRequest;
use crate::openai::chat_api::ChatRequestMessage;
use crate::openai::chat_api::ChatResponse;
use crate::openai::chat_api::ChatResponseMessage;
use crate::openai::chat_api::ChatStreamResponse;
use crate::openai::chat_api::FunctionCall;
use crate::openai::chat_api::Role;
use crate::openai::chat_api::StreamOptions;
use crate::openai::chat_api::Tool;
use crate::openai::chat_api::ToolCall;
use crate::openai::chat_api::Usage;
use crate::openai::function::FunctionPayload;
use crate::openai::function::FunctionStore;
use crate::openai::session::Session;

pub struct Chat {
    model: Arc<Model>,
    function_store: Arc<FunctionStore>,
    http_client: HttpClient,
}

pub struct Model {
    url: String,
    model: String,
    api_key: String,
}

impl Chat {
    pub fn new(
        url: String,
        api_key: String,
        model: String,
        function_store: Arc<FunctionStore>,
        http_client: HttpClient,
    ) -> Self {
        let model = Arc::new(Model { url, model, api_key });
        Chat {
            model,
            http_client,
            function_store,
        }
    }

    pub async fn generate(&self, session: Arc<Mutex<Session>>) -> Result<String, Exception> {
        let tools = self.function_store.definitions(&session.lock().unwrap().functions);
        loop {
            let http_request = openai_request(&self.model, &session, &tools, false)?;
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
            let result = process_chat_response(response, &session, &self.function_store).unwrap();
            if let Some(content) = result {
                return Ok(content);
            }
        }
    }

    pub async fn generate_stream(
        &self,
        session: Arc<Mutex<Session>>,
    ) -> Result<impl Stream<Item = Result<String, Exception>>, Exception> {
        let (tx, rx) = mpsc::channel(64);

        let tools = self.function_store.definitions(&session.lock().unwrap().functions);
        let function_store = Arc::clone(&self.function_store);
        let http_client = self.http_client.clone();

        let model = self.model.clone();
        task::spawn_task(async move {
            loop {
                let result = process_sse(&model, &session, &tx, &tools, &function_store, &http_client).await;
                match result {
                    Ok(Some(_)) => return Ok(()),
                    Ok(None) => {
                        continue;
                    }
                    Err(error) => {
                        tx.send(Err(error)).await?;
                        return Ok(());
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}

async fn process_sse(
    model: &Arc<Model>,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Result<String, Exception>>,
    tools: &Option<Vec<Tool>>,
    function_store: &Arc<FunctionStore>,
    http_client: &HttpClient,
) -> Result<Option<String>, Exception> {
    let http_request = openai_request(model, session, tools, true)?;
    let event_source = http_client.sse(http_request).await?;
    let response = read_sse_response(event_source, tx).await?;
    debug!(
        "usage, prompt_tokens={}, completion_tokens={}",
        response.usage.prompt_tokens, response.usage.completion_tokens
    );
    let result = process_chat_response(response, session, function_store)?;
    Ok(result)
}

fn openai_request(
    model: &Arc<Model>,
    session: &Arc<Mutex<Session>>,
    tools: &Option<Vec<Tool>>,
    stream: bool,
) -> Result<HttpRequest, Exception> {
    let session = session.lock().unwrap();
    let request = ChatRequest {
        model: model.model.to_string(),
        messages: session.messages.clone(),
        temperature: session.temperature.unwrap_or(1.0),
        top_p: session.top_p.unwrap_or(1.0),
        stream,
        stream_options: stream.then_some(StreamOptions { include_usage: true }),
        stop: None,
        max_completion_tokens: session.max_completion_tokens,
        presence_penalty: 0.0,
        frequency_penalty: 0.0,
        tool_choice: tools.is_some().then_some("auto"),
        tools: tools.clone(),
        response_format: session.response_format.clone(),
        prediction: None,
    };
    let mut http_request = HttpRequest::new(POST, &model.url);
    http_request.body(json::to_json(&request)?, "application/json");
    http_request
        .headers
        .insert(HeaderName::from_static("api-key"), model.api_key.to_string());
    Ok(http_request)
}

// call function if needed, or return generated content
fn process_chat_response(
    response: ChatResponse,
    session: &Arc<Mutex<Session>>,
    function_store: &Arc<FunctionStore>,
) -> Result<Option<String>, Exception> {
    let mut session = session.lock().unwrap();

    let message = response.choices.into_iter().next().unwrap();
    if let Some(calls) = message.message.tool_calls {
        let mut functions = Vec::with_capacity(calls.len());
        for call in calls.iter() {
            let id = call.id.to_string();
            let name = call.function.name.to_string();
            let value = json::from_json(&call.function.arguments)?;
            debug!(function_id = id, "[chat] function_call: {name}({value})");
            functions.push(FunctionPayload { id, name, value })
        }

        session.messages.push(ChatRequestMessage::new_function_call(calls));
        let results = function_store.call(functions)?;

        for result in results {
            let id = result.id;
            let value = json::to_json(&result.value)?;
            debug!(function_id = id, "[chat] function_result: {value}");
            session
                .messages
                .push(ChatRequestMessage::new_function_response(id, value));
        }
        Ok(None)
    } else {
        let content = message.message.content.clone().unwrap();
        debug!("[chat] assistant: {content}");
        session
            .messages
            .push(ChatRequestMessage::new_message(Role::Assistant, content.clone()));
        Ok(Some(content))
    }
}

async fn read_sse_response(
    mut event_source: EventSource,
    tx: &Sender<Result<String, Exception>>,
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

        if event.data == "[DONE]" {
            break;
        }

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
                tx.send(Ok(content)).await?;
            }

            if let Some(finish_reason) = stream_choice.finish_reason {
                choice.finish_reason = finish_reason;
                if choice.finish_reason == "stop" {
                    // chatgpt doesn't return '\n' at end of message
                    tx.send(Ok("\n".to_string())).await?;
                }
            }
        }

        if let Some(usage) = stream_response.usage {
            response.usage = usage;
        }
    }
    Ok(response)
}
