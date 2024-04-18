use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use futures::stream::StreamExt;
use reqwest_eventsource::CannotCloneRequestError;
use reqwest_eventsource::Event;
use reqwest_eventsource::EventSource;
use serde::Serialize;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Sender;

use crate::bot::function::FunctionStore;
use crate::bot::ChatEvent;
use crate::bot::ChatHandler;
use crate::bot::Usage;
use crate::openai::api::ChatRequest;
use crate::openai::api::ChatRequestMessage;
use crate::openai::api::ChatResponse;
use crate::openai::api::Role;
use crate::openai::api::Tool;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct ChatGPT {
    url: String,
    api_key: String,
    messages: Rc<Vec<ChatRequestMessage>>,
    tools: Rc<Vec<Tool>>,
    function_store: FunctionStore,
}

enum InternalEvent {
    Event(ChatEvent),
    FunctionCall(FunctionCall),
}

type FunctionCall = HashMap<i64, (String, String, String)>;

impl ChatGPT {
    pub fn new(endpoint: String, model: String, api_key: String, system_message: Option<String>, function_store: FunctionStore) -> Self {
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-02-01");
        let mut chatgpt = ChatGPT {
            url,
            api_key,
            messages: Rc::new(vec![]),
            tools: Rc::new(
                function_store
                    .declarations
                    .iter()
                    .map(|f| Tool {
                        r#type: "function".to_string(),
                        function: f.clone(),
                    })
                    .collect(),
            ),
            function_store,
        };
        if let Some(message) = system_message {
            chatgpt.add_message(ChatRequestMessage::new_message(Role::System, message));
        }
        chatgpt
    }

    pub async fn chat(&mut self, message: String, handler: &dyn ChatHandler) -> Result<(), Exception> {
        self.add_message(ChatRequestMessage::new_message(Role::User, message));
        let result = self.process(handler).await;
        if let Ok(Some(InternalEvent::FunctionCall(calls))) = result {
            let functions = calls
                .into_iter()
                .map(|(_, (id, name, args))| (id, name, json::from_json(&args).unwrap()))
                .collect();
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
                    if let ChatEvent::Delta(ref data) = event {
                        assistant_message.push_str(data);
                    }
                    handler.on_event(event);
                }
                InternalEvent::FunctionCall(calls) => {
                    self.add_message(ChatRequestMessage::new_function_call(&calls));
                    return Ok(Some(InternalEvent::FunctionCall(calls)));
                }
            }
        }

        if !assistant_message.is_empty() {
            self.add_message(ChatRequestMessage::new_message(Role::Assistant, assistant_message));
        }

        Ok(None)
    }

    async fn call_api(&mut self) -> Result<EventSource, Exception> {
        let has_function = !self.tools.is_empty();

        let request = ChatRequest {
            messages: Rc::clone(&self.messages),
            temperature: 0.8,
            top_p: 0.8,
            stream: true,
            stop: None,
            max_tokens: 800,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            tool_choice: has_function.then(|| "auto".to_string()),
            tools: has_function.then(|| Rc::clone(&self.tools)),
        };
        let source = self.post_sse(&request).await?;
        Ok(source)
    }

    async fn post_sse<Request>(&self, request: &Request) -> Result<EventSource, Exception>
    where
        Request: Serialize + fmt::Debug,
    {
        let body = json::to_json(&request)?;

        let request = http_client::http_client()
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.api_key)
            .body(body);

        Ok(EventSource::new(request)?)
    }
}

impl From<CannotCloneRequestError> for Exception {
    fn from(err: CannotCloneRequestError) -> Self {
        Exception::new(err.to_string())
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
        tx.send(InternalEvent::Event(ChatEvent::End(Usage::default()))).await.unwrap();
    }
}
