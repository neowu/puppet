use std::collections::HashMap;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use log::info;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;

use super::function::FUNCTION_STORE;
use crate::llm::function::Function;
use crate::openai::chat::Chat;

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

        let functions = load_functions(config)?;

        let mut model = Chat::new(config.url.to_string(), config.api_key.to_string(), config.model.to_string(), functions);

        if let Some(message) = config.system_message.as_ref() {
            model.system_message(message.to_string());
        }

        Ok(model)
    }
}

fn load_functions(config: &ModelConfig) -> Result<Vec<Function>> {
    let mut declarations: Vec<Function> = vec![];
    let mut function_store = FUNCTION_STORE.lock().unwrap();
    for function in &config.functions {
        info!("load function, name={function}");
        match function.as_str() {
            "get_random_number" => {
                declarations.push(Function {
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
                declarations.push(Function {
                    name: "close_door",
                    description: "close door of home",
                    parameters: None,
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
