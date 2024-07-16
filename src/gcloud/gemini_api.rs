use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;

use crate::llm::function::Function;

#[derive(Debug, Serialize)]
pub struct StreamGenerateContent {
    pub contents: Rc<Vec<Content>>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Rc<Content>>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GenerationConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Rc<[Tool]>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Content {
    pub fn new_user_text(text: String, datas: Vec<InlineData>) -> Self {
        let mut parts: Vec<Part> = vec![];
        for data in datas {
            parts.push(Part {
                text: None,
                inline_data: Some(data),
                function_call: None,
                function_response: None,
            });
        }
        parts.push(Part {
            text: Some(text),
            inline_data: None,
            function_call: None,
            function_response: None,
        });
        Self { role: Role::User, parts }
    }

    pub fn new_model_text(text: String) -> Self {
        Self {
            role: Role::Model,
            parts: vec![Part {
                text: Some(text),
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
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct Candidate {
    pub content: Option<Content>,
    #[serde(rename = "finishReason")]
    pub finish_reason: Option<String>,
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

#[derive(Debug, Deserialize)]
pub struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: i32,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: i32,
}
