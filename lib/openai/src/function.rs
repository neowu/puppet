use std::collections::HashMap;
use std::sync::Arc;

use framework::exception;
use framework::exception::Exception;
use tracing::info;

use crate::chat_api::Function;
use crate::chat_api::Tool;

pub type FunctionImplementation = dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync;

#[derive(Default)]
pub struct FunctionStore {
    implementations: HashMap<String, Arc<FunctionImplementation>>,
    definitions: Vec<Tool>,
}

pub struct FunctionPayload {
    pub id: String,
    pub name: String,
    pub value: serde_json::Value,
}

impl FunctionStore {
    pub fn add(&mut self, function: Function, implementation: Arc<FunctionImplementation>) {
        self.implementations.insert(function.name.to_string(), implementation);
        self.definitions.push(Tool {
            r#type: "function",
            function,
        });
    }

    pub fn definitions(&self) -> Option<Vec<Tool>> {
        if self.definitions.is_empty() {
            None
        } else {
            Some(self.definitions.clone())
        }
    }

    pub fn call(&self, functions: Vec<FunctionPayload>) -> Result<Vec<FunctionPayload>, Exception> {
        let mut results = vec![];
        for function in functions {
            info!(
                "call function, id={}, name={}, args={}",
                function.id, function.name, function.value
            );
            let implementation = self
                .implementations
                .get(function.name.as_str())
                .ok_or_else(|| exception!(message = format!("function not found, function={}", function.name)))?;
            let value = implementation(&function.value);

            results.push(FunctionPayload {
                id: function.id,
                name: function.name,
                value,
            })
        }
        Ok(results)
    }
}
