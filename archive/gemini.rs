use std::fs;
use std::path::Path;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::anyhow;
use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::StreamExt;
use log::info;
use reqwest::Response;
use tokio::sync::mpsc;

use super::gemini_api::Content;
use super::gemini_api::GenerateContentResponse;
use super::gemini_api::GenerationConfig;
use super::gemini_api::GoogleSearchRetrieval;
use super::gemini_api::InlineData;
use super::gemini_api::StreamGenerateContent;
use super::gemini_api::Tool;
use super::token;
use crate::gcloud::gemini_api::Candidate;
use crate::gcloud::gemini_api::GenerateContentStreamResponse;
use crate::gcloud::gemini_api::Role;
use crate::gcloud::gemini_api::UsageMetadata;
use crate::llm::function::Function;
use crate::llm::function::FunctionPayload;
use crate::llm::function::FUNCTION_STORE;
use crate::llm::ChatOption;
use crate::llm::TextStream;
use crate::llm::TokenUsage;
use crate::util::http_client::ResponseExt;
use crate::util::http_client::HTTP_CLIENT;
use crate::util::json;
use crate::util::path::PathExt;

pub struct Gemini {
    context: Arc<Mutex<Context>>,
}

struct Context {
    url: String,
    contents: Arc<Vec<Content>>,
    system_instruction: Option<Arc<Content>>,
    tools: Option<Arc<[Tool]>>,
    option: Option<ChatOption>,
    usage: TokenUsage,
}

impl Gemini {
    pub fn new(endpoint: String, project: String, location: String, model: String, functions: Vec<Function>) -> Self {
        let url = format!("{endpoint}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent?alt=sse");
        let tools = if functions.is_empty() {
            // google_search_retrieval can not be used with function
            vec![Tool {
                function_declarations: None,
                google_search_retrieval: Some(GoogleSearchRetrieval { disable_attribution: false }),
            }]
        } else {
            vec![Tool {
                function_declarations: Some(functions),
                google_search_retrieval: None,
            }]
        };
        Gemini {
            context: Arc::new(Mutex::new(Context {
                url,
                contents: Arc::new(vec![]),
                system_instruction: None,
                tools: Some(Arc::from(tools)),
                option: None,
                usage: TokenUsage::default(),
            })),
        }
    }

    pub async fn generate(&self) -> Result<TextStream> {
        let (tx, rx) = mpsc::channel(64);
        let context = Arc::clone(&self.context);
        tokio::spawn(async move { process(context, tx).await.unwrap() });
        Ok(TextStream::new(rx))
    }

    pub fn system_instruction(&mut self, text: String) {
        self.context.lock().unwrap().system_instruction = Some(Arc::new(Content::new_model_text(text)));
    }

    pub fn add_user_text(&mut self, text: String, files: &[&Path]) -> Result<()> {
        let data = inline_datas(files)?;
        let mut context = self.context.lock().unwrap();
        if !data.is_empty() {
            context.tools = None; // function call is not supported with inline data
        }
        context.add_content(Content::new_user_text(text, data));
        Ok(())
    }

    pub fn add_model_text(&mut self, text: String) {
        self.context.lock().unwrap().add_content(Content::new_model_text(text));
    }

    pub fn option(&mut self, option: ChatOption) {
        self.context.lock().unwrap().option = Some(option);
    }

    pub fn usage(&self) -> TokenUsage {
        self.context.lock().unwrap().usage.clone()
    }
}

impl Context {
    fn add_content(&mut self, content: Content) {
        Arc::get_mut(&mut self.contents).unwrap().push(content);
    }
}

async fn process(context: Arc<Mutex<Context>>, tx: mpsc::Sender<String>) -> Result<()> {
    loop {
        let http_response = call_api(Arc::clone(&context)).await?;
        let response = read_sse_response(http_response, &tx).await?;

        let mut context = context.lock().unwrap();
        context.usage.prompt_tokens += response.usage_metadata.prompt_token_count;
        context.usage.completion_tokens += response.usage_metadata.candidates_token_count;

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

        context.add_content(candidate.content);

        if !functions.is_empty() {
            let results = FUNCTION_STORE.lock().unwrap().call(functions)?;
            context.add_content(Content::new_function_response(results));
        } else {
            return Ok(());
        }
    }
}

async fn call_api(context: Arc<Mutex<Context>>) -> Result<Response> {
    let http_request;
    let body;
    {
        let context = context.lock().unwrap();
        let request = StreamGenerateContent {
            contents: Arc::clone(&context.contents),
            system_instruction: context.system_instruction.clone(),
            generation_config: GenerationConfig {
                temperature: context.option.as_ref().map_or(1.0, |option| option.temperature),
                top_p: 0.95,
                max_output_tokens: 4096,
            },
            tools: context.tools.clone(),
        };

        body = Bytes::from(json::to_json(&request)?);
        http_request = HTTP_CLIENT
            .post(&context.url)
            .bearer_auth(token())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body.clone());
    }
    let response = http_request.send().await?;
    let status = response.status();
    if status != 200 {
        let body = str::from_utf8(&body)?;
        info!("body={}", body);
        let response_text = response.text().await?;
        return Err(anyhow!("failed to call gcloud api, status={status}, response={response_text}"));
    }

    Ok(response)
}

async fn read_sse_response(http_response: Response, tx: &mpsc::Sender<String>) -> Result<GenerateContentResponse> {
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

    let mut lines = http_response.lines();
    while let Some(line) = lines.next().await {
        let line = line?;
        if let Some(data) = line.strip_prefix("data: ") {
            let stream_response: GenerateContentStreamResponse = json::from_json(data)?;
            if let Some(value) = stream_response.usage_metadata {
                response.usage_metadata = value;
            }

            if let Some(stream_candidates) = stream_response.candidates {
                let stream_candidate = stream_candidates.into_iter().next().unwrap();
                if let Some(content) = stream_candidate.content {
                    for part in content.parts {
                        if let Some(text) = part.text {
                            if text.is_empty() {
                                // for function, it response with text=Some("") with finish_reason=STOP
                                break;
                            }
                            candidate.append_text(&text);
                            tx.send(text).await?
                        } else {
                            // except text, all other parts send as whole
                            candidate.content.parts.push(part);
                        }
                    }
                }
                if let Some(reason) = stream_candidate.finish_reason {
                    candidate.finish_reason = reason;
                    if candidate.finish_reason == "STOP" {
                        break;
                    }
                }
            }
        }
    }

    Ok(response)
}

fn inline_datas(files: &[&Path]) -> Result<Vec<InlineData>> {
    let mut data = Vec::with_capacity(files.len());
    for file in files {
        data.push(inline_data(file)?);
    }
    Ok(data)
}

fn inline_data(path: &Path) -> Result<InlineData> {
    let extension = path.file_extension()?;
    let content = fs::read(path)?;
    let mime_type = match extension {
        "jpg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        "pdf" => Ok("application/pdf".to_string()),
        _ => Err(anyhow!("not supported extension, path={}", path.to_string_lossy())),
    }?;
    Ok(InlineData {
        mime_type,
        data: BASE64_STANDARD.encode(content),
    })
}
