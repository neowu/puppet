use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatRequestMessage>,
    pub temperature: f32,
    pub top_p: f32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    pub max_tokens: i32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,
}

impl ChatRequest {
    pub fn new() -> ChatRequest {
        ChatRequest {
            messages: vec![],
            temperature: 0.8,
            top_p: 0.8,
            stream: true,
            stop: None,
            max_tokens: 800,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChatRequestMessage {
    pub role: Role,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatRequestMessage {
    pub fn new(role: Role, message: &str) -> ChatRequestMessage {
        ChatRequestMessage {
            role,
            content: Some(message.to_string()),
            name: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "function")]
    Function,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: i64,
    pub message: Option<ChatResponseMessage>,
    pub delta: Option<ChatResponseMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponseMessage {
    pub content: Option<String>,
}
