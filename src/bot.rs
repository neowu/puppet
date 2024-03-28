use std::any::Any;
use std::error::Error;
use std::fs;
use std::path::Path;

use serde::Serialize;
use tracing::info;

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

pub trait Bot {
    fn register_function(&mut self, function: Function, implementation: Box<FunctionImplementation>);
    fn as_any(&mut self) -> &mut dyn Any;
}

pub fn load(path: &Path) -> Result<Config, Box<dyn Error>> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
