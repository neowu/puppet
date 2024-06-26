use std::collections::HashMap;

use rand::Rng;
use serde::Deserialize;
use serde_json::json;

use crate::gcloud::gemini::Gemini;
use crate::llm::function::Function;
use crate::llm::function::FunctionStore;
use crate::llm::Model;
use crate::openai::chatgpt::ChatGPT;
use crate::util::exception::Exception;

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

#[derive(Deserialize, Debug)]
pub enum Provider {
    Azure,
    GCloud,
}

impl Config {
    pub fn create(&self, name: &str) -> Result<Model, Exception> {
        let config = self
            .models
            .get(name)
            .ok_or_else(|| Exception::ValidationError(format!("can not find model, name={name}")))?;

        let function_store = load_function_store(config);

        let model = match config.provider {
            Provider::Azure => Model::ChatGPT(ChatGPT::new(
                config.endpoint.to_string(),
                config.params.get("model").unwrap().to_string(),
                config.params.get("api_key").unwrap().to_string(),
                config.system_message.clone(),
                function_store,
            )),
            Provider::GCloud => Model::Gemini(Gemini::new(
                config.endpoint.to_string(),
                config.params.get("project").unwrap().to_string(),
                config.params.get("location").unwrap().to_string(),
                config.params.get("model").unwrap().to_string(),
                config.system_message.clone(),
                function_store,
            )),
        };

        Ok(model)
    }
}

fn load_function_store(config: &ModelConfig) -> FunctionStore {
    let mut function_store = FunctionStore::new();
    for function in &config.functions {
        if let "get_random_number" = function.as_str() {
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
        if let "close_door" = function.as_str() {
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
        if let "close_window" = function.as_str() {
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
    }
    function_store
}
