use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Poll;

use anyhow::anyhow;
use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use framework::fs::path::PathExt;
use framework::http_client::ResponseExt;
use framework::http_client::HTTP_CLIENT;
use framework::json;
use framework::json::from_json;
use futures::Stream;
use futures::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tracing::info;

use crate::chat_api::ChatCompletionChoice;
use crate::chat_api::ChatRequest;
use crate::chat_api::ChatRequestMessage;
use crate::chat_api::ChatResponse;
use crate::chat_api::ChatResponseMessage;
use crate::chat_api::ChatStreamResponse;
use crate::chat_api::FunctionCall;
use crate::chat_api::Role;
use crate::chat_api::StreamOptions;
use crate::chat_api::Tool;
use crate::chat_api::ToolCall;
use crate::chat_api::Usage;
use crate::function::FunctionPayload;
use crate::function::FUNCTION_STORE;

pub struct Chat {
    context: Arc<Mutex<Context>>,
}

#[derive(Debug)]
pub struct ChatOption {
    pub temperature: f32,
}

#[derive(Debug)]
struct Context {
    url: String,
    model: String,
    api_key: String,
    messages: Arc<Vec<ChatRequestMessage>>,
    tools: Option<Arc<[Tool]>>,
    option: Option<ChatOption>,
    usage: Arc<Usage>,
}

pub struct TextStream {
    rx: Receiver<String>,
}

impl TextStream {
    pub fn new(rx: Receiver<String>) -> Self {
        TextStream { rx }
    }
}

impl Stream for TextStream {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(context)
    }
}

impl Chat {
    pub fn new(url: String, api_key: String, model: String, tools: Option<Arc<[Tool]>>) -> Self {
        Chat {
            context: Arc::from(Mutex::new(Context {
                url,
                model,
                api_key,
                messages: Arc::new(vec![]),
                tools,
                option: None,
                usage: Arc::new(Usage::default()),
            })),
        }
    }

    pub async fn generate(&self) -> Result<String> {
        let context = Arc::clone(&self.context);
        loop {
            let http_response = call_api(Arc::clone(&context), false).await?;
            let response: ChatResponse = from_json(&http_response.text().await?)?;

            let result = process_chat_response(response, Arc::clone(&context)).unwrap();
            if let Some(content) = result {
                return Ok(content);
            }
        }
    }

    pub async fn generate_stream(&self) -> Result<TextStream> {
        let (tx, rx) = mpsc::channel(64);
        let context = Arc::clone(&self.context);
        tokio::spawn(async move {
            loop {
                let http_response = call_api(Arc::clone(&context), true).await.unwrap();
                let response = read_sse_response(http_response, &tx).await.unwrap();

                let result = process_chat_response(response, Arc::clone(&context)).unwrap();
                if result.is_some() {
                    break;
                }
            }
        });
        Ok(TextStream::new(rx))
    }

    pub fn system_message(&mut self, message: String) {
        let mut context = self.context.lock().unwrap();
        let messages = Arc::get_mut(&mut context.messages).unwrap();
        if let Some(message) = messages.first() {
            if let Role::System = message.role {
                messages.remove(0);
            }
        }
        messages.insert(0, ChatRequestMessage::new_message(Role::System, message))
    }

    pub fn add_user_message(&mut self, message: String, files: &[&Path]) -> Result<()> {
        let image_urls = image_urls(files)?;
        self.context
            .lock()
            .unwrap()
            .add_message(ChatRequestMessage::new_user_message(message, image_urls));
        Ok(())
    }

    pub fn add_assistant_message(&mut self, message: String) {
        self.context
            .lock()
            .unwrap()
            .add_message(ChatRequestMessage::new_message(Role::Assistant, message));
    }

    pub fn option(&mut self, option: ChatOption) {
        self.context.lock().unwrap().option = Some(option);
    }

    pub fn usage(&self) -> Arc<Usage> {
        self.context.lock().unwrap().usage.clone()
    }
}

impl Context {
    fn add_message(&mut self, message: ChatRequestMessage) {
        Arc::get_mut(&mut self.messages).unwrap().push(message);
    }
}

// call function if needed, or return generated content
fn process_chat_response(response: ChatResponse, context: Arc<Mutex<Context>>) -> Result<Option<String>> {
    let mut context = context.lock().unwrap();
    context.usage = Arc::new(response.usage);

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

        context.add_message(ChatRequestMessage::new_function_call(calls));
        let results = FUNCTION_STORE.lock().unwrap().call(functions)?;

        for result in results {
            context.add_message(ChatRequestMessage::new_function_response(result.id, json::to_json(&result.value)?));
        }
        Ok(None)
    } else {
        let content = message.message.content.clone().unwrap();
        context.add_message(ChatRequestMessage::new_message(Role::Assistant, content.clone()));
        Ok(Some(content))
    }
}

async fn call_api(context: Arc<Mutex<Context>>, stream: bool) -> Result<Response> {
    let http_request;
    let body;
    {
        let context = context.lock().unwrap();
        let request = ChatRequest {
            model: context.model.clone(),
            messages: Arc::clone(&context.messages),
            temperature: context.option.as_ref().map_or(0.7, |option| option.temperature),
            top_p: 0.95,
            stream,
            stream_options: stream.then_some(StreamOptions { include_usage: true }),
            stop: None,
            max_tokens: 4096,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            tool_choice: context.tools.is_some().then_some("auto".to_string()),
            tools: context.tools.clone(),
        };

        body = Bytes::from(json::to_json(&request)?);
        http_request = HTTP_CLIENT
            .post(&context.url)
            .header("Content-Type", "application/json")
            .header("api-key", context.api_key.clone()) // azure api use header
            .bearer_auth(context.api_key.clone())
            .body(body.clone());
    }
    let response = http_request.send().await?;
    let status = response.status();
    if status != 200 {
        let body = str::from_utf8(&body)?;
        info!("body={}", body);
        let response_text = response.text().await?;
        return Err(anyhow!("failed to call azure api, status={status}, response={response_text}"));
    }

    Ok(response)
}

async fn read_sse_response(http_response: Response, tx: &mpsc::Sender<String>) -> Result<ChatResponse> {
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

    let mut lines = http_response.lines();
    while let Some(line) = lines.next().await {
        let line = line?;

        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                break;
            }

            let stream_response: ChatStreamResponse = json::from_json(data)?;

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
                    let tool_call = choice.message.tool_calls.as_mut().unwrap().get_mut(stream_call.index as usize).unwrap();
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
    }
    Ok(response)
}

fn image_urls(files: &[&Path]) -> Result<Vec<String>> {
    let mut image_urls = Vec::with_capacity(files.len());
    for file in files {
        image_urls.push(base64_image_url(file)?)
    }
    Ok(image_urls)
}

fn base64_image_url(path: &Path) -> Result<String> {
    let extension = path.file_extension()?;
    let content = fs::read(path)?;
    let mime_type = match extension {
        "jpg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        "pdf" => Ok("application/pdf".to_string()),
        _ => Err(anyhow!("not supported extension, path={}", path.to_string_lossy())),
    }?;
    Ok(format!("data:{mime_type};base64,{}", BASE64_STANDARD.encode(content)))
}
