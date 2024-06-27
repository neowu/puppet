use std::path::Path;

use tokio::fs;
use tracing::info;
use tracing::warn;

use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::config::Config;
use crate::util::exception::Exception;
use crate::util::json;

pub mod config;
pub mod function;

pub trait ChatHandler {
    fn on_event(&self, event: ChatEvent);
}

pub enum ChatEvent {
    Delta(String),
    Error(String),
    End(Usage),
}

#[derive(Default)]
pub struct Usage {
    pub request_tokens: i32,
    pub response_tokens: i32,
}

pub enum Model {
    ChatGPT(ChatGPT),
    Gemini(Gemini),
}

impl Model {
    pub async fn chat(&mut self, message: String, handler: &impl ChatHandler) -> Result<(), Exception> {
        match self {
            Model::ChatGPT(model) => model.chat(message, handler).await,
            Model::Gemini(model) => model.chat(message, handler).await,
        }
    }

    pub fn file(&mut self, path: &Path) -> Result<(), Exception> {
        match self {
            Model::ChatGPT(_model) => {
                warn!("ChatGPT does not support uploading file");
                Ok(())
            }
            Model::Gemini(model) => model.file(path),
        }
    }
}

pub async fn load(path: &Path) -> Result<Config, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path).await?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
