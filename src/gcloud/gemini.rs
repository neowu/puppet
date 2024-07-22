use std::ops::Not;
use std::path::Path;
use std::rc::Rc;
use std::str;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use log::info;
use reqwest::Response;
use tokio::fs;

use super::gemini_api::Content;
use super::gemini_api::GenerateContentResponse;
use super::gemini_api::GenerationConfig;
use super::gemini_api::InlineData;
use super::gemini_api::StreamGenerateContent;
use super::gemini_api::Tool;
use super::token;
use crate::gcloud::gemini_api::Candidate;
use crate::gcloud::gemini_api::GenerateContentStreamResponse;
use crate::gcloud::gemini_api::Role;
use crate::gcloud::gemini_api::UsageMetadata;
use crate::llm::function::Function;
use crate::llm::function::FunctionImplementations;
use crate::llm::function::FunctionPayload;
use crate::llm::ChatOption;
use crate::util::console;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct Gemini {
    url: String,
    contents: Rc<Vec<Content>>,
    system_instruction: Option<Rc<Content>>,
    tools: Option<Rc<[Tool]>>,
    function_implementations: FunctionImplementations,
    pub option: Option<ChatOption>,
}

impl Gemini {
    pub fn new(
        endpoint: String,
        project: String,
        location: String,
        model: String,
        function_declarations: Vec<Function>,
        function_implementations: FunctionImplementations,
    ) -> Self {
        let url = format!("{endpoint}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent");
        Gemini {
            url,
            contents: Rc::new(vec![]),
            system_instruction: None,
            tools: function_declarations
                .is_empty()
                .not()
                .then_some(Rc::from(vec![Tool { function_declarations }])),
            function_implementations,
            option: None,
        }
    }

    pub async fn chat(&mut self) -> Result<&str, Exception> {
        self.process().await?;

        Ok(self.contents.last().unwrap().parts.first().unwrap().text.as_ref().unwrap())
    }

    pub fn system_instruction(&mut self, text: String) {
        self.system_instruction = Some(Rc::new(Content::new_model_text(text)));
    }

    pub async fn add_user_text(&mut self, text: String, files: &[&Path]) -> Result<(), Exception> {
        let data = inline_datas(files).await?;
        if !data.is_empty() {
            self.tools = None; // function call is not supported with inline data
        }
        self.add_content(Content::new_user_text(text, data));
        Ok(())
    }

    pub fn add_model_text(&mut self, text: String) {
        self.add_content(Content::new_model_text(text));
    }

    async fn process(&mut self) -> Result<(), Exception> {
        loop {
            let http_response = self.call_api().await?;
            let response = read_stream_response(http_response).await?;
            info!(
                "usage, prompt_tokens={}, candidates_tokens={}",
                response.usage_metadata.prompt_token_count, response.usage_metadata.candidates_token_count
            );
            // gemini only supports single candidate
            let candidate = response.candidates.into_iter().next().unwrap();

            let mut functions = vec![];
            for (i, part) in candidate.content.parts.iter().enumerate() {
                if let Some(ref call) = part.function_call {
                    functions.push(FunctionPayload {
                        id: i.to_string(),
                        name: call.name.to_string(),
                        value: call.args.clone(),
                    });
                }
            }

            self.add_content(candidate.content);

            if !functions.is_empty() {
                let results = self.function_implementations.call(functions).await?;
                self.add_content(Content::new_function_response(results));
            } else {
                return Ok(());
            }
        }
    }

    fn add_content(&mut self, content: Content) {
        Rc::get_mut(&mut self.contents).unwrap().push(content);
    }

    async fn call_api(&self) -> Result<Response, Exception> {
        let request = StreamGenerateContent {
            contents: Rc::clone(&self.contents),
            system_instruction: self.system_instruction.clone(),
            generation_config: GenerationConfig {
                temperature: self.option.as_ref().map_or(1.0, |option| option.temperature),
                top_p: 0.95,
                max_output_tokens: 4096,
            },
            tools: self.tools.clone(),
        };

        let body = json::to_json(&request)?;
        let body = Bytes::from(body);
        let response = http_client::http_client()
            .post(&self.url)
            .bearer_auth(token())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body.clone())
            .send()
            .await?;

        let status = response.status();
        if status != 200 {
            info!("body={}", str::from_utf8(&body)?);
            let response_text = response.text().await?;
            return Err(Exception::ExternalError(format!(
                "failed to call gcloud api, status={status}, response={response_text}"
            )));
        }

        Ok(response)
    }
}

async fn read_stream_response(mut http_response: Response) -> Result<GenerateContentResponse, Exception> {
    let mut response = GenerateContentResponse {
        candidates: vec![Candidate {
            content: Content {
                role: Role::Model,
                parts: vec![],
            },
            finish_reason: String::new(),
        }],
        usage_metadata: UsageMetadata::default(),
    };
    let candidate = response.candidates.first_mut().unwrap();

    let mut buffer = String::with_capacity(1024);
    while let Some(chunk) = http_response.chunk().await? {
        buffer.push_str(str::from_utf8(&chunk).unwrap());

        // first char is '[' or ','
        if !is_valid_json(&buffer[1..]) {
            continue;
        }

        let stream_response: GenerateContentStreamResponse = json::from_json(&buffer[1..])?;

        if let Some(value) = stream_response.usage_metadata {
            response.usage_metadata = value;
        }

        let stream_candidate = stream_response.candidates.into_iter().next().unwrap();
        if let Some(reason) = stream_candidate.finish_reason {
            candidate.finish_reason = reason;
            if candidate.finish_reason == "STOP" {
                break;
            }
        }
        if let Some(content) = stream_candidate.content {
            for part in content.parts {
                if let Some(text) = part.text {
                    candidate.append_text(&text);
                    console::print(&text).await?;
                } else {
                    // except text, all other parts send as whole
                    candidate.content.parts.push(part);
                }
            }
        }

        buffer.clear();
    }

    if candidate.content.parts.first().unwrap().text.is_some() {
        console::print("\n").await?;
    }

    Ok(response)
}

fn is_valid_json(content: &str) -> bool {
    let result: serde_json::Result<serde::de::IgnoredAny> = serde_json::from_str(content);
    result.is_ok()
}

async fn inline_datas(files: &[&Path]) -> Result<Vec<InlineData>, Exception> {
    let mut data = Vec::with_capacity(files.len());
    for file in files {
        data.push(inline_data(file).await?);
    }
    Ok(data)
}

async fn inline_data(path: &Path) -> Result<InlineData, Exception> {
    let extension = path
        .extension()
        .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", path.to_string_lossy())))?
        .to_string_lossy();
    let extension = extension.as_ref();
    let content = fs::read(path).await?;
    let mime_type = match extension {
        "jpg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        "pdf" => Ok("application/pdf".to_string()),
        _ => Err(Exception::ValidationError(format!(
            "not supported extension, path={}",
            path.to_string_lossy()
        ))),
    }?;
    Ok(InlineData {
        mime_type,
        data: BASE64_STANDARD.encode(content),
    })
}
