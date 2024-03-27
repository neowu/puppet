use std::error::Error;
use std::fs;
use std::path::Path;

use crate::util::json;

use self::config::Config;

pub mod config;
pub mod handler;

pub fn load(path: &Path) -> Result<Config, Box<dyn Error>> {
    println!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
