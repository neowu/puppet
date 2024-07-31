use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use log::info;
use serde::Serialize;

// both openai and gemini shares same openai schema
#[derive(Debug, Serialize)]
pub struct Function {
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

pub type FunctionImplementation = dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync;

pub struct FunctionStore {
    implementations: HashMap<&'static str, Box<FunctionImplementation>>,
}

pub fn function_store<'a>() -> MutexGuard<'a, FunctionStore> {
    static FUNCTION_STORE: LazyLock<Mutex<FunctionStore>> = LazyLock::new(|| Mutex::new(FunctionStore::new()));
    FUNCTION_STORE.lock().unwrap()
}

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
