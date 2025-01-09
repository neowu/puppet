use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use framework::json;
use openai::chat::Chat;
use openai::chat_api::Function;
use openai::chat_api::Tool;
use openai::function::FUNCTION_STORE;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

pub fn load(path: Option<&Path>) -> Result<Config> {
    let default_config_path = format!("{}/.config/puppet/llm.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Deserialize, Debug)]
pub struct ModelConfig {
    pub url: String,
    pub api_key: String,
    pub model: String,
    pub system_message: Option<String>,
    pub functions: Vec<String>,
}

impl Config {
    pub fn create(&self, name: &str) -> Result<Chat> {
        let config = self.models.get(name).with_context(|| format!("can not find model, name={name}"))?;

        info!("create model, name={name}");

        let tools = load_functions(config)?;
        let tools = if tools.is_empty() { None } else { Some(Arc::from(tools)) };

        let mut model = Chat::new(config.url.to_string(), config.api_key.to_string(), config.model.to_string(), tools);

        if let Some(message) = config.system_message.as_ref() {
            model.system_message(message.to_string());
        }

        Ok(model)
    }
}

fn load_functions(config: &ModelConfig) -> Result<Vec<Tool>> {
    let mut declarations: Vec<Tool> = vec![];
    let mut function_store = FUNCTION_STORE.lock().unwrap();
    for function in &config.functions {
        info!("load function, name={function}");
        match function.as_str() {
            "get_random_number" => {
                declarations.push(Tool {
                    r#type: "function",
                    function: Function {
                        name: "get_random_number",
                        description: "generate random number",
                        parameters: Some(serde_json::json!({
                            "type": "object",
                            "properties": {
                              "max": {
                                "type": "number",
                                "description": "max of value"
                              },
                            },
                            "required": ["max"]
                        })),
                    },
                });
                function_store.add(
                    "get_random_number",
                    Box::new(|request| {
                        let max = request.get("max").unwrap().as_i64().unwrap();
                        let mut rng = rand::thread_rng();
                        let result = rng.gen_range(0..max);
                        json!({
                            "success": true,
                            "result": result
                        })
                    }),
                )
            }
            "close_door" => {
                declarations.push(Tool {
                    r#type: "function",
                    function: Function {
                        name: "close_door",
                        description: "close door of home",
                        parameters: None,
                    },
                });
                function_store.add(
                    "close_door",
                    Box::new(|_request| {
                        json!({
                            "success": true
                        })
                    }),
                );
            }
            _ => return Err(anyhow!("unknown function, name={function}")),
        }
    }
    Ok(declarations)
}
