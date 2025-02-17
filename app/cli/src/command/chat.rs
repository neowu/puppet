use std::io::stdout;
use std::io::Write;
use std::mem;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use futures::StreamExt;
use tokio::io::stdin;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::io::Lines;
use tokio::io::Stdin;
use tracing::info;

use crate::agent;

#[derive(Args)]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,

    #[arg(long, help = "agent name", default_value = "chat")]
    agent: String,
}

impl Chat {
    pub async fn execute(&self) -> Result<()> {
        let registry = agent::load_function_registry()?;
        let config = agent::load(self.conf.as_deref())?;
        let mut agent = config.create(&self.agent, &registry)?;

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

            let input = read_input(&mut lines).await?;

            if input.starts_with("/quit") {
                break;
            }
            if let Some(file) = input.strip_prefix("/file ") {
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
                agent.chat.add_user_message(input, files)?;

                let mut stream = agent.chat.generate_stream().await?;
                while let Some(text) = stream.next().await {
                    print!("{text}");
                    stdout().flush()?;
                }
                let usage = agent.chat.usage();
                info!(
                    "usage, prompt_tokens={}, completion_tokens={}",
                    usage.prompt_tokens, usage.completion_tokens
                );
            }
        }

        Ok(())
    }
}

async fn read_input(lines: &mut Lines<BufReader<Stdin>>) -> Result<String> {
    let mut input = String::new();
    let mut is_multiline = false;
    while let Some(line) = lines.next_line().await? {
        if line.contains("```") {
            is_multiline = !is_multiline;
        }
        input.push_str(&line);
        if !is_multiline {
            break;
        }
        input.push('\n');
    }
    Ok(input)
}
