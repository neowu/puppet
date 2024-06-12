use std::io;
use std::io::Write;
use std::path::Path;

use clap::Args;
use tokio::io::stdin;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tracing::info;

use crate::bot;
use crate::bot::ChatEvent;
use crate::bot::ChatHandler;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: String,

    #[arg(long, help = "bot name")]
    name: String,
}

struct ConsoleHandler;

impl ChatHandler for ConsoleHandler {
    fn on_event(&self, event: ChatEvent) {
        match event {
            ChatEvent::Delta(data) => {
                print_flush(&data).unwrap();
            }
            ChatEvent::Error(error) => {
                println!("\nError: {error}");
            }
            ChatEvent::End(usage) => {
                println!();
                info!(
                    "usage, request_tokens={}, response_tokens={}",
                    usage.request_tokens, usage.response_tokens
                );
            }
        }
    }
}

impl Chat {
    pub async fn execute(&self) -> Result<(), Exception> {
        let config = bot::load(Path::new(&self.conf)).await?;
        let mut bot = config.create(&self.name)?;
        let handler = ConsoleHandler {};

        let reader = BufReader::new(stdin());
        let mut lines = reader.lines();

        loop {
            print_flush("> ")?;
            let Some(line) = lines.next_line().await? else { break };
            if line.starts_with("/quit") {
                break;
            }
            if line.starts_with("/file ") {
                bot.file(Path::new(line.strip_prefix("/file ").unwrap()))?;
            } else {
                bot.chat(line, &handler).await?;
            }
        }

        Ok(())
    }
}

fn print_flush(message: &str) -> Result<(), Exception> {
    print!("{message}");
    io::stdout().flush()?;
    Ok(())
}
