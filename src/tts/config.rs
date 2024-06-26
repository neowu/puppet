use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Deserialize, Debug)]
pub struct ModelConfig {
    pub endpoint: String,
    pub params: HashMap<String, String>,
}
