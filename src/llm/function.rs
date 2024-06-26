use std::collections::HashMap;
use std::sync::Arc;

use futures::future::try_join_all;
use serde::Serialize;
use tracing::info;

use crate::util::exception::Exception;

// both openai and gemini shares same openai schema
#[derive(Debug, Serialize, Clone)]
pub struct Function {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

pub type FunctionImplementation = dyn Fn(serde_json::Value) -> serde_json::Value + Send + Sync;

pub struct FunctionStore {
    pub declarations: Vec<Function>,
    pub implementations: HashMap<String, Arc<Box<FunctionImplementation>>>,
}

impl FunctionStore {
    pub fn new() -> Self {
        FunctionStore {
            declarations: vec![],
            implementations: HashMap::new(),
        }
    }

    pub fn add(&mut self, function: Function, implementation: Box<FunctionImplementation>) {
        let name = function.name.to_string();
        self.declarations.push(function);
        self.implementations.insert(name, Arc::new(implementation));
    }

    pub async fn call_function(&self, name: String, args: serde_json::Value) -> Result<serde_json::Value, Exception> {
        let function = self.get(&name)?;
        let response = tokio::spawn(async move {
            info!("call function, name={name}, args={args}");
            function(args)
        })
        .await?;
        Ok(response)
    }

    pub async fn call_functions(&self, functions: Vec<(String, String, serde_json::Value)>) -> Result<Vec<(String, serde_json::Value)>, Exception> {
        let mut handles = Vec::with_capacity(functions.len());
        for (id, name, args) in functions {
            let function = self.get(&name)?;
            handles.push(tokio::spawn(async move {
                info!("call function, id={id}, name={name}, args={args}");
                (id, function(args))
            }));
        }
        let results = try_join_all(handles).await?;
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
