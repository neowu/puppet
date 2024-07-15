use std::io::Write;
use std::mem;
use std::path::PathBuf;

use clap::Args;
use tokio::io::stdin;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tracing::info;

use crate::llm;
use crate::llm::ChatEvent;
use crate::llm::ChatListener;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: PathBuf,

    #[arg(long, help = "model name")]
    name: String,
}

struct ConsoleHandler;

impl ChatListener for ConsoleHandler {
    fn on_event(&self, event: ChatEvent) {
        match event {
            ChatEvent::Delta(data) => {
                print_flush(&data).unwrap();
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
        let config = llm::load(&self.conf).await?;
        let mut model = config.create(&self.name)?;
        model.listener(Box::new(ConsoleHandler));

        let reader = BufReader::new(stdin());
        let mut lines = reader.lines();

        let mut files: Vec<PathBuf> = vec![];
        loop {
            print_flush("> ")?;
            let Some(line) = lines.next_line().await? else { break };
            if line.starts_with("/quit") {
                break;
            }
            if line.starts_with("/file ") {
                let file = PathBuf::from(line.strip_prefix("/file ").unwrap().to_string());
                println!("added file, path={}", file.to_string_lossy());
                files.push(file);
            } else {
                let files = mem::take(&mut files).into_iter().map(Some).collect();
                model.add_user_message(line, files).await?;
                model.chat().await?;
            }
        }

        Ok(())
    }
}

fn print_flush(message: &str) -> Result<(), Exception> {
    print!("{message}");
    std::io::stdout().flush()?;
    Ok(())
}
