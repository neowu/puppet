use futures::StreamExt;
use reqwest::Response;
use serde::Serialize;
use tokio::sync::mpsc::channel;

use std::borrow::Cow;
use std::env;
use std::error::Error;

use crate::bot::handler::ChatEvent;
use crate::bot::handler::ChatHandler;
use crate::gcloud::api::GenerateContentResponse;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

use super::api::Content;
use super::api::GenerationConfig;
use super::api::Role;
use super::api::StreamGenerateContent;

pub struct Vertex {
    pub endpoint: String,
    pub project: String,
    pub location: String,
    pub model: String,
    messages: Vec<Content>,
}

enum InternalEvent {
    Event(ChatEvent),
    FunctionCall { name: String, arguments: String },
}

impl Vertex {
    pub fn new(endpoint: String, project: String, location: String, model: String) -> Self {
        Vertex {
            endpoint,
            project,
            location,
            model,
            messages: vec![],
        }
    }

    pub async fn chat(&mut self, message: &str, handler: &dyn ChatHandler) -> Result<(), Box<dyn Error>> {
        let response = self.call_api(message).await?;

        let (tx, mut rx) = channel(64);

        tokio::spawn(async move {
            let stream = &mut response.bytes_stream();

            let mut buffer = String::new();
            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        buffer.push_str(std::str::from_utf8(&chunk).unwrap());

                        if !is_valid_json(&buffer[1..]) {
                            continue;
                        }

                        let content: GenerateContentResponse = json::from_json(&buffer[1..]).unwrap();
                        let delta = content.candidates.first().unwrap().content.parts.first().unwrap().text.as_ref().unwrap();
                        tx.send(InternalEvent::Event(ChatEvent::Delta(delta.to_string()))).await.unwrap();
                        buffer.clear();
                    }
                    Err(err) => {
                        tx.send(InternalEvent::Event(ChatEvent::Error(err.to_string()))).await.unwrap();
                        break;
                    }
                }
            }
        });

        let mut model_message = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                InternalEvent::Event(event) => {
                    handler.on_event(&event);
                    if let ChatEvent::Delta(data) = event {
                        model_message.push_str(&data);
                    }
                }
                InternalEvent::FunctionCall { name: _, arguments: _ } => {}
            }
        }
        if !model_message.is_empty() {
            self.messages.push(Content::new(Role::Model, &model_message));
        }
        handler.on_event(&ChatEvent::End);
        Ok(())
    }

    async fn call_api(&mut self, message: &str) -> Result<Response, Box<dyn Error>> {
        let endpoint = &self.endpoint;
        let project = &self.project;
        let location = &self.location;
        let model = &self.model;
        let url = format!("{endpoint}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent");
        self.messages.push(Content::new(Role::User, message));
        let request = StreamGenerateContent {
            contents: Cow::from(&self.messages),
            generation_config: GenerationConfig {
                temperature: 0.8,
                top_p: 1.0,
                max_output_tokens: 800,
            },
        };
        let response = self.post(&url, &request).await?;
        Ok(response)
    }

    async fn post<Request>(&self, url: &str, request: &Request) -> Result<Response, Box<dyn Error>>
    where
        Request: Serialize,
    {
        let body = serde_json::to_string(request)?;
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

fn is_valid_json(content: &str) -> bool {
    let result: serde_json::Result<serde::de::IgnoredAny> = serde_json::from_str(content);
    result.is_ok()
}

fn token() -> String {
    env::var("GCLOUD_AUTH_TOKEN").expect("please set GCLOUD_AUTH_TOKEN env")
}
