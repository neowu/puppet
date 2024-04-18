use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;
use serde::Serialize;
use tokio::task::JoinHandle;
use tracing::info;

use crate::util::exception::Exception;

// both openai and gemini shares same openai schema
#[derive(Debug, Serialize, Clone)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
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

    pub async fn call_function(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value, Exception> {
        info!("call function, name={name}, args={args}");
        let function = self.get(name)?;
        let response = tokio::spawn(async move { function(args) }).await?;
        Ok(response)
    }

    pub async fn call_functions(&self, functions: Vec<(String, String, serde_json::Value)>) -> Result<Vec<(String, serde_json::Value)>, Exception> {
        let handles: Result<Vec<JoinHandle<_>>, _> = functions
            .into_iter()
            .map(|(id, name, args)| {
                info!("call function, id={id}, name={name}, args={args}");
                let function = self.get(&name)?;
                Ok::<JoinHandle<_>, Exception>(tokio::spawn(async move { (id, function(args)) }))
            })
            .collect();

        let results = join_all(handles?).await.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(results)
    }

    fn get(&self, name: &str) -> Result<Arc<Box<FunctionImplementation>>, Exception> {
        let function = Arc::clone(
            self.implementations
                .get(name)
                .ok_or_else(|| Exception::new(format!("function not found, name={name}")))?,
        );
        Ok(function)
    }
}
