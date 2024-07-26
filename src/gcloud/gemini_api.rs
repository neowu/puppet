use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;

use crate::llm::function::Function;
use crate::llm::function::FunctionPayload;

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

    pub fn new_function_response(results: Vec<FunctionPayload>) -> Self {
        Self {
            role: Role::User,
            parts: results
                .into_iter()
                .map(|result| Part {
                    text: None,
                    inline_data: None,
                    function_call: None,
                    function_response: Some(FunctionResponse {
                        name: result.name,
                        response: result.value,
                    }),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Tool {
    #[serde(rename = "functionDeclarations", skip_serializing_if = "Option::is_none")]
    pub function_declarations: Option<Vec<Function>>,

    #[serde(rename = "googleSearchRetrieval", skip_serializing_if = "Option::is_none")]
    pub google_search_retrieval: Option<GoogleSearchRetrieval>,
}

#[derive(Debug, Serialize)]
pub struct GoogleSearchRetrieval {
    #[serde(rename = "disableAttribution")]
    pub disable_attribution: bool,
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
pub struct GenerateContentStreamResponse {
    pub candidates: Option<Vec<StreamCandidate>>,
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct StreamCandidate {
    pub content: Option<Content>,
    #[serde(rename = "finishReason")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

#[derive(Debug, Default, Deserialize)]
pub struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: i32,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct GenerateContentResponse {
    pub candidates: Vec<Candidate>,
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: UsageMetadata,
}

#[derive(Debug, Deserialize)]
pub struct Candidate {
    pub content: Content,
    #[serde(rename = "finishReason")]
    pub finish_reason: String,
}

impl Candidate {
    pub fn append_text(&mut self, delta: &str) {
        if let Some(part) = self.content.parts.first_mut() {
            part.text.as_mut().unwrap().push_str(delta);
        } else {
            self.content.parts.push(Part {
                text: Some(delta.to_string()),
                inline_data: None,
                function_call: None,
                function_response: None,
            })
        }
    }
}
