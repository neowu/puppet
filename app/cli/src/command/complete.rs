use std::io::stdout;
use std::io::Write;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use ::agent::agent::Agent;
use anyhow::anyhow;
use anyhow::Result;
use clap::Args;
use framework::fs::path::PathExt;
use futures::StreamExt;
use glob::glob;
use openai::chat::ChatOption;
use regex::Regex;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tracing::info;

use crate::agent;

#[derive(Args)]
pub struct Complete {
    #[arg(help = "prompt file path")]
    prompt: PathBuf,

    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,

    #[arg(long, help = "agent name", default_value = "chat")]
    agent: String,
}

enum ParserState {
    System,
    User,
    Assistant,
}

impl Complete {
    pub async fn execute(&self) -> Result<()> {
        let registry = agent::load_function_registry()?;
        let config = agent::load(self.conf.as_deref())?;
        let mut agent = config.create(&self.agent, &registry)?;

        let prompt = fs::OpenOptions::new().read(true).open(&self.prompt).await?;
        let reader = BufReader::new(prompt);
        let mut lines = reader.lines();

        let mut files: Vec<PathBuf> = vec![];
        let mut message = String::new();
        let mut state = ParserState::User;

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                continue;
            }
            state = self
                .process_line(&state, &line, &mut agent, &mut message, &mut files)
                .await?
                .unwrap_or(state);
        }
        add_message(&mut agent, &state, message, files).await?;

        if !matches!(state, ParserState::User) {
            return Err(anyhow!("last message must be user message".to_string()));
        }

        let mut stream = agent.chat.generate_stream().await?;
        let mut prompt = fs::OpenOptions::new().append(true).open(&self.prompt).await?;
        prompt
            .write_all(format!("\n# assistant ({})\n\n", self.agent).as_bytes())
            .await?;
        while let Some(text) = stream.next().await {
            print!("{text}");
            stdout().flush()?;
            prompt.write_all(text.as_bytes()).await?;
        }
        let usage = agent.chat.usage();
        info!(
            "usage, prompt_tokens={}, completion_tokens={}",
            usage.prompt_tokens, usage.completion_tokens
        );
        Ok(())
    }

    async fn process_line(
        &self,
        state: &ParserState,
        line: &str,
        agent: &mut Agent,
        message: &mut String,
        files: &mut Vec<PathBuf>,
    ) -> Result<Option<ParserState>> {
        if line.starts_with("# system") {
            if !message.is_empty() {
                return Err(anyhow!("system message must be at first"));
            }
            if let Some(option) = parse_option(line)? {
                info!("option: {:?}", option);
                agent.chat.option(option);
            }
            return Ok(Some(ParserState::System));
        } else if line.starts_with("# user") {
            add_message(agent, state, mem::take(message), mem::take(files)).await?;
            return Ok(Some(ParserState::User));
        } else if line.starts_with("# assistant") {
            add_message(agent, state, mem::take(message), vec![]).await?;
            return Ok(Some(ParserState::Assistant));
        } else if let Some(file) = line.strip_prefix("> file: ") {
            if !matches!(state, ParserState::User) {
                return Err(anyhow!("file can only be included in user message, line={line}"));
            }

            let pattern = self.pattern(file).await?;
            info!("include files, pattern: {pattern}");
            for entry in glob(&pattern)? {
                let path = entry?;
                let extension = path.file_extension()?;
                match extension {
                    "txt" | "md" => {
                        message.push_str(&fs::read_to_string(path).await?);
                    }
                    "java" | "rs" => {
                        message.push_str(&format!(
                            "```{} (path: {})\n",
                            language(extension)?,
                            path.to_string_lossy()
                        ));
                        message.push_str(&fs::read_to_string(path).await?);
                        message.push_str("```\n");
                    }
                    _ => {
                        files.push(path);
                    }
                }
            }
        } else {
            message.push_str(line);
            message.push('\n');
        }
        Ok(None)
    }

    async fn pattern(&self, pattern: &str) -> Result<String> {
        if !pattern.starts_with('/') {
            return Ok(format!(
                "{}/{}",
                fs::canonicalize(&self.prompt)
                    .await?
                    .parent()
                    .unwrap()
                    .to_string_lossy(),
                pattern
            ));
        }
        Ok(pattern.to_string())
    }
}

async fn add_message(agent: &mut Agent, state: &ParserState, message: String, files: Vec<PathBuf>) -> Result<()> {
    match state {
        ParserState::System => {
            info!("set system message: {}", message);
            agent.chat.system_message(message);
        }
        ParserState::User => {
            info!("add user message: {}", message);
            let files: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
            for file in files.iter() {
                info!("add user data: {}", file.to_string_lossy());
            }
            agent.chat.add_user_message(message, files)?;
        }
        ParserState::Assistant => {
            info!("add assistent message: {}", message);
            agent.chat.add_assistant_message(message);
        }
    }
    Ok(())
}

fn parse_option(line: &str) -> Result<Option<ChatOption>> {
    let regex = Regex::new(r".*temperature=(\d+\.\d+).*")?;
    if let Some(capture) = regex.captures(line) {
        let temperature = f32::from_str(&capture[1])?;
        Ok(Some(ChatOption {
            temperature,
            response_format: None,
        }))
    } else {
        Ok(None)
    }
}

fn language(extenstion: &str) -> Result<&'static str> {
    match extenstion {
        "java" => Ok("java"),
        "rs" => Ok("rust"),
        _ => Err(anyhow!("unsupported extension, ext={}", extenstion)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_option() {
        let option = super::parse_option("# system, temperature=2.0, top_p=0.95");
        assert_eq!(option.unwrap().unwrap().temperature, 2.0);
    }
}
