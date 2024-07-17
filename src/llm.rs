use std::path::Path;

use tokio::fs;
use tracing::info;

use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::config::Config;
use crate::util::console;
use crate::util::exception::Exception;
use crate::util::json;

pub mod config;
pub mod function;

pub trait ChatListener {
    async fn on_event(&self, event: ChatEvent) -> Result<(), Exception>;
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

pub enum Model<L>
where
    L: ChatListener,
{
    ChatGPT(ChatGPT<L>),
    Gemini(Gemini<L>),
}

impl<L: ChatListener> Model<L> {
    pub async fn chat(&mut self) -> Result<&str, Exception> {
        match self {
            Model::ChatGPT(model) => model.chat().await,
            Model::Gemini(model) => model.chat().await,
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

    pub async fn add_user_message(&mut self, message: String, files: &[&Path]) -> Result<(), Exception> {
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

pub async fn load(path: Option<&Path>) -> Result<Config, Exception> {
    let default_config_path = format!("{}/.config/puppet/llm.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path).await?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}

pub struct ConsolePrinter;

impl ChatListener for ConsolePrinter {
    async fn on_event(&self, event: ChatEvent) -> Result<(), Exception> {
        match event {
            ChatEvent::Delta(data) => {
                console::print(&data).await?;
                Ok(())
            }
            ChatEvent::End(usage) => {
                console::print("\n").await?;
                info!(
                    "usage, request_tokens={}, response_tokens={}",
                    usage.request_tokens, usage.response_tokens
                );
                Ok(())
            }
        }
    }
}
