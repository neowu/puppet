use std::mem;
use std::path::Path;
use std::path::PathBuf;

use clap::Args;
use tokio::io::stdin;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;

use crate::llm;
use crate::util::console;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Chat {
    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,

    #[arg(long, help = "model name", default_value = "gpt4o")]
    model: String,
}

impl Chat {
    pub async fn execute(&self) -> Result<(), Exception> {
        let config = llm::load(self.conf.as_deref()).await?;
        let mut model = config.create(&self.model)?;

        let welcome_text = r#"
---
# Welcome to Puppet Chat
---
# Usage Instructions:

- Type /quit to quit the application.

- Type /file {file} to add a file.
---
"#;
        console::print(welcome_text).await?;
        let reader = BufReader::new(stdin());
        let mut lines = reader.lines();
        let mut files: Vec<PathBuf> = vec![];
        loop {
            console::print("> ").await?;
            let Some(line) = lines.next_line().await? else {
                break;
            };
            if line.starts_with("/quit") {
                break;
            }
            if let Some(file) = line.strip_prefix("/file ") {
                let file = PathBuf::from(file);
                if !file.exists() {
                    console::print(&format!("file not exists, path: {}\n", file.to_string_lossy())).await?;
                } else {
                    console::print(&format!("added file, path: {}\n", file.to_string_lossy())).await?;
                    files.push(file);
                }
            } else {
                let files = mem::take(&mut files);
                let files: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
                model.add_user_message(line, &files).await?;

                model.chat().await?;
            }
        }

        Ok(())
    }
}
