use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tracing::info;

use crate::gcloud::vertex::Vertex;
use crate::openai::chatgpt::ChatGPT;
use crate::util::exception::Exception;
use crate::util::json;

use self::config::Config;

pub mod config;

pub trait ChatHandler {
    fn on_event(&self, event: ChatEvent);
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

pub struct FunctionStore {
    pub declarations: Vec<Function>,
    pub implementations: HashMap<String, Arc<Box<FunctionImplementation>>>,
}

impl FunctionStore {
    pub fn new() -> Self {
        FunctionStore {
            declarations: vec![],
            implementations: HashMap::new(),
        }
    }

    pub fn add(&mut self, function: Function, implementation: Box<FunctionImplementation>) {
        let name = function.name.to_string();
        self.declarations.push(function);
        self.implementations.insert(name, Arc::new(implementation));
    }

    pub fn get(&self, name: &str) -> Result<Arc<Box<FunctionImplementation>>, Exception> {
        let function = Arc::clone(
            self.implementations
                .get(name)
                .ok_or_else(|| Exception::new(&format!("function not found, name={name}")))?,
        );
        Ok(function)
    }
}

pub enum Bot {
    ChatGPT(ChatGPT),
    Vertex(Vertex),
}

impl Bot {
    pub async fn chat(&mut self, message: String, handler: &dyn ChatHandler) -> Result<(), Exception> {
        match self {
            Bot::ChatGPT(bot) => bot.chat(message, handler).await,
            Bot::Vertex(bot) => bot.chat(message, handler).await,
        }
    }
}

pub fn load(path: &Path) -> Result<Config, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
