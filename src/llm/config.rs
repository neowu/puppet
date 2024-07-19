use std::collections::HashMap;

use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::function::Function;
use crate::llm::function::FunctionStore;
use crate::llm::Model;
use crate::provider::Provider;
use crate::util::exception::Exception;
use crate::util::json;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Deserialize, Debug)]
pub struct ModelConfig {
    pub endpoint: String,
    pub provider: Provider,
    pub system_message: Option<String>,
    pub params: HashMap<String, String>,
    pub functions: Vec<String>,
}

impl Config {
    pub fn create(&self, name: &str) -> Result<Model, Exception> {
        let config = self
            .models
            .get(name)
            .ok_or_else(|| Exception::ValidationError(format!("can not find model, name={name}")))?;

        info!("create model, name={name}, provider={}", json::to_json_value(&config.provider)?);

        let function_store = load_function_store(config)?;

        let mut model = match config.provider {
            Provider::Azure => Model::ChatGPT(ChatGPT::new(
                config.endpoint.to_string(),
                config.params.get("model").unwrap().to_string(),
                config.params.get("api_key").unwrap().to_string(),
                function_store,
            )),
            Provider::GCloud => Model::Gemini(Gemini::new(
                config.endpoint.to_string(),
                config.params.get("project").unwrap().to_string(),
                config.params.get("location").unwrap().to_string(),
                config.params.get("model").unwrap().to_string(),
                function_store,
            )),
        };

        if let Some(message) = config.system_message.as_ref() {
            model.system_message(message.to_string());
        }

        Ok(model)
    }
}

fn load_function_store(config: &ModelConfig) -> Result<FunctionStore, Exception> {
    let mut function_store = FunctionStore::new();
    for function in &config.functions {
        info!("load function, name={function}");
        match function.as_str() {
            "get_random_number" => {
                function_store.add(
                    Function {
                        name: "get_random_number".to_string(),
                        description: "generate random number".to_string(),
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
                    Box::new(|request| {
                        let max = request.get("max").unwrap().as_i64().unwrap();
                        let mut rng = rand::thread_rng();
                        let result = rng.gen_range(0..max);
                        json!({
                            "success": true,
                            "result": result
                        })
                    }),
                );
            }
            "close_door" => {
                function_store.add(
                    Function {
                        name: "close_door".to_string(),
                        description: "close door of home".to_string(),
                        parameters: None,
                    },
                    Box::new(|_request| {
                        json!({
                            "success": true
                        })
                    }),
                );
            }
            "close_window" => {
                function_store.add(
                    Function {
                        name: "close_window".to_string(),
                        description: "close window of home with id".to_string(),
                        parameters: Some(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "id of window"
                                }
                            }
                        })),
                    },
                    Box::new(|_request| {
                        json!({
                            "success": true
                        })
                    }),
                );
            }
            _ => return Err(Exception::ValidationError(format!("unknown function, name={function}"))),
        }
    }
    Ok(function_store)
}
