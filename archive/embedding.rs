use core::str;

use bytes::Bytes;
use framework::exception::Exception;
use framework::json;
use serde::Deserialize;
use serde::Serialize;
use tracing::info;

use crate::api_key;

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
    encoding_format: &'static str,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    #[allow(dead_code)]
    object: String,
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    index: i32,
    embedding: Vec<f32>,
}

pub struct Embedding {
    url: String,
    model: String,
    api_key: String,
}

impl Embedding {
    pub fn new(url: String, model: String, api_key: String) -> Self {
        Self { url, model, api_key }
    }

    pub async fn encode(&self, input: String) -> Result<Vec<f32>, Exception> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            input,
            encoding_format: "float",
        };

        let body = Bytes::from(json::to_json(&request)?);
        let api_key = api_key(&self.api_key)?;
        let http_request = HTTP_CLIENT
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("api-key", api_key.clone()) // azure api use header
            .bearer_auth(api_key.clone())
            .body(body.clone());
        let http_response = http_request.send().await?;

        let status = http_response.status();
        if status != 200 {
            let body = str::from_utf8(&body)?;
            info!("body={}", body);
            let response_text = http_response.text().await?;
            return Err(anyhow!("failed to call api, status={status}, response={response_text}"));
        }

        let response: EmbeddingResponse = json::from_json(&http_response.text().await?)?;
        Ok(response.data.into_iter().next().unwrap().embedding)
    }
}
