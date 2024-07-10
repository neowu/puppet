use std::collections::HashMap;
use std::ops::Not;
use std::rc::Rc;
use std::str;
use std::str::Utf8Error;

use bytes::Bytes;
use reqwest::Response;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tracing::info;

use crate::azure::chatgpt_api::ChatRequest;
use crate::azure::chatgpt_api::ChatRequestMessage;
use crate::azure::chatgpt_api::ChatResponse;
use crate::azure::chatgpt_api::Role;
use crate::azure::chatgpt_api::Tool;
use crate::llm::function::FunctionStore;
use crate::llm::ChatEvent;
use crate::llm::ChatHandler;
use crate::llm::Usage;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct ChatGPT {
    url: String,
    api_key: String,
    messages: Rc<Vec<ChatRequestMessage>>,
    tools: Option<Rc<[Tool]>>,
    function_store: FunctionStore,
}

type FunctionCall = HashMap<i64, (String, String, String)>;

impl ChatGPT {
    pub fn new(endpoint: String, model: String, api_key: String, system_message: Option<String>, function_store: FunctionStore) -> Self {
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-06-01");
        let tools: Option<Rc<[Tool]>> = function_store.declarations.is_empty().not().then_some(
            function_store
                .declarations
                .iter()
                .map(|f| Tool {
                    r#type: "function".to_string(),
                    function: f.clone(),
                })
                .collect(),
        );
        let mut chatgpt = ChatGPT {
            url,
            api_key,
            messages: Rc::new(vec![]),
            tools,
            function_store,
        };
        if let Some(message) = system_message {
            chatgpt.add_message(ChatRequestMessage::new_message(Role::System, message));
        }
        chatgpt
    }

    pub async fn chat(&mut self, message: String, handler: &impl ChatHandler) -> Result<(), Exception> {
        self.add_message(ChatRequestMessage::new_message(Role::User, message));

        let result = self.process(handler).await?;
        if let Some(calls) = result {
            self.add_message(ChatRequestMessage::new_function_call(&calls));

            let mut functions = Vec::with_capacity(calls.len());
            for (_, (id, name, args)) in calls {
                functions.push((id, name, json::from_json::<serde_json::Value>(&args)?))
            }

            let results = self.function_store.call_functions(functions).await?;
            for result in results {
                let function_response = ChatRequestMessage::new_function_response(result.0, json::to_json(&result.1)?);
                self.add_message(function_response);
            }
            self.process(handler).await?;
        }
        Ok(())
    }

    fn add_message(&mut self, message: ChatRequestMessage) {
        Rc::get_mut(&mut self.messages).unwrap().push(message);
    }

    async fn process(&mut self, handler: &impl ChatHandler) -> Result<Option<FunctionCall>, Exception> {
        let (tx, rx) = channel(64);

        let response = self.call_api().await?;
        let handle = tokio::spawn(read_sse(response, tx));
        let function_call = self.process_response(rx, handler).await?;
        handle.await??;

        Ok(function_call)
    }

    async fn process_response(&mut self, mut rx: Receiver<ChatResponse>, handler: &impl ChatHandler) -> Result<Option<FunctionCall>, Exception> {
        let mut function_calls: FunctionCall = HashMap::new();
        let mut usage = Usage::default();
        let mut assistant_message = String::new();

        while let Some(response) = rx.recv().await {
            if let Some(choice) = response.choices.into_iter().next() {
                let delta = choice.delta.unwrap();

                if let Some(tool_calls) = delta.tool_calls {
                    let call = tool_calls.into_iter().next().unwrap();
                    if let Some(name) = call.function.name {
                        function_calls.insert(call.index, (call.id.unwrap(), name, String::new()));
                    }
                    function_calls.get_mut(&call.index).unwrap().2.push_str(&call.function.arguments)
                } else if let Some(value) = delta.content {
                    assistant_message.push_str(&value);
                    handler.on_event(ChatEvent::Delta(value));
                }
            }

            if let Some(value) = response.usage {
                usage = Usage {
                    request_tokens: value.prompt_tokens,
                    response_tokens: value.completion_tokens,
                };
            }
        }

        if !assistant_message.is_empty() {
            self.add_message(ChatRequestMessage::new_message(Role::Assistant, assistant_message));
        }

        if !function_calls.is_empty() {
            Ok(Some(function_calls))
        } else {
            handler.on_event(ChatEvent::End(usage));
            Ok(None)
        }
    }

    async fn call_api(&mut self) -> Result<Response, Exception> {
        let request = ChatRequest {
            messages: Rc::clone(&self.messages),
            temperature: 0.7,
            top_p: 0.95,
            stream: true,
            // stream_options: Some(StreamOptions { include_usage: true }),
            stream_options: None,
            stop: None,
            max_tokens: 800,
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
            let body = str::from_utf8(&body).unwrap();
            info!("body={}", body);
            let response_text = response.text().await?;
            return Err(Exception::ExternalError(format!(
                "failed to call azure api, status={status}, response={response_text}"
            )));
        }

        Ok(response)
    }
}

async fn read_sse(response: Response, tx: Sender<ChatResponse>) -> Result<(), Exception> {
    let mut buffer = String::with_capacity(1024);
    let mut response = response;
    while let Some(chunk) = response.chunk().await? {
        buffer.push_str(str::from_utf8(&chunk)?);

        while let Some(index) = buffer.find("\n\n") {
            if buffer.starts_with("data:") {
                let data = &buffer[6..index];

                if data == "[DONE]" {
                    return Ok(());
                }

                let response: ChatResponse = json::from_json(data)?;
                tx.send(response).await?;

                buffer.replace_range(0..index + 2, "");
            } else {
                return Err(Exception::unexpected(format!("unexpected sse message, buffer={}", buffer)));
            }
        }
    }
    Ok(())
}

impl From<Utf8Error> for Exception {
    fn from(err: Utf8Error) -> Self {
        Exception::unexpected(err)
    }
}