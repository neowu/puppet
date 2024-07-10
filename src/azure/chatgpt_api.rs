use std::collections::HashMap;
use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;

use crate::llm::function::Function;

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub messages: Rc<Vec<ChatRequestMessage>>,
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
    pub tools: Option<Rc<[Tool]>>,
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
    pub r#type: String,
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
                r#type: "text".to_string(),
                text: Some(message),
                image_url: None,
            }]),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn new_function_response(id: String, result: String) -> Self {
        ChatRequestMessage {
            role: Role::Tool,
            content: Some(vec![Content {
                r#type: "text".to_string(),
                text: Some(result),
                image_url: None,
            }]),
            tool_call_id: Some(id),
            tool_calls: None,
        }
    }

    pub fn new_function_call(calls: &HashMap<i64, (String, String, String)>) -> ChatRequestMessage {
        ChatRequestMessage {
            role: Role::Assistant,
            content: None,
            tool_call_id: None,
            tool_calls: Some(
                calls
                    .iter()
                    .map(|(key, (id, name, arguments))| ToolCall {
                        index: *key,
                        id: Some(id.to_string()),
                        function: FunctionCall {
                            name: Some(name.to_string()),
                            arguments: arguments.to_string(),
                        },
                        r#type: Some("function".to_string()),
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub r#type: String,
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Option<Usage>, // not supported by azure openai api yet
}

#[allow(dead_code)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub index: i64,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: Option<String>,
    pub arguments: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub completion_tokens: i32,
    pub prompt_tokens: i32,
    pub total_tokens: i32,
}
