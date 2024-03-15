use std::error::Error;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;

use clap::Args;

use crate::config;
use crate::openai;
use crate::openai::chat_completion::ChatRequestMessage;
use crate::openai::chat_completion::ChatResponse;
use crate::openai::chat_completion::Role;
use crate::util::json;
use futures::stream::StreamExt;
use reqwest_eventsource::Event;

#[derive(Args)]
#[command(about = "chat")]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: String,
}

impl Chat {
    pub async fn execute(&self) -> Result<(), Box<dyn Error>> {
        let config = config::load(Path::new(&self.conf))?;
        let stdin = io::stdin();

        let mut request = openai::chat_completion::ChatRequest {
            messages: vec![],
            temperature: 0.8,
            top_p: 0.8,
            stream: true,
            stop: None,
            max_tokens: 800,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
        };
        loop {
            print!("> ");
            io::stdout().flush()?;
            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;
            let line = line.trim_end();
            if line == "/quit" {
                break;
            }

            request.messages.push(ChatRequestMessage {
                role: Role::User,
                content: Some(line.to_string()),
                name: None,
            });

            let bot = config.bots.get("azure").unwrap();
            let client = openai::Client {
                endpoint: &bot.endpoint,
                api_key: &bot.api_key,
                model: bot.params.get("model").unwrap(),
            };
            let mut source = client.post_sse(&request).await?;

            let mut assistant_message = String::new();
            while let Some(event) = source.next().await {
                match event {
                    Ok(Event::Open) => {}
                    Ok(Event::Message(message)) => {
                        let data = message.data;

                        if data == "[DONE]" {
                            source.close();
                            println!();
                            continue;
                        }

                        let response: ChatResponse = json::from_json(&data)?;
                        if response.choices.is_empty() {
                            continue;
                        }
                        let content = response.choices.first().unwrap().delta.as_ref().unwrap();
                        if let Some(value) = content.content.as_ref() {
                            assistant_message.push_str(value);
                            print!("{}", value);
                            io::stdout().flush()?;
                        }
                    }
                    Err(err) => {
                        println!("Error: {}", err);
                        source.close();
                    }
                }
            }

            request.messages.push(ChatRequestMessage {
                role: Role::Assistant,
                content: Some(assistant_message.to_string()),
                name: None,
            });
        }

        Ok(())
    }
}
