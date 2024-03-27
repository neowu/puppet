use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ChatRequest<'a> {
    #[serde(borrow)]
    pub messages: Cow<'a, [ChatRequestMessage]>,
    pub temperature: f32,
    pub top_p: f32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    pub max_tokens: i32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Cow<'a, [Tool]>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatRequestMessage {
    pub role: Role,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatRequestMessage {
    pub fn new(role: Role, message: &str) -> Self {
        ChatRequestMessage {
            role,
            content: Some(message.to_string()),
            name: None,
        }
    }

    pub fn new_function(name: String, result: String) -> Self {
        ChatRequestMessage {
            role: Role::Function,
            content: Some(result),
            name: Some(name),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Tool {
    pub r#type: String,
    pub function: Function,
}

#[derive(Debug, Serialize, Clone)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub id: Option<String>,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize)]
pub struct FunctionCall {
    pub name: Option<String>,
    pub arguments: String,
}
