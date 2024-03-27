use std::error::Error;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;

use clap::Args;

use crate::bot;
use crate::chatgpt;
use crate::chatgpt::ChatHandler;

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

impl Chat {
    pub async fn execute(&self) -> Result<(), Box<dyn Error>> {
        let config = bot::load(Path::new(&self.conf))?;
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
