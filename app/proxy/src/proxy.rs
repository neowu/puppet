use anyhow::Result;
use axum::Router;
use axum::body::Bytes;
use axum::debug_handler;
use axum::extract::Path;
use axum::extract::State;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::post;
use framework::http_client::HTTP_CLIENT;
use framework::http_client::ResponseExt;
use framework::json::from_json;
use framework::task;
use framework::web::error::HttpResult;
use futures::Stream;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::trace;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(openai))
        .route("/v1/chat/completions", post(deepseek))
        .route("/v1beta/models/{model}", post(vertexai))
}

#[debug_handler]
async fn openai(State(config): State<AppState>, body: Bytes) -> HttpResult<Sse<impl Stream<Item = Result<Event>>>> {
    let url = config.config.proxy["openai"].url("gpt-4o");
    let api_key = config.config.proxy["openai"].api_key()?;
    proxy(url, body, api_key).await
}

#[debug_handler]
async fn deepseek(State(config): State<AppState>, body: Bytes) -> HttpResult<Sse<impl Stream<Item = Result<Event>>>> {
    let url = config.config.proxy["deepseek"].url("DeepSeek-R1");
    let api_key = config.config.proxy["deepseek"].api_key()?;
    proxy(url, body, api_key).await
}

#[debug_handler]
async fn vertexai(
    State(config): State<AppState>,
    Path(model): Path<String>,
    body: Bytes,
) -> HttpResult<Sse<impl Stream<Item = Result<Event>>>> {
    let model = if model.contains("flash") {
        "gemini-2.0-flash-001"
    } else {
        "gemini-2.0-pro-exp-02-05"
    };

    let url = config.config.proxy["vertexai"].url(model);
    let api_key = config.config.proxy["vertexai"].api_key()?;
    proxy(url, body, api_key).await
}

async fn proxy(url: String, body: Bytes, api_key: String) -> HttpResult<Sse<impl Stream<Item = Result<Event>>>> {
    let (tx, rx) = mpsc::channel(64);
    task::spawn(async move {
        let http_request = HTTP_CLIENT
            .post(url)
            .header("Content-Type", "application/json")
            .header("api-key", &api_key)
            .bearer_auth(&api_key)
            .body(body);
        let response = http_request.send().await?;

        let mut lines = response.lines();
        while let Some(line) = lines.next().await {
            if let Some(data) = line?.strip_prefix("data: ") {
                trace!("data={data}");
                if data != "[DONE]" {
                    let chunk: Value = from_json(data)?;
                    if chunk.get("usage").is_some() && chunk.get("object").is_none() {
                        continue;
                    }
                }
                tx.send(Ok(Event::default().data(data))).await?;
            }
        }
        Ok(())
    });

    let stream = ReceiverStream::new(rx);
    Ok(Sse::new(stream))
}
