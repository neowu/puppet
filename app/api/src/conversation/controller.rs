use axum::debug_handler;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use chrono::DateTime;
use chrono::Utc;
use serde::Serialize;
use tracing::trace;

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
        .route("/test", get(test))
}

#[derive(Serialize, Debug)]
struct ConversationView {
    id: u32,
    summary: String,
    created_time: DateTime<Utc>,
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

#[derive(Serialize, Debug)]
enum RoleView {
    Assistant,
    User,
}

#[derive(Serialize, Debug)]
struct ConversationDetailView {
    id: u32,
    summary: String,
    messages: Vec<(RoleView, String)>,
    created_time: DateTime<Utc>,
}

#[debug_handler]
async fn get_conversation(Path(id): Path<u32>, State(ApiState { db }): State<ApiState>) -> Result<Json<ConversationDetailView>, ApiError> {
    let conversation = repository::get_conversation(db, id)?;
    trace!("conversation={:?}", conversation);
    let json = Json(ConversationDetailView {
        id: conversation.id,
        summary: conversation.summary,
        messages: conversation
            .messages
            .into_iter()
            .map(|Message { role, message }| match role.as_str() {
                "assistant" => (RoleView::Assistant, message),
                _ => (RoleView::User, message),
            })
            .collect(),
        created_time: conversation.created_time,
    });
    Ok(json)
}

#[debug_handler]
async fn test(State(ApiState { db }): State<ApiState>) -> Result<(), ApiError> {
    let conversation = repository::Conversation {
        id: 10000,
        summary: "test conv".to_string(),
        messages: vec![
            Message {
                role: "user".to_string(),
                message: "hello".to_string(),
            },
            Message {
                role: "assistant".to_string(),
                message: "how can i help you".to_string(),
            },
        ],
        created_time: Utc::now(),
    };
    repository::save_conversation(db, conversation)?;
    Ok(())
}
