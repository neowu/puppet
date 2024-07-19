use std::collections::HashMap;

use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use super::function::FunctionImplementations;
use crate::azure::chatgpt::ChatGPT;
use crate::gcloud::gemini::Gemini;
use crate::llm::function::Function;
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

        let (function_declarations, function_implementations) = load_functions(config)?;

        let mut model = match config.provider {
            Provider::Azure => Model::ChatGPT(ChatGPT::new(
                config.endpoint.to_string(),
                config.params.get("model").unwrap().to_string(),
                config.params.get("api_key").unwrap().to_string(),
                function_declarations,
                function_implementations,
            )),
            Provider::GCloud => Model::Gemini(Gemini::new(
                config.endpoint.to_string(),
                config.params.get("project").unwrap().to_string(),
                config.params.get("location").unwrap().to_string(),
                config.params.get("model").unwrap().to_string(),
                function_declarations,
                function_implementations,
            )),
        };

        if let Some(message) = config.system_message.as_ref() {
            model.system_message(message.to_string());
        }

        Ok(model)
    }
}

fn load_functions(config: &ModelConfig) -> Result<(Vec<Function>, FunctionImplementations), Exception> {
    let mut declarations: Vec<Function> = vec![];
    let mut implementations = FunctionImplementations::new();

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
                implementations.add(
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
                implementations.add(
                    "close_door",
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
    Ok((declarations, implementations))
}
