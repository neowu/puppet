use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use agent::openai::chat::Chat;
use agent::openai::chat_api::Function;
use agent::openai::function::FunctionStore;
use framework::exception::Exception;
use rand::Rng;
use serde_json::json;

pub struct TestStruct {}

pub fn load(path: &Path) -> Result<HashMap<String, Chat>, Exception> {
    let store = create_function_store()?;
    let agent = agent::load(path, store)?;
    Ok(agent)
}

fn create_function_store() -> Result<FunctionStore, Exception> {
    let mut store = FunctionStore::default();
    store.add(
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
    store.add(
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
    Ok(store)
}
