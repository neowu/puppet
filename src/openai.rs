use std::error::Error;
use std::fmt;

use crate::util::http_client;
use crate::util::json;
use reqwest_eventsource::EventSource;
use serde::Serialize;

pub mod api;
pub mod chatgpt;

pub struct Client {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

impl Client {
    pub async fn post_sse<Request>(&self, request: &Request) -> Result<EventSource, Box<dyn Error>>
    where
        Request: Serialize + fmt::Debug,
    {
        let endpoint = &self.endpoint;
        let model = &self.model;
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-02-15-preview");
        let body = json::to_json(&request)?;
        // dbg!(&body);
        let request = http_client::http_client()
            .post(url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.api_key)
            .body(body);

        Ok(EventSource::new(request).unwrap())
    }
}
