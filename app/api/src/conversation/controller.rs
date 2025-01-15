use std::time::Duration;

use anyhow::Result;
use axum::debug_handler;
use axum::extract::Path;
use axum::extract::State;
use axum::response::sse::Event;
use axum::response::sse::KeepAlive;
use axum::response::Sse;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use chrono::DateTime;
use chrono::Utc;
use framework::task;
use futures::Stream;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::repository;
use super::repository::Conversation;
use super::repository::Message;
use crate::web::ApiError;
use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/conversation", get(list_conversations))
        .route("/conversation", post(start_conversation))
        .route("/conversation/{id}", get(get_conversation))
        .route("/conversation/{id}/chat", post(chat))
}

#[derive(Serialize, Debug)]
struct ConversationView {
    id: u32,
    summary: String,
    created_time: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
struct ConversationDetailView {
    id: u32,
    summary: String,
    messages: Vec<Message>,
    created_time: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
struct ChatRequest {
    message: String,
}

#[debug_handler]
async fn list_conversations(State(ApiState { db }): State<ApiState>) -> Result<Json<Vec<ConversationView>>, ApiError> {
    let conversations = repository::list_conversations(db)?;
    Ok(Json(conversations.into_iter().map(conversation_view).collect()))
}

#[debug_handler]
async fn start_conversation(State(ApiState { db }): State<ApiState>) -> Result<Json<ConversationView>, ApiError> {
    let conversation = repository::create_conversation(db)?;
    Ok(Json(conversation_view(conversation)))
}

fn conversation_view(conversation: Conversation) -> ConversationView {
    ConversationView {
        id: conversation.id,
        summary: conversation.summary,
        created_time: conversation.created_time,
    }
}

#[debug_handler]
async fn get_conversation(Path(id): Path<u32>, State(ApiState { db }): State<ApiState>) -> Result<Json<ConversationDetailView>, ApiError> {
    let conversation = repository::get_conversation(db, id)?;
    let json = Json(ConversationDetailView {
        id: conversation.id,
        summary: conversation.summary,
        messages: conversation.messages,
        created_time: conversation.created_time,
    });
    Ok(json)
}

#[debug_handler]
async fn chat(
    Path(id): Path<u32>,
    State(ApiState { db }): State<ApiState>,
    Json(request): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event>>> {
    let (tx, rx) = mpsc::channel(64);

    task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            tx.send(Ok(Event::default().data("hello"))).await.unwrap();
        }
    });

    let stream = ReceiverStream::new(rx);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
}
