use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use tracing::info;

pub type FunctionImplementation = dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync;

pub struct FunctionStore {
    implementations: HashMap<&'static str, Box<FunctionImplementation>>,
}

pub static FUNCTION_STORE: LazyLock<Mutex<FunctionStore>> = LazyLock::new(|| Mutex::new(FunctionStore::new()));

pub struct FunctionPayload {
    pub id: String,
    pub name: String,
    pub value: serde_json::Value,
}

impl FunctionStore {
    fn new() -> Self {
        FunctionStore {
            implementations: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: &'static str, implementation: Box<FunctionImplementation>) {
        self.implementations.insert(name, implementation);
    }

    pub fn call(&self, functions: Vec<FunctionPayload>) -> Result<Vec<FunctionPayload>> {
        let mut results = vec![];
        for function in functions {
            info!("call function, id={}, name={}, args={}", function.id, function.name, function.value);
            let implementation = self
                .implementations
                .get(function.name.as_str())
                .with_context(|| anyhow!("function not found, name={}", function.name))?;
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
