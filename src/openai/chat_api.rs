use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::llm::function::Function;

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Arc<Vec<ChatRequestMessage>>,
    pub temperature: f32,
    pub top_p: f32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    pub max_tokens: i32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Arc<[Tool]>>,
}

#[derive(Debug, Serialize)]
pub struct ChatRequestMessage {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<Content>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize)]
pub struct Content {
    pub r#type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

#[derive(Debug, Serialize)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

impl ChatRequestMessage {
    pub fn new_message(role: Role, message: String) -> Self {
        ChatRequestMessage {
            role,
            content: Some(vec![Content {
                r#type: "text",
                text: Some(message),
                image_url: None,
            }]),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn new_user_message(message: String, image_urls: Vec<String>) -> Self {
        let mut content = vec![];
        content.push(Content {
            r#type: "text",
            text: Some(message),
            image_url: None,
        });
        for url in image_urls {
            content.push(Content {
                r#type: "image_url",
                text: None,
                image_url: Some(ImageUrl { url }),
            });
        }
        ChatRequestMessage {
            role: Role::User,
            content: Some(content),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn new_function_response(id: String, result: String) -> Self {
        ChatRequestMessage {
            role: Role::Tool,
            content: Some(vec![Content {
                r#type: "text",
                text: Some(result),
                image_url: None,
            }]),
            tool_call_id: Some(id),
            tool_calls: None,
        }
    }

    pub fn new_function_call(calls: Vec<ToolCall>) -> ChatRequestMessage {
        ChatRequestMessage {
            role: Role::Assistant,
            content: None,
            tool_call_id: None,
            tool_calls: Some(calls),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub r#type: &'static str,
    pub function: Function,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "tool")]
    Tool,
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamResponse {
    pub choices: Vec<ChatStreamCompletionChoice>,
    pub usage: Option<Usage>, // not supported by azure openai api yet
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamCompletionChoice {
    pub index: i64,
    pub delta: ChatStreamResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamResponseMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct StreamToolCall {
    pub index: i64,
    pub id: Option<String>,
    pub function: StreamFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamFunctionCall {
    pub name: Option<String>,
    pub arguments: String,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Usage,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: i64,
    pub message: ChatResponseMessage,
    pub finish_reason: String,
}

impl ChatCompletionChoice {
    pub fn append_content(&mut self, delta: &str) {
        if let Some(content) = self.message.content.as_mut() {
            content.push_str(delta);
        } else {
            self.message.content = Some(delta.to_string());
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatResponseMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
