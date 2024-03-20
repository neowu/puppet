use std::error::Error;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;

use clap::Args;
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::chatgpt;
use crate::chatgpt::ChatGPT;
use crate::chatgpt::ChatHandler;
use crate::config;
use crate::openai;
use crate::openai::chat_completion::Function;
use crate::util::json::from_json;

#[derive(Args)]
#[command(about = "chat")]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: String,
}

struct ConsoleHandler;

impl ChatHandler for ConsoleHandler {
    fn on_event(&self, event: &chatgpt::ChatEvent) {
        match event {
            chatgpt::ChatEvent::Delta(data) => {
                print_flush(data).unwrap();
            }
            chatgpt::ChatEvent::Error(error) => {
                println!("Error: {}", error);
            }
            chatgpt::ChatEvent::End => {
                println!();
            }
        }
    }
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

impl Chat {
    pub async fn execute(&self) -> Result<(), Box<dyn Error>> {
        let config = config::load(Path::new(&self.conf))?;
        let bot = config.bots.get("azure").unwrap();
        let client = openai::Client {
            endpoint: bot.endpoint.to_string(),
            api_key: bot.api_key.to_string(),
            model: bot.params.get("model").unwrap().to_string(),
        };
        let mut chatgpt = ChatGPT::new(client, Option::None);
        chatgpt.register_function(
            Function {
                name: "get_random_number".to_string(),
                description: "generate random number".to_string(),
                parameters: json!({
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

        let handler = ConsoleHandler {};

        loop {
            print_flush("> ")?;

            let line = read_line()?;
            if line == "/quit" {
                break;
            }

            chatgpt.chat(&line, &handler).await?;
        }

        Ok(())
    }
}

fn read_line() -> Result<String, Box<dyn Error>> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let line = line.trim_end();
    Ok(line.to_string())
}

fn print_flush(message: &str) -> Result<(), Box<dyn Error>> {
    print!("{}", message);
    io::stdout().flush()?;
    Ok(())
}
