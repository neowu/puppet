use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio::task::JoinSet;
use tracing::info;

use crate::util::exception::Exception;

// both openai and gemini shares same openai schema
#[derive(Debug, Serialize)]
pub struct Function {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

pub type FunctionImplementation = dyn Fn(&serde_json::Value) -> serde_json::Value + Send + Sync;

pub struct FunctionObject {
    pub id: String,
    pub name: String,
    pub value: serde_json::Value,
}

pub struct FunctionStore {
    pub declarations: Vec<Function>,
    pub implementations: FunctionImplementations,
}

pub struct FunctionImplementations {
    implementations: HashMap<String, Arc<Box<FunctionImplementation>>>,
}

impl FunctionStore {
    pub fn new() -> Self {
        FunctionStore {
            declarations: vec![],
            implementations: FunctionImplementations {
                implementations: HashMap::new(),
            },
        }
    }

    pub fn add(&mut self, function: Function, implementation: Box<FunctionImplementation>) {
        let name = function.name.to_string();
        self.declarations.push(function);
        self.implementations.implementations.insert(name, Arc::new(implementation));
    }
}

impl FunctionImplementations {
    pub async fn call_functions(&self, functions: Vec<FunctionObject>) -> Result<Vec<FunctionObject>, Exception> {
        let mut handles = JoinSet::new();
        for function_param in functions {
            let function = self.get(&function_param.name)?;
            handles.spawn(async move {
                info!(
                    "call function, id={}, name={}, args={}",
                    function_param.id, function_param.name, function_param.value
                );
                FunctionObject {
                    id: function_param.id,
                    name: function_param.name,
                    value: function(&function_param.value),
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
