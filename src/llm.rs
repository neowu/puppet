use std::fs;
use std::path::Path;

use anyhow::Result;
use log::info;

use crate::llm::config::Config;
use crate::util::json;

pub mod config;
pub mod function;

pub fn load(path: Option<&Path>) -> Result<Config> {
    let default_config_path = format!("{}/.config/puppet/llm.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
