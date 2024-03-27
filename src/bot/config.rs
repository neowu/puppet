use std::collections::HashMap;

use crate::chatgpt::ChatGPT;
use crate::openai;
use crate::openai::chat_completion::Function;
use crate::util::json::from_json;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bots: HashMap<String, Bot>,
}

impl Config {
    // TODO: think about how to make async trait object
    pub fn create_chatgpt(&self, name: &str) -> ChatGPT {
        let bot = self.bots.get(name).unwrap();

        if let BotType::Azure = bot.r#type {
            let client = openai::Client {
                endpoint: bot.endpoint.to_string(),
                api_key: bot.params.get("api_key").unwrap().to_string(),
                model: bot.params.get("model").unwrap().to_string(),
            };
            let mut chatgpt = ChatGPT::new(client, Option::None);
            register_function(&mut chatgpt);
            return chatgpt;
        }

        panic!("bot type must be azure, name={name}");
    }
}

#[derive(Deserialize, Debug)]
pub struct Bot {
    pub endpoint: String,
    pub r#type: BotType,
    pub params: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
pub enum BotType {
    Azure,
    GCloud,
}

fn register_function(chatgpt: &mut ChatGPT) {
    chatgpt.register_function(
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
        Box::new(|request_json| {
            let request: GetRandomNumberRequest = from_json(&request_json).unwrap();
            let mut rng = rand::thread_rng();
            let response = GetRandomNumberResponse {
                success: true,
                result: rng.gen_range(0..request.max),
            };
            serde_json::to_string(&response).unwrap()
        }),
    );
}

#[derive(Deserialize, Debug)]
struct GetRandomNumberRequest {
    pub max: i32,
}

#[derive(Serialize, Debug)]
struct GetRandomNumberResponse {
    pub success: bool,
    pub result: i32,
}
