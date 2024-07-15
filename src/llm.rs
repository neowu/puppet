use std::path::Path;
use std::path::PathBuf;

use tokio::fs;
use tracing::info;

use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::config::Config;
use crate::util::exception::Exception;
use crate::util::json;

pub mod config;
pub mod function;

pub trait ChatListener {
    fn on_event(&self, event: ChatEvent);
}

pub enum ChatEvent {
    Delta(String),
    End(Usage),
}

#[derive(Debug)]
pub struct ChatOption {
    pub temperature: f32,
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
    pub async fn chat(&mut self) -> Result<String, Exception> {
        match self {
            Model::ChatGPT(model) => model.chat().await,
            Model::Gemini(model) => model.chat().await,
        }
    }

    pub fn listener(&mut self, listener: Box<dyn ChatListener>) {
        match self {
            Model::ChatGPT(model) => model.listener = Some(listener),
            Model::Gemini(model) => model.listener = Some(listener),
        }
    }

    pub fn system_message(&mut self, message: String) {
        match self {
            Model::ChatGPT(model) => model.system_message(message),
            Model::Gemini(model) => model.system_instruction(message),
        }
    }

    pub fn option(&mut self, option: ChatOption) {
        match self {
            Model::ChatGPT(model) => model.option = Some(option),
            Model::Gemini(model) => model.option = Some(option),
        }
    }

    pub async fn add_user_message(&mut self, message: String, files: Option<Vec<PathBuf>>) -> Result<(), Exception> {
        match self {
            Model::ChatGPT(model) => model.add_user_message(message, files).await,
            Model::Gemini(model) => model.add_user_text(message, files).await,
        }
    }

    pub fn add_assistant_message(&mut self, message: String) {
        match self {
            Model::ChatGPT(model) => model.add_assistant_message(message),
            Model::Gemini(model) => model.add_model_text(message),
        }
    }
}

pub async fn load(path: &Path) -> Result<Config, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path).await?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
