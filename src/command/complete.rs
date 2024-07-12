use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tracing::info;

use crate::llm;
use crate::llm::ChatEvent;
use crate::llm::ChatListener;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Complete {
    #[arg(help = "prompt file")]
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
        let mut on_system_message = false;
        loop {
            let Some(line) = lines.next_line().await? else { break };

            if line.is_empty() {
                continue;
            }

            if line.starts_with("# system") {
                if !message.is_empty() {
                    return Err(Exception::ValidationError("system message must be at first".to_string()));
                }
                on_system_message = true;
            } else if line.starts_with("# prompt") {
                if on_system_message {
                    info!("system message: {}", message);
                    model.system_message(message);
                    message = String::new();
                    on_system_message = false;
                }
            } else if line.starts_with("# anwser") {
                break;
            } else if line.starts_with("> file: ") {
                let file = self.prompt.with_file_name(line.strip_prefix("> file: ").unwrap());
                let extension = file
                    .extension()
                    .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", file.to_string_lossy())))?
                    .to_str()
                    .unwrap();
                if extension == "txt" {
                    message.push_str(&fs::read_to_string(file).await?)
                } else {
                    info!("file: {}", file.to_string_lossy());
                    files.push(file);
                }
            } else {
                message.push_str(&line);
                message.push('\n');
            }
        }

        info!("prompt: {}", message);
        let files = files.into_iter().map(Some).collect();
        let message = model.chat(message, files).await?;

        let mut prompt = fs::OpenOptions::new().append(true).open(&self.prompt).await?;
        prompt.write_all(format!("\n# anwser ({})\n\n", self.name).as_bytes()).await?;
        prompt.write_all(message.as_bytes()).await?;

        Ok(())
    }
}
