use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tokio::fs;
use tracing::info;

use crate::azure::tts::AzureTTS;
use crate::gcloud::tts::GCloudTTS;
use crate::provider::Provider;
use crate::util::exception::Exception;
use crate::util::json;

#[derive(Deserialize, Debug)]
struct Config {
    models: HashMap<String, ModelConfig>,
}

#[derive(Deserialize, Debug)]
struct ModelConfig {
    endpoint: String,
    provider: Provider,
    params: HashMap<String, String>,
}

pub enum Speech {
    Azure(AzureTTS),
    GCloud(GCloudTTS),
}

impl Speech {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, Exception> {
        match self {
            Speech::Azure(model) => model.synthesize(text).await,
            Speech::GCloud(model) => model.synthesize(text).await,
        }
    }
}

pub async fn load(path: &Path, name: &str) -> Result<Speech, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path).await?;
    let config: Config = json::from_json(&content)?;

    let config = config
        .models
        .get(name)
        .ok_or_else(|| Exception::ValidationError(format!("can not find model, name={name}")))?;

    let model = match config.provider {
        Provider::Azure => Speech::Azure(AzureTTS {
            endpoint: config.endpoint.to_string(),
            resource: config.params.get("resource").unwrap().to_string(),
            api_key: config.params.get("api_key").unwrap().to_string(),
            voice: config.params.get("voice").unwrap().to_string(),
        }),
        Provider::GCloud => Speech::GCloud(GCloudTTS {
            endpoint: config.endpoint.to_string(),
            project: config.params.get("project").unwrap().to_string(),
            voice: config.params.get("voice").unwrap().to_string(),
        }),
    };

    Ok(model)
}
