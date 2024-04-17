use std::borrow::Cow;
use std::collections::HashMap;

use std::fmt;
use std::sync::Arc;

use futures::future::join_all;
use futures::stream::StreamExt;
use reqwest_eventsource::CannotCloneRequestError;
use reqwest_eventsource::Event;
use reqwest_eventsource::EventSource;
use serde::Serialize;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

use crate::bot::ChatEvent;
use crate::bot::ChatHandler;
use crate::bot::Function;
use crate::bot::FunctionImplementation;
use crate::openai::api::ChatRequest;
use crate::openai::api::ChatRequestMessage;
use crate::openai::api::ChatResponse;
use crate::openai::api::Role;
use crate::openai::api::Tool;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct ChatGPT {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    messages: Vec<ChatRequestMessage>,
    tools: Vec<Tool>,
    function_implementations: HashMap<String, Arc<Box<FunctionImplementation>>>,
}

enum InternalEvent {
    Event(ChatEvent),
    FunctionCall(FunctionCall),
}

type FunctionCall = HashMap<i64, (String, String, String)>;

impl ChatGPT {
    pub fn new(endpoint: String, api_key: String, model: String, system_message: Option<String>) -> Self {
        let mut chatgpt = ChatGPT {
            endpoint,
            api_key,
            model,
            messages: vec![],
            tools: vec![],
            function_implementations: HashMap::new(),
        };
        if let Some(message) = system_message {
            chatgpt.messages.push(ChatRequestMessage::new_message(Role::System, &message));
        }
        chatgpt
    }

    pub fn register_function(&mut self, function: Function, implementation: Box<FunctionImplementation>) {
        let name = function.name.to_string();
        self.tools.push(Tool {
            r#type: "function".to_string(),
            function,
        });
        self.function_implementations.insert(name, Arc::new(implementation));
    }

    pub async fn chat(&mut self, message: &str, handler: &dyn ChatHandler) -> Result<(), Exception> {
        self.messages.push(ChatRequestMessage::new_message(Role::User, message));
        let result = self.process(handler).await;
        if let Ok(Some(InternalEvent::FunctionCall(calls))) = result {
            let handles: Result<Vec<JoinHandle<_>>, _> = calls
                .into_iter()
                .map(|(_, (id, name, args))| {
                    let function = self.get_function(&name)?;
                    Ok::<JoinHandle<_>, Exception>(tokio::spawn(async move { (id, function(json::from_json(&args).unwrap())) }))
                })
                .collect();

            let results: Result<Vec<_>, _> = join_all(handles?).await.into_iter().collect();
            for result in results? {
                let function_message = ChatRequestMessage::new_function_response(result.0.to_string(), json::to_json(&result.1)?);
                self.messages.push(function_message);
            }

            self.process(handler).await?;
        }
        Ok(())
    }

    fn get_function(&mut self, name: &str) -> Result<Arc<Box<FunctionImplementation>>, Exception> {
        let function = Arc::clone(
            self.function_implementations
                .get(name)
                .ok_or_else(|| Exception::new(&format!("function not found, name={name}")))?,
        );
        Ok(function)
    }

    async fn process(&mut self, handler: &dyn ChatHandler) -> Result<Option<InternalEvent>, Exception> {
        let source = self.call_api().await?;

        let (tx, mut rx) = channel(64);
        tokio::spawn(async move {
            process_event_source(source, tx).await;
        });

        let mut assistant_message = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                InternalEvent::Event(event) => {
                    handler.on_event(&event);
                    if let ChatEvent::Delta(data) = event {
                        assistant_message.push_str(&data);
                    }
                }
                InternalEvent::FunctionCall(calls) => {
                    self.messages.push(ChatRequestMessage::new_function_call(&calls));
                    return Ok(Some(InternalEvent::FunctionCall(calls)));
                }
            }
        }

        if !assistant_message.is_empty() {
            self.messages.push(ChatRequestMessage::new_message(Role::Assistant, &assistant_message));
        }

        Ok(None)
    }

    async fn call_api(&mut self) -> Result<EventSource, Exception> {
        let has_function = !self.function_implementations.is_empty();

        let request = ChatRequest {
            messages: Cow::from(&self.messages),
            temperature: 0.8,
            top_p: 0.8,
            stream: true,
            stop: None,
            max_tokens: 800,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            tool_choice: has_function.then(|| "auto".to_string()),
            tools: has_function.then(|| Cow::from(&self.tools)),
        };
        let source = self.post_sse(&request).await?;
        Ok(source)
    }

    async fn post_sse<Request>(&self, request: &Request) -> Result<EventSource, Exception>
    where
        Request: Serialize + fmt::Debug,
    {
        let endpoint = &self.endpoint;
        let model = &self.model;
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-02-01");
        let body = json::to_json(&request)?;

        let request = http_client::http_client()
            .post(url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.api_key)
            .body(body);

        Ok(EventSource::new(request)?)
    }
}

impl From<CannotCloneRequestError> for Exception {
    fn from(err: CannotCloneRequestError) -> Self {
        Exception::new(&err.to_string())
    }
}

async fn process_event_source(mut source: EventSource, tx: Sender<InternalEvent>) {
    let mut function_calls: FunctionCall = HashMap::new();
    while let Some(event) = source.next().await {
        match event {
            Ok(Event::Open) => {}
            Ok(Event::Message(message)) => {
                let data = message.data;

                if data == "[DONE]" {
                    source.close();
                    break;
                }

                let response: ChatResponse = json::from_json(&data).unwrap();
                if response.choices.is_empty() {
                    continue;
                }

                let choice = response.choices.first().unwrap();
                let delta = choice.delta.as_ref().unwrap();

                if let Some(tool_calls) = delta.tool_calls.as_ref() {
                    let call = tool_calls.first().unwrap();
                    if let Some(name) = &call.function.name {
                        function_calls.insert(call.index, (call.id.as_ref().unwrap().to_string(), name.to_string(), String::new()));
                    }
                    function_calls.get_mut(&call.index).unwrap().2.push_str(&call.function.arguments)
                } else if let Some(value) = delta.content.as_ref() {
                    tx.send(InternalEvent::Event(ChatEvent::Delta(value.to_string()))).await.unwrap();
                }
            }
            Err(err) => {
                tx.send(InternalEvent::Event(ChatEvent::Error(err.to_string()))).await.unwrap();
                source.close();
            }
        }
    }
    if !function_calls.is_empty() {
        tx.send(InternalEvent::FunctionCall(function_calls)).await.unwrap();
    } else {
        tx.send(InternalEvent::Event(ChatEvent::End)).await.unwrap();
    }
}
