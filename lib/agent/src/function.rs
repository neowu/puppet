use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use openai::chat_api::Function;
use openai::function::FunctionImplementation;
use openai::function::FunctionStore;

#[derive(Default)]
pub struct FunctionRegistry {
    implementations: HashMap<String, Arc<FunctionImplementation>>,
    functions: HashMap<String, Function>,
}

impl FunctionRegistry {
    pub fn add(&mut self, function: Function, implementation: Arc<FunctionImplementation>) {
        self.implementations.insert(function.name.to_string(), implementation);
        self.functions.insert(function.name.to_string(), function);
    }

    pub fn create_store(&self, functions: &[String]) -> Result<FunctionStore> {
        let mut store = FunctionStore::default();
        for function in functions {
            let implementation = self
                .implementations
                .get(function)
                .with_context(|| format!("function not found, name={function}"))?
                .clone();
            let function = self
                .functions
                .get(function)
                .with_context(|| format!("function not found, name={function}"))?
                .clone();
            store.add(function, implementation);
        }
        Ok(store)
    }
}
