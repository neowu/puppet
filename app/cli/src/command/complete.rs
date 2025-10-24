use std::io::Write;
use std::io::stdout;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use ::agent::openai::session::Message;
use ::agent::openai::session::Session;
use clap::Args;
use framework::exception;
use framework::exception::Exception;
use glob::glob;
use regex::Regex;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio_stream::StreamExt;

use crate::agent;

#[derive(Args)]
pub struct Complete {
    #[arg(help = "prompt file path")]
    prompt: PathBuf,

    #[arg(long, help = "conf path")]
    conf: PathBuf,
}

impl Complete {
    pub async fn execute(&self) -> Result<(), Exception> {
        let chats = agent::load(&self.conf)?;

        let prompt = fs::OpenOptions::new().read(true).open(&self.prompt).await?;
        let reader = BufReader::new(prompt);
        let mut lines = reader.lines();

        let mut session = Session::default();
        let mut parser = Parser::new(&mut session, &self.prompt);

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                continue;
            }
            parser.process_line(&line).await?;
        }
        parser.add_message()?;
        if !matches!(parser.state, ParserState::User) {
            return Err(exception!(message = "last message must be user message"));
        }

        let chat = chats
            .get(&parser.model.unwrap_or("gpt5".to_string()))
            .ok_or_else(|| exception!(message = ""))?;

        let session = Arc::new(Mutex::new(session));
        let mut stream = chat.generate_stream(session).await?;
        let mut prompt = fs::OpenOptions::new().append(true).open(&self.prompt).await?;
        prompt.write_all("\n# assistant\n\n".as_bytes()).await?;
        while let Some(text) = stream.next().await {
            let text = text?;
            print!("{text}");
            stdout().flush()?;
            prompt.write_all(text.as_bytes()).await?;
        }
        Ok(())
    }
}

struct Parser<'a> {
    state: ParserState,
    current_message: String,
    model: Option<String>,
    session: &'a mut Session,
    current_path: &'a Path,
}

enum ParserState {
    System,
    User,
    Assistant,
}

impl<'a> Parser<'a> {
    fn new(session: &'a mut Session, current_path: &'a Path) -> Self {
        Self {
            state: ParserState::User,
            current_message: String::new(),
            model: None,
            session,
            current_path,
        }
    }

    async fn process_line(&mut self, line: &str) -> Result<(), Exception> {
        if line.starts_with("# system") {
            self.add_message()?;

            let regex = Regex::new(r#"model=([^,]+)"#)?;
            if let Some(captures) = regex.captures(line) {
                let model = captures[1].to_string();
                self.model = Some(model);
            }
            let regex = Regex::new(r#"top_p=([^,]+)"#)?;
            if let Some(captures) = regex.captures(line) {
                let top_p = captures[1].parse()?;
                self.session.top_p = Some(top_p);
            }

            self.state = ParserState::System;
        } else if line.starts_with("# user") {
            self.add_message()?;

            self.state = ParserState::User;
        } else if line.starts_with("> ![@img]") {
            self.add_message()?;

            let regex = Regex::new(r#"> ![@img]=\((.*)\)"#)?;
            if let Some(captures) = regex.captures(line) {
                let mut images = vec![];
                let pattern = self.pattern(&captures[1]).await?;
                for entry in glob(&pattern)? {
                    let path = entry?;
                    images.push(path);
                }
                self.session.add_message(Message::Images(images))?;
            }
        } else if line.starts_with("> [@file]") {
            self.add_message()?;

            let regex = Regex::new(r#"> [@file]=\((.*)\)"#)?;
            if let Some(captures) = regex.captures(line) {
                let mut files = vec![];
                let pattern = self.pattern(&captures[1]).await?;
                for entry in glob(&pattern)? {
                    let path = entry?;
                    files.push(path);
                }
                self.session.add_message(Message::Files(files))?;
            }
        } else if line.starts_with("# assistant") {
            self.add_message()?;

            self.state = ParserState::Assistant;
        } else {
            self.current_message.push_str(line);
            self.current_message.push('\n');
        }
        Ok(())
    }

    fn add_message(&mut self) -> Result<(), Exception> {
        let message = mem::take(&mut self.current_message);
        if !message.is_empty() {
            match self.state {
                ParserState::System => self.session.add_message(Message::SystemMessage(message)),
                ParserState::User => self.session.add_message(Message::UserMessage(message)),
                ParserState::Assistant => self.session.add_message(Message::AssistantMessage(message)),
            }?;
        }
        Ok(())
    }

    async fn pattern(&self, pattern: &str) -> Result<String, Exception> {
        if !pattern.starts_with('/') {
            return Ok(format!(
                "{}/{}",
                fs::canonicalize(&self.current_path)
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
