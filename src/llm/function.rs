use std::collections::HashMap;
use std::sync::Arc;

use log::info;
use serde::Serialize;
use tokio::task::JoinSet;

use crate::util::exception::Exception;

// both openai and gemini shares same openai schema
#[derive(Debug, Serialize)]
pub struct Function {
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

pub type FunctionImplementation = dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync;

pub struct FunctionImplementations {
    implementations: HashMap<&'static str, Arc<Box<FunctionImplementation>>>,
}

pub struct FunctionPayload {
    pub id: String,
    pub name: String,
    pub value: serde_json::Value,
}

impl FunctionImplementations {
    pub fn new() -> Self {
        FunctionImplementations {
            implementations: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: &'static str, implementation: Box<FunctionImplementation>) {
        self.implementations.insert(name, Arc::new(implementation));
    }

    pub async fn call(&self, functions: Vec<FunctionPayload>) -> Result<Vec<FunctionPayload>, Exception> {
        let mut handles = JoinSet::new();
        for function in functions {
            let implementation = self.get(&function.name)?;
            handles.spawn(async move {
                info!("call function, id={}, name={}, args={}", function.id, function.name, function.value);
                FunctionPayload {
                    id: function.id,
                    name: function.name,
                    value: implementation(&function.value),
                }
            });
        }
        let mut results = vec![];
        while let Some(result) = handles.join_next().await {
            results.push(result?)
        }
        Ok(results)
    }

    fn get(&self, name: &str) -> Result<Arc<Box<FunctionImplementation>>, Exception> {
        let function = self
            .implementations
            .get(name)
            .ok_or_else(|| Exception::ValidationError(format!("function not found, name={name}")))?;
        Ok(Arc::clone(function))
    }
}
