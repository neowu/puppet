use std::fs;
use std::ops::Not;
use std::path::Path;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::anyhow;
use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::StreamExt;
use log::info;
use reqwest::Response;
use tokio::sync::mpsc;

use super::chatgpt_api::ChatCompletionChoice;
use super::chatgpt_api::ChatResponse;
use super::chatgpt_api::ChatResponseMessage;
use super::chatgpt_api::FunctionCall;
use super::chatgpt_api::ToolCall;
use super::chatgpt_api::Usage;
use crate::azure::chatgpt_api::ChatRequest;
use crate::azure::chatgpt_api::ChatRequestMessage;
use crate::azure::chatgpt_api::ChatStreamResponse;
use crate::azure::chatgpt_api::Role;
use crate::azure::chatgpt_api::Tool;
use crate::llm::function::Function;
use crate::llm::function::FunctionPayload;
use crate::llm::function::FUNCTION_STORE;
use crate::llm::ChatOption;
use crate::llm::TextStream;
use crate::llm::TokenUsage;
use crate::util::http_client::ResponseExt;
use crate::util::http_client::HTTP_CLIENT;
use crate::util::json;
use crate::util::path::PathExt;

pub struct ChatGPT {
    context: Arc<Mutex<Context>>,
}

struct Context {
    url: String,
    api_key: String,
    messages: Arc<Vec<ChatRequestMessage>>,
    tools: Option<Arc<[Tool]>>,
    option: Option<ChatOption>,
    usage: TokenUsage,
}

impl ChatGPT {
    pub fn new(endpoint: String, model: String, api_key: String, functions: Vec<Function>) -> Self {
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-06-01");
        let tools: Option<Arc<[Tool]>> = functions.is_empty().not().then_some(
            functions
                .into_iter()
                .map(|function| Tool {
                    r#type: "function",
                    function,
                })
                .collect(),
        );
        ChatGPT {
            context: Arc::from(Mutex::new(Context {
                url,
                api_key,
                messages: Arc::new(vec![]),
                tools,
                option: None,
                usage: TokenUsage::default(),
            })),
        }
    }

    pub async fn generate(&self) -> Result<TextStream> {
        let (tx, rx) = mpsc::channel(64);
        let context = Arc::clone(&self.context);
        tokio::spawn(async move { process(context, tx).await.unwrap() });
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

    pub fn usage(&self) -> TokenUsage {
        self.context.lock().unwrap().usage.clone()
    }
}

impl Context {
    fn add_message(&mut self, message: ChatRequestMessage) {
        Arc::get_mut(&mut self.messages).unwrap().push(message);
    }
}

async fn process(context: Arc<Mutex<Context>>, tx: mpsc::Sender<String>) -> Result<()> {
    loop {
        let http_response = call_api(Arc::clone(&context)).await?;
        let response = read_sse_response(http_response, &tx).await?;

        let mut context = context.lock().unwrap();
        context.usage.prompt_tokens += response.usage.prompt_tokens;
        context.usage.completion_tokens += response.usage.completion_tokens;

        let message = response.choices.into_iter().next().unwrap().message;

        if let Some(calls) = message.tool_calls {
            let mut functions = Vec::with_capacity(calls.len());
            for call in calls.iter() {
                functions.push(FunctionPayload {
                    id: call.id.to_string(),
                    name: call.function.name.to_string(),
                    value: json::from_json::<serde_json::Value>(&call.function.arguments)?,
                })
            }

            context.add_message(ChatRequestMessage::new_function_call(calls));
            let results = FUNCTION_STORE.lock().unwrap().call(functions)?;

            for result in results {
                context.add_message(ChatRequestMessage::new_function_response(result.id, json::to_json(&result.value)?));
            }
        } else {
            context.add_message(ChatRequestMessage::new_message(Role::Assistant, message.content.unwrap()));
            return Ok(());
        }
    }
}

async fn call_api(context: Arc<Mutex<Context>>) -> Result<Response> {
    let http_request;
    let body;
    {
        let context = context.lock().unwrap();
        let request = ChatRequest {
            messages: Arc::clone(&context.messages),
            temperature: context.option.as_ref().map_or(0.7, |option| option.temperature),
            top_p: 0.95,
            stream: true,
            // stream_options: Some(StreamOptions { include_usage: true }),
            stream_options: None,
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
            .header("api-key", &context.api_key)
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
        _ => Err(anyhow!("not supported extension, path={}", path.to_string_lossy())),
    }?;
    Ok(format!("data:{mime_type};base64,{}", BASE64_STANDARD.encode(content)))
}
