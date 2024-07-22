use std::path::Path;

use log::info;
use tokio::fs;

use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::config::Config;
use crate::util::exception::Exception;
use crate::util::json;

pub mod config;
pub mod function;

#[derive(Debug)]
pub struct ChatOption {
    pub temperature: f32,
}

pub enum Model {
    ChatGPT(ChatGPT),
    Gemini(Gemini),
}

impl Model {
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
