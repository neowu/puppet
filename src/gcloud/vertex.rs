use futures::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Sender;

use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::Arc;

use crate::bot::Bot;
use crate::bot::ChatEvent;
use crate::bot::ChatHandler;
use crate::bot::Function;
use crate::bot::FunctionImplementation;
use crate::gcloud::api::GenerateContentResponse;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

use super::api::Content;
use super::api::FunctionCall;
use super::api::GenerationConfig;
use super::api::Role;
use super::api::StreamGenerateContent;
use super::api::Tool;

pub struct Vertex {
    pub endpoint: String,
    pub project: String,
    pub location: String,
    pub model: String,
    messages: Vec<Content>,
    tools: Vec<Tool>,
    function_implementations: HashMap<String, Arc<Box<FunctionImplementation>>>,
}

impl Bot for Vertex {
    fn register_function(&mut self, function: Function, implementation: Box<FunctionImplementation>) {
        let name = function.name.to_string();
        self.tools.push(Tool {
            function_declarations: vec![function],
        });
        self.function_implementations.insert(name, Arc::new(implementation));
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Vertex {
    pub fn new(endpoint: String, project: String, location: String, model: String) -> Self {
        Vertex {
            endpoint,
            project,
            location,
            model,
            messages: vec![],
            tools: vec![],
            function_implementations: HashMap::new(),
        }
    }

    pub async fn chat(&mut self, message: &str, handler: &dyn ChatHandler) -> Result<(), Box<dyn Error>> {
        let mut result = self.process(Content::new_text(Role::User, message), handler).await?;

        while let Some(function_call) = result {
            let function = Arc::clone(
                self.function_implementations
                    .get(&function_call.name)
                    .ok_or_else(|| Exception::new(&format!("function not found, name={}", function_call.name)))?,
            );

            let function_response = tokio::spawn(async move { function(function_call.args) }).await?;

            let content = Content::new_function_response(&function_call.name, function_response);
            result = self.process(content, handler).await?;
        }
        Ok(())
    }

    async fn process(&mut self, content: Content, handler: &dyn ChatHandler) -> Result<Option<FunctionCall>, Box<dyn Error>> {
        self.messages.push(content);

        let response = self.call_api().await?;

        let (tx, mut rx) = channel(64);

        tokio::spawn(async move {
            process_response_stream(response, tx).await;
        });

        let mut model_message = String::new();
        while let Some(response) = rx.recv().await {
            match response {
                Ok(response) => {
                    let part = response.candidates.first().unwrap().content.parts.first().unwrap();

                    if let Some(function) = part.function_call.as_ref() {
                        self.messages.push(Content::new_function_call(function.clone()));
                        return Ok(Some(function.clone()));
                    } else if let Some(text) = part.text.as_ref() {
                        handler.on_event(&ChatEvent::Delta(text.to_string()));
                        model_message.push_str(text);
                    }
                }
                Err(err) => {
                    return Err(Box::new(err));
                }
            }
        }
        if !model_message.is_empty() {
            self.messages.push(Content::new_text(Role::Model, &model_message));
        }
        handler.on_event(&ChatEvent::End);
        Ok(None)
    }

    async fn call_api(&mut self) -> Result<Response, Box<dyn Error>> {
        let has_function = !self.function_implementations.is_empty();

        let endpoint = &self.endpoint;
        let project = &self.project;
        let location = &self.location;
        let model = &self.model;
        let url = format!("{endpoint}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent");

        let request = StreamGenerateContent {
            contents: Cow::from(&self.messages),
            generation_config: GenerationConfig {
                temperature: 1.0,
                top_p: 0.95,
                max_output_tokens: 2048,
            },
            tools: has_function.then(|| Cow::from(&self.tools)),
        };
        let response = self.post(&url, &request).await?;
        Ok(response)
    }

    async fn post(&self, url: &str, request: &StreamGenerateContent<'_>) -> Result<Response, Box<dyn Error>> {
        let body = json::to_json(request)?;
        let response = http_client::http_client()
            .post(url)
            .bearer_auth(token())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        if status != 200 {
            return Err(Box::new(Exception::new(&format!(
                "failed to call gcloud api, status={}, response={}",
                status,
                response.text().await?
            ))));
        }
        Ok(response)
    }
}

async fn process_response_stream(response: Response, tx: Sender<Result<GenerateContentResponse, Exception>>) {
    let stream = &mut response.bytes_stream();

    let mut buffer = String::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                buffer.push_str(std::str::from_utf8(&chunk).unwrap());

                // first char is '[' or ','
                if !is_valid_json(&buffer[1..]) {
                    continue;
                }

                let content: GenerateContentResponse = json::from_json(&buffer[1..]).unwrap();
                tx.send(Ok(content)).await.unwrap();
                buffer.clear();
            }
            Err(err) => {
                // tx.send(InternalEvent::Event(ChatEvent::Error(err.to_string()))).await.unwrap();
                tx.send(Err(Exception::new(&err.to_string()))).await.unwrap();
                break;
            }
        }
    }
}

fn is_valid_json(content: &str) -> bool {
    let result: serde_json::Result<serde::de::IgnoredAny> = serde_json::from_str(content);
    result.is_ok()
}

fn token() -> String {
    env::var("GCLOUD_AUTH_TOKEN").expect("please set GCLOUD_AUTH_TOKEN env")
}
