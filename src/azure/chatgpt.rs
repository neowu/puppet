use std::collections::HashMap;
use std::ops::Not;
use std::rc::Rc;

use futures::stream::StreamExt;
use reqwest_eventsource::CannotCloneRequestError;
use reqwest_eventsource::Event;
use reqwest_eventsource::EventSource;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

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

enum InternalEvent {
    Event(ChatEvent),
    FunctionCall(FunctionCall),
}

type FunctionCall = HashMap<i64, (String, String, String)>;

impl ChatGPT {
    pub fn new(endpoint: String, model: String, api_key: String, system_message: Option<String>, function_store: FunctionStore) -> Self {
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-02-01");
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
        let source = self.call_api().await?;

        let (tx, rx) = channel(64);
        let handle = tokio::spawn(read_event_source(source, tx));

        let function_call = self.process_event(rx, handler).await;
        handle.await??;

        Ok(function_call)
    }

    async fn process_event(&mut self, mut rx: Receiver<InternalEvent>, handler: &impl ChatHandler) -> Option<FunctionCall> {
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
                    return Some(calls);
                }
            }
        }
        if !assistant_message.is_empty() {
            self.add_message(ChatRequestMessage::new_message(Role::Assistant, assistant_message));
        }
        None
    }

    async fn call_api(&mut self) -> Result<EventSource, Exception> {
        let request = ChatRequest {
            messages: Rc::clone(&self.messages),
            temperature: 0.7,
            top_p: 0.95,
            stream: true,
            stop: None,
            max_tokens: 800,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            tool_choice: self.tools.is_some().then_some("auto".to_string()),
            tools: self.tools.clone(),
        };

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
        Exception::unexpected(err)
    }
}

async fn read_event_source(mut source: EventSource, tx: Sender<InternalEvent>) -> Result<(), Exception> {
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

                let response: ChatResponse = json::from_json(&data)?;

                if let Some(choice) = response.choices.into_iter().next() {
                    let delta = choice.delta.unwrap();

                    if let Some(tool_calls) = delta.tool_calls {
                        let call = tool_calls.into_iter().next().unwrap();
                        if let Some(name) = call.function.name {
                            function_calls.insert(call.index, (call.id.unwrap(), name, String::new()));
                        }
                        function_calls.get_mut(&call.index).unwrap().2.push_str(&call.function.arguments)
                    } else if let Some(value) = delta.content {
                        tx.send(InternalEvent::Event(ChatEvent::Delta(value))).await?;
                    }
                }
            }
            Err(err) => {
                source.close();
                return Err(Exception::unexpected(err));
            }
        }
    }
    if !function_calls.is_empty() {
        tx.send(InternalEvent::FunctionCall(function_calls)).await?;
    } else {
        // chatgpt doesn't support token usage with stream mode
        tx.send(InternalEvent::Event(ChatEvent::End(Usage::default()))).await?;
    }

    Ok(())
}
