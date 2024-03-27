use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use futures::stream::StreamExt;
use reqwest_eventsource::Event;
use reqwest_eventsource::EventSource;
use tokio::sync::mpsc::channel;

use crate::bot::handler::ChatEvent;
use crate::bot::handler::ChatHandler;
use crate::openai::api::ChatRequest;
use crate::openai::api::ChatRequestMessage;
use crate::openai::api::ChatResponse;
use crate::openai::api::Function;
use crate::openai::api::Role;
use crate::openai::api::Tool;
use crate::openai::Client;
use crate::util::json;

pub struct ChatGPT {
    pub client: Client,
    pub messages: Vec<ChatRequestMessage>,
    tools: Vec<Tool>,
    function_implementations: HashMap<String, Arc<Box<FunctionImplementation>>>,
}

type FunctionImplementation = dyn Fn(String) -> String + Send + Sync;

enum InternalEvent {
    Event(ChatEvent),
    FunctionCall { name: String, arguments: String },
}

impl ChatGPT {
    pub fn new(client: Client, system_message: Option<String>) -> Self {
        let mut chatgpt = ChatGPT {
            client,
            messages: vec![],
            tools: vec![],
            function_implementations: HashMap::new(),
        };
        if let Some(message) = system_message {
            chatgpt.messages.push(ChatRequestMessage::new(Role::System, &message));
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

    pub async fn chat(&mut self, message: &str, handler: &dyn ChatHandler) -> Result<(), Box<dyn Error>> {
        let result = self.process(ChatRequestMessage::new(Role::User, message), handler).await;
        if let Ok(Some(InternalEvent::FunctionCall { name, arguments })) = result {
            let function = Arc::clone(self.function_implementations.get(&name).unwrap());

            let result = tokio::spawn(async move { function(arguments) }).await?;

            self.process(ChatRequestMessage::new_function(name, result), handler).await?;
        }
        Ok(())
    }

    async fn process(&mut self, message: ChatRequestMessage, handler: &dyn ChatHandler) -> Result<Option<InternalEvent>, Box<dyn Error>> {
        let mut source = self.call_api(message).await?;

        let (tx, mut rx) = channel(64);
        tokio::spawn(async move {
            let mut function_name: Option<String> = None;
            let mut function_arguments = String::new();
            while let Some(event) = source.next().await {
                match event {
                    Ok(Event::Open) => {}
                    Ok(Event::Message(message)) => {
                        let data = message.data;

                        if data == "[DONE]" {
                            source.close();
                            if function_name.is_none() {
                                tx.send(InternalEvent::Event(ChatEvent::End)).await.unwrap();
                            }
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
                                function_name = Some(name.to_string());
                            }
                            function_arguments.push_str(&call.function.arguments);
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
            if let Some(function_name) = function_name {
                tx.send(InternalEvent::FunctionCall {
                    name: function_name,
                    arguments: function_arguments,
                })
                .await
                .unwrap();
            }
        });

        let mut assistant_message = String::new();
        let mut function_name: Option<String> = None;
        let mut function_arguments = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                InternalEvent::Event(event) => {
                    handler.on_event(&event);
                    if let ChatEvent::Delta(data) = event {
                        assistant_message.push_str(&data);
                    }
                }
                InternalEvent::FunctionCall { name, arguments } => {
                    function_name = Some(name);
                    function_arguments.push_str(&arguments);
                }
            }
        }

        if !assistant_message.is_empty() {
            self.messages.push(ChatRequestMessage::new(Role::Assistant, &assistant_message));
        }

        if let Some(function_name) = function_name {
            return Ok(Some(InternalEvent::FunctionCall {
                name: function_name,
                arguments: function_arguments,
            }));
        }

        Ok(None)
    }

    async fn call_api(&mut self, message: ChatRequestMessage) -> Result<EventSource, Box<dyn Error>> {
        let has_function = !self.function_implementations.is_empty();
        self.messages.push(message);
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
        let source = self.client.post_sse(&request).await?;
        Ok(source)
    }
}
