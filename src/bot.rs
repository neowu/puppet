use std::fs;
use std::path::Path;

use serde::Serialize;
use tracing::info;

use crate::gcloud::vertex::Vertex;
use crate::openai::chatgpt::ChatGPT;
use crate::util::exception::Exception;
use crate::util::json;

use self::config::Config;

pub mod config;

pub trait ChatHandler {
    fn on_event(&self, event: &ChatEvent);
}

pub enum ChatEvent {
    Delta(String),
    Error(String),
    End,
}

// both openai and gemini shares same openai schema
#[derive(Debug, Serialize, Clone)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub type FunctionImplementation = dyn Fn(serde_json::Value) -> serde_json::Value + Send + Sync;

pub enum Bot {
    ChatGPT(ChatGPT),
    Vertex(Vertex),
}

impl Bot {
    pub fn register_function(&mut self, function: Function, implementation: Box<FunctionImplementation>) {
        match self {
            Bot::ChatGPT(chat_gpt) => {
                chat_gpt.register_function(function, implementation);
            }
            Bot::Vertex(vertex) => {
                vertex.register_function(function, implementation);
            }
        }
    }

    pub async fn chat(&mut self, message: &str, handler: &dyn ChatHandler) -> Result<(), Exception> {
        match self {
            Bot::ChatGPT(chat_gpt) => chat_gpt.chat(message, handler).await,
            Bot::Vertex(vertex) => vertex.chat(message, handler).await,
        }
    }
}

pub fn load(path: &Path) -> Result<Config, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
