use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use framework::json;
use openai::chat::Chat;
use serde::Deserialize;
use tracing::info;

use crate::agent::Agent;
use crate::function::FunctionRegistry;

#[derive(Deserialize, Debug)]
pub struct Config {
    models: HashMap<String, ModelConfig>,
    agents: HashMap<String, AgentConfig>,
}

#[derive(Deserialize, Debug)]
pub struct ModelConfig {
    pub url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Deserialize, Debug)]
pub struct AgentConfig {
    pub model: String,
    pub system_message: Option<String>,
    pub functions: Vec<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        info!("load config, path={}", path.to_string_lossy());
        let content = fs::read_to_string(path)?;
        let config: Config = json::from_json(&content)?;
        Ok(config)
    }

    pub fn create(&self, name: &str, registry: &FunctionRegistry) -> Result<Agent> {
        let agent_config = self
            .agents
            .get(name)
            .with_context(|| format!("can not find agent, name={name}"))?;

        info!("create agent, name={name}");

        let model_config = self
            .models
            .get(&agent_config.model)
            .with_context(|| format!("can not find model, name={}", agent_config.model))?;

        let function_store = registry.create_store(&agent_config.functions)?;

        let mut chat = Chat::new(
            model_config.url.to_string(),
            model_config.api_key.to_string(),
            model_config.model.to_string(),
            function_store,
        );

        if let Some(message) = agent_config.system_message.as_ref() {
            chat.system_message(message.to_string());
        }

        Ok(Agent { chat })
    }
}
