use std::error::Error;
use std::sync::OnceLock;

use reqwest_eventsource::EventSource;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::util::exception::Exception;

pub mod chat_completion;

fn http_client() -> &'static reqwest::Client {
    static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP_CLIENT.get_or_init(reqwest::Client::new)
}

pub struct Client {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

impl Client {
    #[allow(dead_code)]
    async fn post<Request, Response>(&self, url: &str, request: &Request) -> Result<Response, Box<dyn Error>>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let body = serde_json::to_string(request)?;
        let response = http_client()
            .post(url)
            .header("api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        if status != 200 {
            return Err(Box::new(Exception::new(&format!(
                "failed to call openai api, status={}, response={}",
                status, text
            ))));
        }

        Ok(serde_json::from_str(&text)?)
    }

    pub async fn post_sse<Request>(&self, request: &Request) -> Result<EventSource, Box<dyn Error>>
    where
        Request: Serialize,
    {
        let endpoint = &self.endpoint;
        let model = &self.model;
        let url = format!("{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-02-15-preview");
        let body = serde_json::to_string(request)?;

        let request = http_client()
            .post(url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.api_key)
            .body(body);

        Ok(EventSource::new(request).unwrap())
    }
}
