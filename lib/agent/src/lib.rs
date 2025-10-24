use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use framework::exception::Exception;
use framework::http::HttpClient;
use framework::json;
use serde::Deserialize;
use tracing::info;

use crate::openai::chat::Chat;
use crate::openai::function::FunctionStore;

pub mod openai;

#[derive(Deserialize, Debug)]
struct Config {
    models: HashMap<String, ModelConfig>,
}

#[derive(Deserialize, Debug)]
struct ModelConfig {
    url: String,
    api_key: String,
    model: String,
}

pub fn load(path: &Path, function_store: FunctionStore) -> Result<HashMap<String, Chat>, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let config: Config = json::load_file(path)?;

    let function_store = Arc::new(function_store);
    let http_client = HttpClient::default();

    let chats = config
        .models
        .into_iter()
        .map(|(name, model)| {
            (
                name,
                Chat::new(
                    model.url,
                    model.api_key,
                    model.model,
                    function_store.clone(),
                    http_client.clone(),
                ),
            )
        })
        .collect();
    Ok(chats)
}
