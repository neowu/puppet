use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;

use crate::bot::Function;

#[derive(Debug, Serialize)]
pub struct StreamGenerateContent {
    pub contents: Rc<Vec<Content>>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Rc<Option<Content>>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GenerationConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Rc<Vec<Tool>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Content {
    pub fn new_text(role: Role, message: String) -> Self {
        Self {
            role,
            parts: vec![Part {
                text: Some(message),
                inline_data: None,
                function_call: None,
                function_response: None,
            }],
        }
    }

    pub fn new_function_response(name: String, response: serde_json::Value) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part {
                text: None,
                inline_data: None,
                function_call: None,
                function_response: Some(FunctionResponse { name, response }),
            }],
        }
    }

    pub fn new_function_call(function_call: FunctionCall) -> Self {
        Self {
            role: Role::Model,
            parts: vec![Part {
                text: None,
                inline_data: None,
                function_call: Some(function_call),
                function_response: None,
            }],
        }
    }

    pub fn new_inline_data(mime_type: String, data: String, message: String) -> Self {
        Self {
            role: Role::User,
            parts: vec![
                Part {
                    text: None,
                    inline_data: Some(InlineData { mime_type, data }),
                    function_call: None,
                    function_response: None,
                },
                Part {
                    text: Some(message),
                    inline_data: None,
                    function_call: None,
                    function_response: None,
                },
            ],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Tool {
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<Function>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "model")]
    Model,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,

    #[serde(rename = "functionCall")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    #[serde(rename = "functionResponse")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
}

#[derive(Debug, Serialize)]
pub struct GenerationConfig {
    pub temperature: f32,
    #[serde(rename = "topP")]
    pub top_p: f32,
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InlineData {
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateContentResponse {
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
pub struct Candidate {
    pub content: Content,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}
