use std::ops::Not;
use std::path::Path;
use std::rc::Rc;
use std::str;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use reqwest::Response;
use tokio::fs;
use tracing::info;

use super::chatgpt_api::ChatCompletionChoice;
use super::chatgpt_api::ChatResponse;
use super::chatgpt_api::ChatResponseMessage;
use super::chatgpt_api::ToolCall;
use super::chatgpt_api::Usage;
use crate::azure::chatgpt_api::ChatRequest;
use crate::azure::chatgpt_api::ChatRequestMessage;
use crate::azure::chatgpt_api::ChatStreamResponse;
use crate::azure::chatgpt_api::Role;
use crate::azure::chatgpt_api::Tool;
use crate::llm::function::FunctionImplementations;
use crate::llm::function::FunctionObject;
use crate::llm::function::FunctionStore;
use crate::llm::ChatOption;
use crate::util::console;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct ChatGPT {
    url: String,
    api_key: String,
    messages: Rc<Vec<ChatRequestMessage>>,
    tools: Option<Rc<[Tool]>>,
    implementations: FunctionImplementations,
    pub option: Option<ChatOption>,
}

impl ChatGPT {
    pub fn new(endpoint: String, model: String, api_key: String, function_store: FunctionStore) -> Self {
        let FunctionStore {
            declarations,
            implementations,
        } = function_store;

        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-06-01");
        let tools: Option<Rc<[Tool]>> = declarations.is_empty().not().then_some(
            declarations
                .into_iter()
                .map(|function| Tool {
                    r#type: "function",
                    function,
                })
                .collect(),
        );
        ChatGPT {
            url,
            api_key,
            messages: Rc::new(vec![]),
            tools,
            implementations,
            option: None,
        }
    }

    pub async fn chat(&mut self) -> Result<&str, Exception> {
        self.process().await?;

        Ok(self
            .messages
            .last()
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .first()
            .unwrap()
            .text
            .as_ref()
            .unwrap())
    }

    pub fn system_message(&mut self, message: String) {
        let messages = Rc::get_mut(&mut self.messages).unwrap();
        if let Some(message) = messages.first() {
            if let Role::System = message.role {
                messages.remove(0);
            }
        }
        messages.insert(0, ChatRequestMessage::new_message(Role::System, message))
    }

    pub async fn add_user_message(&mut self, message: String, files: &[&Path]) -> Result<(), Exception> {
        let image_urls = image_urls(files).await?;
        self.add_message(ChatRequestMessage::new_user_message(message, image_urls));
        Ok(())
    }

    pub fn add_assistant_message(&mut self, message: String) {
        self.add_message(ChatRequestMessage::new_message(Role::Assistant, message));
    }

    async fn process(&mut self) -> Result<(), Exception> {
        loop {
            let http_response = self.call_api().await?;
            let response = read_sse_response(http_response).await?;
            info!(
                "usage, prompt_tokens={}, completion_tokens={}",
                response.usage.prompt_tokens, response.usage.completion_tokens
            );

            let message = response.choices.into_iter().next().unwrap().message;

            if let Some(calls) = message.tool_calls {
                let mut functions = Vec::with_capacity(calls.len());
                for call in calls.iter() {
                    functions.push(FunctionObject {
                        id: call.id.to_string(),
                        name: call.function.name.to_string(),
                        value: json::from_json::<serde_json::Value>(&call.function.arguments)?,
                    })
                }
                self.add_message(ChatRequestMessage::new_function_call(calls));

                let results = self.implementations.call_functions(functions).await?;
                for result in results {
                    self.add_message(ChatRequestMessage::new_function_response(result.id, json::to_json(&result.value)?));
                }
            } else {
                self.add_message(ChatRequestMessage::new_message(Role::Assistant, message.content.unwrap()));
                return Ok(());
            }
        }
    }

    async fn call_api(&mut self) -> Result<Response, Exception> {
        let request = ChatRequest {
            messages: Rc::clone(&self.messages),
            temperature: self.option.as_ref().map_or(0.7, |option| option.temperature),
            top_p: 0.95,
            stream: true,
            // stream_options: Some(StreamOptions { include_usage: true }),
            stream_options: None,
            stop: None,
            max_tokens: 4096,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            tool_choice: self.tools.is_some().then_some("auto".to_string()),
            tools: self.tools.clone(),
        };

        let body = json::to_json(&request)?;
        let body = Bytes::from(body);
        let request = http_client::http_client()
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.api_key)
            .body(body.clone());

        let response = request.send().await?;
        let status = response.status();
        if status != 200 {
            let body = str::from_utf8(&body)?;
            info!("body={}", body);
            let response_text = response.text().await?;
            return Err(Exception::ExternalError(format!(
                "failed to call azure api, status={status}, response={response_text}"
            )));
        }

        Ok(response)
    }

    fn add_message(&mut self, message: ChatRequestMessage) {
        Rc::get_mut(&mut self.messages).unwrap().push(message);
    }
}

async fn read_sse_response(mut http_response: Response) -> Result<ChatResponse, Exception> {
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

    let mut buffer = String::with_capacity(1024);
    while let Some(chunk) = http_response.chunk().await? {
        buffer.push_str(str::from_utf8(&chunk)?);

        while let Some(index) = buffer.find("\n\n") {
            if buffer.starts_with("data:") {
                let data = &buffer[6..index];

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
                                function: super::chatgpt_api::FunctionCall {
                                    name,
                                    arguments: String::new(),
                                },
                            });
                        }
                        let tool_call = choice.message.tool_calls.as_mut().unwrap().get_mut(stream_call.index as usize).unwrap();
                        tool_call.function.arguments.push_str(&stream_call.function.arguments);
                    } else if let Some(content) = stream_choice.delta.content {
                        choice.append_content(&content);
                        console::print(&content).await?;
                    }

                    if let Some(finish_reason) = stream_choice.finish_reason {
                        choice.finish_reason = finish_reason;
                        if choice.finish_reason == "stop" {
                            console::print("\n").await?;
                        }
                    }
                }

                if let Some(usage) = stream_response.usage {
                    response.usage = usage;
                }

                buffer.replace_range(0..index + 2, "");
            } else {
                return Err(Exception::unexpected(format!("unexpected sse message, buffer={}", buffer)));
            }
        }
    }
    Ok(response)
}

async fn image_urls(files: &[&Path]) -> Result<Vec<String>, Exception> {
    let mut image_urls = Vec::with_capacity(files.len());
    for file in files {
        image_urls.push(base64_image_url(file).await?)
    }
    Ok(image_urls)
}

async fn base64_image_url(path: &Path) -> Result<String, Exception> {
    let extension = path
        .extension()
        .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", path.to_string_lossy())))?
        .to_str()
        .unwrap();
    let content = fs::read(path).await?;
    let mime_type = match extension {
        "jpg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        _ => Err(Exception::ValidationError(format!(
            "not supported extension, path={}",
            path.to_string_lossy()
        ))),
    }?;
    Ok(format!("data:{mime_type};base64,{}", BASE64_STANDARD.encode(content)))
}
