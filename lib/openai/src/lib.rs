use std::env;

use framework::exception::Exception;

pub mod chat;
pub mod chat_api;

pub mod function;

fn api_key(api_key: &String) -> Result<String, Exception> {
    if let Some(env) = api_key.strip_prefix("env:") {
        Ok(env::var(env)?)
    } else {
        Ok(api_key.to_string())
    }
}
