use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use log::info;
use serde::Deserialize;
use tokio::fs;

use crate::azure::tts::AzureTTS;
use crate::gcloud::tts::GCloudTTS;
use crate::provider::Provider;
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

impl ModelConfig {
    fn param(&self, name: &str) -> Result<String> {
        let value = self
            .params
            .get(name)
            .with_context(|| format!("config param {name} is required"))?
            .to_string();
        Ok(value)
    }
}

pub enum Speech {
    Azure(AzureTTS),
    GCloud(GCloudTTS),
}

impl Speech {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        match self {
            Speech::Azure(model) => model.synthesize(text).await,
            Speech::GCloud(model) => model.synthesize(text).await,
        }
    }
}

pub async fn load(path: Option<&Path>, name: &str) -> Result<Speech> {
    let default_config_path = format!("{}/.config/puppet/tts.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path).await?;
    let config: Config = json::from_json(&content)?;

    let config = config.models.get(name).with_context(|| format!("can not find model, name={name}"))?;

    let model = match config.provider {
        Provider::Azure => Speech::Azure(AzureTTS {
            endpoint: config.endpoint.to_string(),
            resource: config.param("resource")?,
            api_key: config.param("api_key")?,
            voice: config.param("voice")?,
        }),
        Provider::GCloud => Speech::GCloud(GCloudTTS {
            endpoint: config.endpoint.to_string(),
            project: config.param("project")?,
            voice: config.param("voice")?,
        }),
    };

    Ok(model)
}
