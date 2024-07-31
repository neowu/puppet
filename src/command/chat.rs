use std::io::stdout;
use std::io::Write;
use std::mem;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use futures::StreamExt;
use log::info;
use tokio::io::stdin;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;

use crate::llm;

#[derive(Args)]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,

    #[arg(long, help = "model name", default_value = "gpt4o")]
    model: String,
}

impl Chat {
    pub async fn execute(&self) -> Result<()> {
        let config = llm::load(self.conf.as_deref())?;
        let mut model = config.create(&self.model)?;

        println!(
            r"---
# Welcome to Puppet Chat
---
# Usage Instructions:

- Type /quit to quit the application.

- Type /file {{file}} to add a file.
---"
        );
        let reader = BufReader::new(stdin());
        let mut lines = reader.lines();
        let mut files: Vec<PathBuf> = vec![];
        loop {
            print!("> ");
            stdout().flush()?;
            let Some(line) = lines.next_line().await? else {
                break;
            };
            if line.starts_with("/quit") {
                break;
            }
            if let Some(file) = line.strip_prefix("/file ") {
                let file = PathBuf::from(file);
                if !file.exists() {
                    println!("file not exists, path: {}", file.to_string_lossy());
                } else {
                    println!("added file, path: {}", file.to_string_lossy());
                    files.push(file);
                }
            } else {
                let files = mem::take(&mut files);
                let files: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
                model.add_user_message(line, &files)?;

                let mut stream = model.generate().await?;
                while let Some(text) = stream.next().await {
                    print!("{text}");
                    stdout().flush()?;
                }
                let usage = model.usage();
                info!(
                    "usage, prompt_tokens={}, completion_tokens={}",
                    usage.prompt_tokens, usage.completion_tokens
                );
            }
        }

        Ok(())
    }
}
