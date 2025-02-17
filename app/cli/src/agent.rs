use std::fs;
use std::path::Path;
use std::sync::Arc;

use agent::config::Config;
use agent::function::FunctionRegistry;
use anyhow::Result;
use framework::json;
use openai::chat_api::Function;
use rand::Rng;
use serde_json::json;
use tracing::info;

pub fn load(path: Option<&Path>) -> Result<Config> {
    let default_config_path = format!("{}/.config/puppet/agent.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}

pub fn load_function_registry() -> Result<FunctionRegistry> {
    let mut registry = FunctionRegistry::default();
    registry.add(
        Function {
            name: "get_random_number",
            description: "generate random number",
            parameters: Some(json!({
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
        Arc::new(|request| {
            let max = request.get("max").unwrap().as_i64().unwrap();
            let mut rng = rand::thread_rng();
            let result = rng.gen_range(0..max);
            json!({
                "success": true,
                "result": result
            })
        }),
    );
    registry.add(
        Function {
            name: "close_door",
            description: "close door of home",
            parameters: None,
        },
        Arc::new(|_request| {
            json!({
                "success": true
            })
        }),
    );
    Ok(registry)
}
