use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;

use clap::Args;

use crate::bot;
use crate::bot::ChatEvent;
use crate::bot::ChatHandler;

use crate::util::exception::Exception;

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
    fn on_event(&self, event: ChatEvent) {
        match event {
            ChatEvent::Delta(ref data) => {
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
    pub async fn execute(&self) -> Result<(), Exception> {
        let config = bot::load(Path::new(&self.conf))?;
        let mut bot = config.create(&self.name)?;

        let handler = ConsoleHandler {};
        loop {
            print_flush("> ")?;

            let line = read_line()?;
            if line == "/quit" {
                break;
            }
            if line.starts_with("/data ") {
                let index = line.find(',').unwrap();
                bot.data(Path::new(line[6..index].trim()), line[(index + 1)..].to_string(), &handler)
                    .await?;
            } else {
                bot.chat(line, &handler).await?;
            }
        }
        Ok(())
    }
}

fn read_line() -> Result<String, Exception> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let line = line.trim_end();
    Ok(line.to_string())
}

fn print_flush(message: &str) -> Result<(), Exception> {
    print!("{}", message);
    io::stdout().flush()?;
    Ok(())
}
