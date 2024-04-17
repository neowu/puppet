use std::collections::HashMap;

use crate::bot::Bot;
use crate::bot::Function;
use crate::gcloud::vertex::Vertex;
use crate::openai::chatgpt::ChatGPT;
use crate::util::exception::Exception;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bots: HashMap<String, BotConfig>,
}

impl Config {
    pub fn create(&self, name: &str) -> Result<Bot, Exception> {
        let config = self
            .bots
            .get(name)
            .ok_or_else(|| Exception::new(&format!("can not find bot, name={name}")))?;

        let mut bot = match config.r#type {
            BotType::Azure => Bot::ChatGPT(ChatGPT::new(
                config.endpoint.to_string(),
                config.params.get("api_key").unwrap().to_string(),
                config.params.get("model").unwrap().to_string(),
                Option::None,
            )),
            BotType::GCloud => Bot::Vertex(Vertex::new(
                config.endpoint.to_string(),
                config.params.get("project").unwrap().to_string(),
                config.params.get("location").unwrap().to_string(),
                config.params.get("model").unwrap().to_string(),
            )),
        };
        register_function(config, &mut bot);
        Ok(bot)
    }
}

#[derive(Deserialize, Debug)]
pub struct BotConfig {
    pub endpoint: String,
    pub r#type: BotType,
    pub params: HashMap<String, String>,
    pub functions: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub enum BotType {
    Azure,
    GCloud,
}

fn register_function(config: &BotConfig, bot: &mut Bot) {
    for function in &config.functions {
        if let "get_random_number" = function.as_str() {
            bot.register_function(
                Function {
                    name: "get_random_number".to_string(),
                    description: "generate random number".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                          "max": {
                            "type": "number",
                            "description": "max of value"
                          },
                        },
                        "required": ["max"]
                    }),
                },
                Box::new(|request| {
                    let max = request.get("max").unwrap().as_i64().unwrap();
                    let mut rng = rand::thread_rng();
                    let result = rng.gen_range(0..max);
                    json!({
                        "success": true,
                        "result": result
                    })
                }),
            );
        }
    }
}
