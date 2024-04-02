use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use crate::bot::Function;

#[derive(Debug, Serialize)]
pub struct StreamGenerateContent<'a> {
    #[serde(borrow)]
    pub contents: Cow<'a, [Content]>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GenerationConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Cow<'a, [Tool]>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Content {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Content {
    pub fn new_text(role: Role, message: &str) -> Self {
        Self {
            role,
            parts: vec![Part {
                text: Some(message.to_string()),
                function_call: None,
                function_response: None,
            }],
        }
    }

    pub fn new_function_response(name: &str, response: serde_json::Value) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part {
                text: None,
                function_call: None,
                function_response: Some(FunctionResponse {
                    name: name.to_string(),
                    response,
                }),
            }],
        }
    }

    pub fn new_function_call(function_call: FunctionCall) -> Self {
        Self {
            role: Role::Model,
            parts: vec![Part {
                text: None,
                function_call: Some(function_call),
                function_response: None,
            }],
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Tool {
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<Function>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "model")]
    Model,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}
