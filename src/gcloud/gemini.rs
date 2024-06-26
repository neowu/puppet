use std::fs;
use std::mem;
use std::ops::Not;
use std::path::Path;
use std::rc::Rc;
use std::str;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tracing::info;

use super::gemini_api::Content;
use super::gemini_api::FunctionCall;
use super::gemini_api::GenerationConfig;
use super::gemini_api::InlineData;
use super::gemini_api::Role;
use super::gemini_api::StreamGenerateContent;
use super::gemini_api::Tool;
use super::token;
use crate::gcloud::gemini_api::GenerateContentResponse;
use crate::llm::function::FunctionStore;
use crate::llm::ChatEvent;
use crate::llm::ChatHandler;
use crate::llm::Usage;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct Gemini {
    url: String,
    messages: Rc<Vec<Content>>,
    system_message: Option<Rc<Content>>,
    tools: Option<Rc<[Tool]>>,
    function_store: FunctionStore,
    data: Vec<InlineData>,
    usage: Usage,
}

impl Gemini {
    pub fn new(
        endpoint: String,
        project: String,
        location: String,
        model: String,
        system_message: Option<String>,
        function_store: FunctionStore,
    ) -> Self {
        let url = format!("{endpoint}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent");
        Gemini {
            url,
            messages: Rc::new(vec![]),
            system_message: system_message.map(|message| Rc::new(Content::new_text(Role::Model, message))),
            tools: function_store.declarations.is_empty().not().then_some(Rc::from(vec![Tool {
                function_declarations: function_store.declarations.to_vec(),
            }])),
            function_store,
            data: vec![],
            usage: Usage::default(),
        }
    }

    pub async fn chat(&mut self, message: String, handler: &impl ChatHandler) -> Result<(), Exception> {
        let data = mem::take(&mut self.data);
        let mut result = self.process(Content::new_text_with_inline_data(message, data), handler).await?;

        while let Some(function_call) = result {
            let function_response = self.function_store.call_function(function_call.name.clone(), function_call.args).await?;
            let content = Content::new_function_response(function_call.name, function_response);
            result = self.process(content, handler).await?;
        }
        Ok(())
    }

    pub fn file(&mut self, path: &Path) -> Result<(), Exception> {
        let extension = path
            .extension()
            .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", path.to_string_lossy())))?
            .to_str()
            .unwrap();
        let content = fs::read(path)?;
        let mime_type = match extension {
            "jpg" => Ok("image/jpeg".to_string()),
            "png" => Ok("image/png".to_string()),
            "pdf" => Ok("application/pdf".to_string()),
            _ => Err(Exception::ValidationError(format!(
                "not supported extension, path={}",
                path.to_string_lossy()
            ))),
        }?;
        info!(
            "file added, will submit with next message, mime_type={mime_type}, path={}",
            path.to_string_lossy()
        );
        self.data.push(InlineData {
            mime_type,
            data: BASE64_STANDARD.encode(content),
        });
        Ok(())
    }

    async fn process(&mut self, content: Content, handler: &impl ChatHandler) -> Result<Option<FunctionCall>, Exception> {
        self.add_message(content);

        let response = self.call_api().await?;

        let (tx, rx) = channel(64);
        let handle = tokio::spawn(read_response_stream(response, tx));
        let function_call = self.process_response(rx, handler).await;
        handle.await??;

        Ok(function_call)
    }

    async fn process_response(&mut self, mut rx: Receiver<GenerateContentResponse>, handler: &impl ChatHandler) -> Option<FunctionCall> {
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
            match candidate.content {
                Some(content) => {
                    let part = content.parts.into_iter().next().unwrap();

                    if let Some(call) = part.function_call {
                        function_call = Some(call);
                    } else if let Some(text) = part.text {
                        model_message.push_str(&text);
                        handler.on_event(ChatEvent::Delta(text));
                    }
                }
                None => {
                    handler.on_event(ChatEvent::Error(format!(
                        "response ended, finish_reason={}",
                        candidate.finish_reason.unwrap_or("".to_string())
                    )));
                }
            }
        }

        if let Some(call) = function_call {
            self.add_message(Content::new_function_call(call.clone()));
            return Some(call);
        }

        if !model_message.is_empty() {
            self.add_message(Content::new_text(Role::Model, model_message));
        }

        let usage = mem::take(&mut self.usage);
        handler.on_event(ChatEvent::End(usage));

        None
    }

    fn add_message(&mut self, content: Content) {
        Rc::get_mut(&mut self.messages).unwrap().push(content);
    }

    async fn call_api(&self) -> Result<Response, Exception> {
        let request = StreamGenerateContent {
            contents: Rc::clone(&self.messages),
            system_instruction: self.system_message.clone(),
            generation_config: GenerationConfig {
                temperature: 1.0,
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
            let body = str::from_utf8(&body).unwrap();
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
    let stream = &mut response.bytes_stream();

    let mut buffer = String::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                buffer.push_str(str::from_utf8(&chunk).unwrap());

                // first char is '[' or ','
                if !is_valid_json(&buffer[1..]) {
                    continue;
                }

                let content: GenerateContentResponse = json::from_json(&buffer[1..])?;
                tx.send(content).await?;
                buffer.clear();
            }
            Err(err) => {
                return Err(Exception::unexpected(err));
            }
        }
    }
    Ok(())
}

fn is_valid_json(content: &str) -> bool {
    let result: serde_json::Result<serde::de::IgnoredAny> = serde_json::from_str(content);
    result.is_ok()
}
