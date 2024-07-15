use std::mem;
use std::ops::Not;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::str;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use reqwest::Response;
use tokio::fs;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tracing::info;

use super::gemini_api::Content;
use super::gemini_api::FunctionCall;
use super::gemini_api::GenerationConfig;
use super::gemini_api::InlineData;
use super::gemini_api::StreamGenerateContent;
use super::gemini_api::Tool;
use super::token;
use crate::gcloud::gemini_api::GenerateContentResponse;
use crate::llm::function::FunctionStore;
use crate::llm::ChatEvent;
use crate::llm::ChatListener;
use crate::llm::ChatOption;
use crate::llm::Usage;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct Gemini {
    url: String,
    messages: Rc<Vec<Content>>,
    system_instruction: Option<Rc<Content>>,
    tools: Option<Rc<[Tool]>>,
    function_store: FunctionStore,
    pub option: Option<ChatOption>,
    pub listener: Option<Box<dyn ChatListener>>,
    last_model_message: String,
    usage: Usage,
}

impl Gemini {
    pub fn new(endpoint: String, project: String, location: String, model: String, function_store: FunctionStore) -> Self {
        let url = format!("{endpoint}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent");
        Gemini {
            url,
            messages: Rc::new(vec![]),
            system_instruction: None,
            tools: function_store.declarations.is_empty().not().then_some(Rc::from(vec![Tool {
                function_declarations: function_store.declarations.to_vec(),
            }])),
            function_store,
            option: None,
            listener: None,
            last_model_message: String::with_capacity(1024),
            usage: Usage::default(),
        }
    }

    pub async fn chat(&mut self) -> Result<String, Exception> {
        let mut result = self.process().await?;
        while let Some(function_call) = result {
            let function_response = self.function_store.call_function(function_call.name.clone(), function_call.args).await?;
            self.add_message(Content::new_function_response(function_call.name, function_response));
            result = self.process().await?;
        }
        Ok(self.last_model_message.to_string())
    }

    pub fn system_instruction(&mut self, text: String) {
        self.system_instruction = Some(Rc::new(Content::new_model_text(text)));
    }

    pub async fn add_user_text(&mut self, text: String, files: Option<Vec<PathBuf>>) -> Result<(), Exception> {
        let data = inline_datas(files).await?;
        if data.is_some() {
            self.tools = None; // function call is not supported with inline data
        }
        self.add_message(Content::new_user_text(text, data));
        Ok(())
    }

    pub fn add_model_text(&mut self, text: String) {
        self.add_message(Content::new_model_text(text));
    }

    async fn process(&mut self) -> Result<Option<FunctionCall>, Exception> {
        let (tx, rx) = channel(64);

        let response = self.call_api().await?;
        let handle = tokio::spawn(read_response_stream(response, tx));
        let function_call = self.process_response(rx).await?;
        handle.await??;

        Ok(function_call)
    }

    async fn process_response(&mut self, mut rx: Receiver<GenerateContentResponse>) -> Result<Option<FunctionCall>, Exception> {
        let mut model_message = String::new();
        let mut function_call = None;
        while let Some(response) = rx.recv().await {
            if let Some(usage) = response.usage_metadata {
                self.usage.request_tokens += usage.prompt_token_count;
                self.usage.response_tokens += usage.candidates_token_count;
            }

            let candidate = response.candidates.into_iter().next().unwrap();
            if let Some(reason) = candidate.finish_reason.as_ref() {
                if reason == "STOP" {
                    continue;
                }
            }
            if candidate.content.is_none() {
                return Err(Exception::unexpected(format!(
                    "response ended, finish_reason={}",
                    candidate.finish_reason.unwrap_or("".to_string())
                )));
            }
            if let Some(content) = candidate.content {
                let part = content.parts.into_iter().next().unwrap();

                if let Some(call) = part.function_call {
                    function_call = Some(call);
                } else if let Some(text) = part.text {
                    model_message.push_str(&text);
                    if let Some(listener) = self.listener.as_ref() {
                        listener.on_event(ChatEvent::Delta(text));
                    }
                }
            }
        }

        if let Some(call) = function_call {
            self.add_message(Content::new_function_call(call.clone()));
            return Ok(Some(call));
        }

        if !model_message.is_empty() {
            self.last_model_message = model_message.to_string();
            self.add_message(Content::new_model_text(model_message));
        }

        let usage = mem::take(&mut self.usage);
        if let Some(listener) = self.listener.as_ref() {
            listener.on_event(ChatEvent::End(usage));
        }

        Ok(None)
    }

    fn add_message(&mut self, content: Content) {
        Rc::get_mut(&mut self.messages).unwrap().push(content);
    }

    async fn call_api(&self) -> Result<Response, Exception> {
        let request = StreamGenerateContent {
            contents: Rc::clone(&self.messages),
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
            let body = str::from_utf8(&body)?;
            info!("body={}", body);
            let response_text = response.text().await?;
            return Err(Exception::ExternalError(format!(
                "failed to call gcloud api, status={status}, response={response_text}"
            )));
        }

        Ok(response)
    }
}

async fn read_response_stream(response: Response, tx: Sender<GenerateContentResponse>) -> Result<(), Exception> {
    let mut response = response;
    let mut buffer = String::with_capacity(1024);
    while let Some(chunk) = response.chunk().await? {
        buffer.push_str(str::from_utf8(&chunk).unwrap());

        // first char is '[' or ','
        if !is_valid_json(&buffer[1..]) {
            continue;
        }

        let content: GenerateContentResponse = json::from_json(&buffer[1..])?;
        tx.send(content).await?;
        buffer.clear();
    }
    Ok(())
}

fn is_valid_json(content: &str) -> bool {
    let result: serde_json::Result<serde::de::IgnoredAny> = serde_json::from_str(content);
    result.is_ok()
}

async fn inline_datas(files: Option<Vec<PathBuf>>) -> Result<Option<Vec<InlineData>>, Exception> {
    let data = if let Some(paths) = files {
        let mut data = Vec::with_capacity(paths.len());
        for path in paths {
            data.push(inline_data(&path).await?);
        }
        Some(data)
    } else {
        None
    };
    Ok(data)
}

async fn inline_data(path: &Path) -> Result<InlineData, Exception> {
    let extension = path
        .extension()
        .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", path.to_string_lossy())))?
        .to_str()
        .unwrap();
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
