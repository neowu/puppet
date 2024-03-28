use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StreamGenerateContent<'a> {
    #[serde(borrow)]
    pub contents: Cow<'a, [Content]>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GenerationConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Content {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Content {
    pub fn new(role: Role, message: &str) -> Self {
        Self {
            role,
            parts: vec![Part {
                text: Some(message.to_string()),
            }],
        }
    }
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
