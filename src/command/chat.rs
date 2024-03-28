use std::error::Error;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;

use clap::Args;

use crate::bot;
use crate::bot::handler::ChatEvent;
use crate::bot::handler::ChatHandler;

#[derive(Args)]
#[command(about = "chat")]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: String,

    #[arg(long, help = "bot name")]
    name: String,
}

struct ConsoleHandler;

impl ChatHandler for ConsoleHandler {
    fn on_event(&self, event: &ChatEvent) {
        match event {
            ChatEvent::Delta(data) => {
                print_flush(data).unwrap();
            }
            ChatEvent::Error(error) => {
                println!("Error: {}", error);
            }
            ChatEvent::End => {
                println!();
            }
        }
    }
}

impl Chat {
    pub async fn execute(&self) -> Result<(), Box<dyn Error>> {
        let config = bot::load(Path::new(&self.conf))?;

        if self.name == "gpt" {
            call_chatgpt(config).await?;
        } else {
            call_vertex(config).await?;
        }

        Ok(())
    }
}

async fn call_vertex(config: bot::config::Config) -> Result<(), Box<dyn Error>> {
    let mut vertex = config.create_vertex("gemini");
    let handler = ConsoleHandler {};
    loop {
        print_flush("> ")?;

        let line = read_line()?;
        if line == "/quit" {
            break;
        }

        vertex.chat(&line, &handler).await?;
    }
    Ok(())
}

async fn call_chatgpt(config: bot::config::Config) -> Result<(), Box<dyn Error>> {
    let mut chatgpt = config.create_chatgpt("gpt4");
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
