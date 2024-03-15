use std::error::Error;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;

use clap::Args;

use crate::config;
use crate::openai;
use crate::openai::chat_completion::ChatRequest;
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

        let mut request = ChatRequest::new();
        loop {
            print_flush("> ")?;

            let line = read_line()?;
            if line == "/quit" {
                break;
            }

            request.messages.push(ChatRequestMessage::new(Role::User, &line));

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
                            break;
                        }

                        let response: ChatResponse = json::from_json(&data)?;
                        if response.choices.is_empty() {
                            continue;
                        }
                        let content = response.choices.first().unwrap().delta.as_ref().unwrap();
                        if let Some(value) = content.content.as_ref() {
                            assistant_message.push_str(value);
                            print_flush(value)?;
                        }
                    }
                    Err(err) => {
                        println!("Error: {}", err);
                        source.close();
                    }
                }
            }

            request.messages.push(ChatRequestMessage::new(Role::Assistant, &assistant_message));
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
