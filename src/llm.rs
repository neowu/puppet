use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use anyhow::Result;
use futures::Stream;
use log::info;
use tokio::sync::mpsc::Receiver;

use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::config::Config;
use crate::util::json;

pub mod config;
pub mod function;

#[derive(Default, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
}

#[derive(Debug)]
pub struct ChatOption {
    pub temperature: f32,
}

pub struct TextStream {
    rx: Receiver<String>,
}

impl TextStream {
    pub fn new(rx: Receiver<String>) -> Self {
        TextStream { rx }
    }
}

impl Stream for TextStream {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(context)
    }
}

pub enum Model {
    ChatGPT(ChatGPT),
    Gemini(Gemini),
}

impl Model {
    pub async fn generate(&mut self) -> Result<TextStream> {
        match self {
            Model::ChatGPT(model) => model.generate().await,
            Model::Gemini(model) => model.generate().await,
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
            Model::ChatGPT(model) => model.option(option),
            Model::Gemini(model) => model.option(option),
        }
    }

    pub fn add_user_message(&mut self, message: String, files: &[&Path]) -> Result<()> {
        match self {
            Model::ChatGPT(model) => model.add_user_message(message, files),
            Model::Gemini(model) => model.add_user_text(message, files),
        }
    }

    pub fn add_assistant_message(&mut self, message: String) {
        match self {
            Model::ChatGPT(model) => model.add_assistant_message(message),
            Model::Gemini(model) => model.add_model_text(message),
        }
    }

    pub fn usage(&self) -> TokenUsage {
        match self {
            Model::ChatGPT(model) => model.usage(),
            Model::Gemini(model) => model.usage(),
        }
    }
}

pub fn load(path: Option<&Path>) -> Result<Config> {
    let default_config_path = format!("{}/.config/puppet/llm.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
