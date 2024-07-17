use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Args;
use glob::glob;
use glob::GlobError;
use glob::PatternError;
use regex::Regex;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tracing::info;

use crate::llm;
use crate::llm::ChatOption;
use crate::llm::ConsolePrinter;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Complete {
    #[arg(help = "prompt file path")]
    prompt: PathBuf,

    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,

    #[arg(long, help = "model name", default_value = "gpt4o")]
    model: String,
}

enum ParserState {
    System,
    User,
    Assistant,
}

impl Complete {
    pub async fn execute(&self) -> Result<(), Exception> {
        let config = llm::load(self.conf.as_deref()).await?;
        let mut model = config.create(&self.model, Some(ConsolePrinter))?;

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
                .process_line(&state, &line, &mut model, &mut message, &mut files)
                .await?
                .unwrap_or(state);
        }
        add_message(&mut model, &state, message, files).await?;

        if !matches!(state, ParserState::User) {
            return Err(Exception::ValidationError("last message must be user message".to_string()));
        }

        let assistant_message = model.chat().await?;
        let mut prompt = fs::OpenOptions::new().append(true).open(&self.prompt).await?;
        prompt.write_all(format!("\n# assistant ({})\n\n", self.model).as_bytes()).await?;
        prompt.write_all(assistant_message.as_bytes()).await?;
        prompt.write_all(b"\n").await?;
        Ok(())
    }

    async fn process_line(
        &self,
        state: &ParserState,
        line: &str,
        model: &mut llm::Model<ConsolePrinter>,
        message: &mut String,
        files: &mut Vec<PathBuf>,
    ) -> Result<Option<ParserState>, Exception> {
        if line.starts_with("# system") {
            if !message.is_empty() {
                return Err(Exception::ValidationError("system message must be at first".to_string()));
            }
            if let Some(option) = parse_option(line) {
                info!("option: {:?}", option);
                model.option(option);
            }
            return Ok(Some(ParserState::System));
        } else if line.starts_with("# user") {
            add_message(model, state, mem::take(message), mem::take(files)).await?;
            return Ok(Some(ParserState::User));
        } else if line.starts_with("# assistant") {
            add_message(model, state, mem::take(message), vec![]).await?;
            return Ok(Some(ParserState::Assistant));
        } else if line.starts_with("> file: ") {
            if !matches!(state, ParserState::User) {
                return Err(Exception::ValidationError(format!(
                    "file can only be included in user message, line={line}"
                )));
            }

            let pattern = self.pattern(line.strip_prefix("> file: ").unwrap()).await?;
            info!("include files, pattern: {pattern}");
            for entry in glob(&pattern)? {
                let entry = entry?;
                let extension = extension(&entry)?;
                match extension {
                    "txt" | "md" => {
                        message.push_str(&fs::read_to_string(entry).await?);
                    }
                    "java" | "rs" => {
                        message.push_str(&format!("```{} (path: {})\n", language(extension)?, entry.to_string_lossy()));
                        message.push_str(&fs::read_to_string(entry).await?);
                        message.push_str("```\n");
                    }
                    _ => {
                        files.push(entry);
                    }
                }
            }
        } else {
            message.push_str(line);
            message.push('\n');
        }
        Ok(None)
    }

    async fn pattern(&self, pattern: &str) -> Result<String, Exception> {
        if !pattern.starts_with('/') {
            return Ok(format!(
                "{}/{}",
                fs::canonicalize(&self.prompt).await?.parent().unwrap().to_string_lossy(),
                pattern
            ));
        }
        Ok(pattern.to_string())
    }
}

fn extension(file: &Path) -> Result<&str, Exception> {
    let extension = file
        .extension()
        .ok_or_else(|| Exception::ValidationError(format!("file must have a valid extension, path={}", file.to_string_lossy())))?
        .to_str()
        .unwrap();
    Ok(extension)
}

async fn add_message(model: &mut llm::Model<ConsolePrinter>, state: &ParserState, message: String, files: Vec<PathBuf>) -> Result<(), Exception> {
    match state {
        ParserState::System => {
            info!("set system message: {}", message);
            model.system_message(message);
        }
        ParserState::User => {
            info!("add user message: {}", message);
            let files: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
            for file in files.iter() {
                info!("add user data: {}", file.to_string_lossy());
            }
            model.add_user_message(message, &files).await?;
        }
        ParserState::Assistant => {
            info!("add assistent message: {}", message);
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

fn language(extenstion: &str) -> Result<&'static str, Exception> {
    match extenstion {
        "java" => Ok("java"),
        "rs" => Ok("rust"),
        _ => Err(Exception::ValidationError(format!("unsupported extension, ext={}", extenstion))),
    }
}

impl From<PatternError> for Exception {
    fn from(err: PatternError) -> Self {
        Exception::unexpected(err)
    }
}

impl From<GlobError> for Exception {
    fn from(err: GlobError) -> Self {
        Exception::unexpected(err)
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
