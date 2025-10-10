use std::path::Path;
use std::sync::Arc;

use agent::agent::Agent;
use agent::function::FunctionRegistry;
use framework::exception::Exception;
use openai::chat_api::Function;
use rand::Rng;
use serde_json::json;

pub fn load(path: Option<&Path>) -> Result<Agent, Exception> {
    let default_config_path = format!("{}/.config/puppet/agent.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    let registry = load_function_registry()?;
    let agent = Agent::load(path, &registry)?;
    Ok(agent)
}

fn load_function_registry() -> Result<FunctionRegistry, Exception> {
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
            let mut rng = rand::rng();
            let result = rng.random_range(0..max);
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
