use std::{collections::HashMap, error::Error, fs, path::Path};

use crate::util::json;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bots: HashMap<String, Bot>,
}

#[derive(Deserialize, Debug)]
pub struct Bot {
    pub endpoint: String,
    pub api_key: String,
    pub params: HashMap<String, String>,
}

pub fn load(path: &Path) -> Result<Config, Box<dyn Error>> {
    println!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
