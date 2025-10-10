use std::collections::HashMap;
use std::env;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub proxy: HashMap<String, Proxy>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Proxy {
    url: String,
    api_key: String,
}

impl Proxy {
    pub fn url(&self, model: &str) -> String {
        self.url.replace("{model}", model)
    }

    pub fn api_key(&self) -> Result<String> {
        if let Some(env) = self.api_key.strip_prefix("env:") {
            Ok(env::var(env).context(format!("can not find env, name={env}"))?)
        } else {
            Ok(self.api_key.to_string())
        }
    }
}
