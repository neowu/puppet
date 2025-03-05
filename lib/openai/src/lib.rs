use std::env;

use anyhow::Context;
use anyhow::Result;

pub mod chat;
pub mod chat_api;
pub mod embedding;

pub mod function;

fn api_key(api_key: &String) -> Result<String> {
    if let Some(env) = api_key.strip_prefix("env:") {
        Ok(env::var(env).context(format!("can not find env, name={env}"))?)
    } else {
        Ok(api_key.to_string())
    }
}
