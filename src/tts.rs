use std::path::Path;

use config::Config;
use tokio::fs;
use tracing::info;

use crate::util::exception::Exception;
use crate::util::json;

pub mod config;

pub async fn load(path: &Path) -> Result<Config, Exception> {
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path).await?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
