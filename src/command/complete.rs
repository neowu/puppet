use std::io::Write;
use std::mem;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Args;
use regex::Regex;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tracing::info;

use crate::llm;
use crate::llm::ChatEvent;
use crate::llm::ChatListener;
use crate::llm::ChatOption;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Complete {
    #[arg(help = "prompt file path")]
    prompt: PathBuf,

    #[arg(long, help = "conf path")]
    conf: PathBuf,

    #[arg(long, help = "model name")]
    name: String,
}

struct Listener;

impl ChatListener for Listener {
    fn on_event(&self, event: ChatEvent) {
        match event {
            ChatEvent::Delta(data) => {
                print!("{data}");
                let _ = std::io::stdout().flush();
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

enum ParserState {
    System,
    User,
    Assistant,
}

impl Complete {
    pub async fn execute(&self) -> Result<(), Exception> {
        let config = llm::load(&self.conf).await?;
        let mut model = config.create(&self.name)?;
        model.listener(Box::new(Listener));

        let prompt = fs::OpenOptions::new().read(true).open(&self.prompt).await?;
        let reader = BufReader::new(prompt);
        let mut lines = reader.lines();

        let mut files: Vec<PathBuf> = vec![];
        let mut message = String::new();
        let mut state = ParserState::User;
        loop {
            let Some(line) = lines.next_line().await? else { break };

            if line.is_empty() {
                continue;
            }

            if line.starts_with("# system") {
                if !message.is_empty() {
                    return Err(Exception::ValidationError("system message must be at first".to_string()));
                }
                state = ParserState::System;
                if let Some(option) = parse_option(&line) {
                    info!("option: {:?}", option);
                    model.option(option);
                }
            } else if line.starts_with("# user") {
                add_message(&mut model, &state, mem::take(&mut message), mem::take(&mut files)).await?;
                state = ParserState::User;
            } else if line.starts_with("# assistant") {
                add_message(&mut model, &state, mem::take(&mut message), mem::take(&mut files)).await?;
                state = ParserState::Assistant;
            } else if line.starts_with("> file: ") {
                let file = self.prompt.with_file_name(line.strip_prefix("> file: ").unwrap());
                let extension = file
                    .extension()
                    .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", file.to_string_lossy())))?
                    .to_str()
                    .unwrap();
                if extension == "txt" {
                    message.push_str(&format!("> start of file: {}\n", &file.to_string_lossy()));
                    message.push_str(&fs::read_to_string(&file).await?);
                    message.push_str(&format!("> end of file: {}\n", &file.to_string_lossy()));
                } else {
                    info!("file: {}", file.to_string_lossy());
                    files.push(file);
                }
            } else {
                message.push_str(&line);
                message.push('\n');
            }
        }

        add_message(&mut model, &state, message, files).await?;
        let message = model.chat().await?;

        let mut prompt = fs::OpenOptions::new().append(true).open(&self.prompt).await?;
        prompt.write_all(format!("\n# assistant ({})\n\n", self.name).as_bytes()).await?;
        prompt.write_all(message.as_bytes()).await?;

        Ok(())
    }
}

async fn add_message(model: &mut llm::Model, state: &ParserState, message: String, files: Vec<PathBuf>) -> Result<(), Exception> {
    match state {
        ParserState::System => {
            info!("system message: {}", message);
            model.system_message(message);
        }
        ParserState::User => {
            info!("user message: {}", message);
            model.add_user_message(message, files.into_iter().map(Some).collect()).await?;
        }
        ParserState::Assistant => {
            if !files.is_empty() {
                return Err(Exception::ValidationError(format!(
                    "cannot include file in assistant message, files={:?}",
                    files
                )));
            }
            info!("assistent message: {}", message);
            model.add_assistant_message(message);
        }
    }
    Ok(())
}

fn parse_option(line: &str) -> Option<ChatOption> {
    let regex = Regex::new(r".*temperature=(\d+\.\d+).*").unwrap();
    if let Some(capture) = regex.captures(line) {
        let temperature = f32::from_str(&capture[1]).unwrap();
        Some(ChatOption { temperature })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_option() {
        let option = super::parse_option("# system, temperature=2.0, top_p=0.95");
        assert_eq!(option.unwrap().temperature, 2.0);
    }
}
