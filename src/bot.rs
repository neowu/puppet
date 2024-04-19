use std::fs;
use std::path::Path;

use tracing::info;
use tracing::warn;

use self::config::Config;
use crate::gcloud::vertex::Vertex;
use crate::openai::chatgpt::ChatGPT;
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

pub enum Bot {
    ChatGPT(ChatGPT),
    Vertex(Vertex),
}

impl Bot {
    pub async fn chat(&mut self, message: String, handler: &impl ChatHandler) -> Result<(), Exception> {
        match self {
            Bot::ChatGPT(bot) => bot.chat(message, handler).await,
            Bot::Vertex(bot) => bot.chat(message, handler).await,
        }
    }

    pub fn file(&mut self, path: &Path) -> Result<(), Exception> {
        match self {
            Bot::ChatGPT(_bot) => {
                warn!("ChatGPT does not support uploading file");
                Ok(())
            }
            Bot::Vertex(bot) => bot.file(path),
        }
    }
}

pub fn load(path: &Path) -> Result<Config, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
