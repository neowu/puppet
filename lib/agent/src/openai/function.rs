use std::collections::HashMap;
use std::sync::Arc;

use framework::exception;
use framework::exception::Exception;

use crate::openai::chat_api::Function;
use crate::openai::chat_api::Tool;

pub type FunctionImplementation = dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync;

#[derive(Default)]
pub struct FunctionStore {
    implementations: HashMap<&'static str, Arc<FunctionImplementation>>,
    definitions: HashMap<&'static str, Tool>,
}

pub struct FunctionPayload {
    pub id: String,
    pub name: String,
    pub value: serde_json::Value,
}

impl FunctionStore {
    pub fn add(&mut self, function: Function, implementation: Arc<FunctionImplementation>) {
        self.implementations.insert(function.name, implementation);
        self.definitions.insert(
            function.name,
            Tool {
                r#type: "function",
                function,
            },
        );
    }

    pub fn definitions(&self, functions: &Option<Vec<String>>) -> Option<Vec<Tool>> {
        if let Some(functions) = functions {
            let mut definitions = Vec::with_capacity(functions.len());
            for function in functions {
                definitions.push(self.definitions.get(function.as_str()).unwrap().clone());
            }
            Some(definitions)
        } else {
            None
        }
    }

    pub fn call(&self, functions: Vec<FunctionPayload>) -> Result<Vec<FunctionPayload>, Exception> {
        let mut results = vec![];
        for function in functions {
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
