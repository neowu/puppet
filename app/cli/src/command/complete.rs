use std::io::stdout;
use std::io::Write;
use std::mem;
use std::path::Path;
use std::path::PathBuf;

use ::agent::agent::Agent;
use anyhow::anyhow;
use anyhow::Result;
use clap::Args;
use framework::fs::path::PathExt;
use futures::StreamExt;
use glob::glob;
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
}

enum ParserState {
    User,
    Assistant,
}

impl Complete {
    pub async fn execute(&self) -> Result<()> {
        let mut agent = agent::load(self.conf.as_deref())?;

        let prompt = fs::OpenOptions::new().read(true).open(&self.prompt).await?;
        let reader = BufReader::new(prompt);
        let mut lines = reader.lines();

        let mut files: Vec<PathBuf> = vec![];
        let mut message = String::new();
        let mut agent_name: Option<String> = None;
        let mut state = ParserState::User;

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                continue;
            }
            state = self
                .process_line(&state, &line, &mut agent, &mut message, &mut files, &mut agent_name)
                .await?
                .unwrap_or(state);
        }
        add_message(&mut agent, &state, message, files).await?;

        if !matches!(state, ParserState::User) {
            return Err(anyhow!("last message must be user message".to_string()));
        }

        let mut stream = agent.chat(None).await?;
        let mut prompt = fs::OpenOptions::new().append(true).open(&self.prompt).await?;
        prompt
            .write_all(format!("\n# assistant @{}\n\n", agent_name.unwrap_or("main".to_string())).as_bytes())
            .await?;
        while let Some(text) = stream.next().await {
            print!("{text}");
            stdout().flush()?;
            prompt.write_all(text.as_bytes()).await?;
        }
        Ok(())
    }

    async fn process_line(
        &self,
        state: &ParserState,
        line: &str,
        agent: &mut Agent,
        message: &mut String,
        files: &mut Vec<PathBuf>,
        agent_name: &mut Option<String>,
    ) -> Result<Option<ParserState>> {
        if line.starts_with("# user") {
            let regex = Regex::new(r"@(\w+)")?;
            if let Some(captures) = regex.captures(line) {
                *agent_name = Some(captures[1].to_string());
            }
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
        ParserState::User => {
            info!("add user message: {}", message);
            let files: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
            for file in files.iter() {
                info!("add user data: {}", file.to_string_lossy());
            }
            agent.add_user_message(message, files)?;
        }
        ParserState::Assistant => {
            info!("add assistent message: {}", message);
            agent.add_assistant_message(message);
        }
    }
    Ok(())
}

fn language(extenstion: &str) -> Result<&'static str> {
    match extenstion {
        "java" => Ok("java"),
        "rs" => Ok("rust"),
        _ => Err(anyhow!("unsupported extension, ext={}", extenstion)),
    }
}
