use std::io::stdout;
use std::io::Write;
use std::mem;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use futures::StreamExt;
use regex::Regex;
use tokio::io::stdin;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::io::Lines;
use tokio::io::Stdin;

use crate::agent;

#[derive(Args)]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,
}

impl Chat {
    pub async fn execute(&self) -> Result<()> {
        let mut agent = agent::load(self.conf.as_deref())?;

        println!(
            r"---
# Welcome to Puppet Chat
---
# Usage Instructions:

- /quit to quit the application.

- /file {{file}} to add a file.

- @agent {{message}} to talk to specific agent.
---"
        );

        let reader = BufReader::new(stdin());
        let mut lines = reader.lines();
        let mut files: Vec<PathBuf> = vec![];

        let regex = Regex::new(r"@(\w+) (.*)")?;
        loop {
            print!("> ");
            stdout().flush()?;

            let mut input = read_input(&mut lines).await?;

            if input.starts_with("/quit") {
                break;
            }

            let mut agent_name: Option<String> = None;
            if let Some(capture) = regex.captures(&input) {
                agent_name = Some(capture[1].to_string());
                input = capture[2].to_string();
            }

            if let Some(file) = input.strip_prefix("/file ") {
                let file = PathBuf::from(file);
                if !file.exists() {
                    println!("file not exists, path: {}", file.to_string_lossy());
                } else {
                    println!("added file, path: {}", file.to_string_lossy());
                    files.push(file);
                }
                continue;
            }

            let files = mem::take(&mut files);
            let files: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
            agent.add_user_message(input, files)?;

            let mut stream = agent.chat(agent_name).await?;
            while let Some(text) = stream.next().await {
                print!("{text}");
                stdout().flush()?;
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
