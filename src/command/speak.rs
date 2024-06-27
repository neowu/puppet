use std::env::temp_dir;
use std::path::PathBuf;

use clap::arg;
use clap::Args;
use tokio::fs;
use tokio::io::stdin;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

use crate::tts;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct Speak {
    #[arg(long, help = "conf path")]
    conf: PathBuf,

    #[arg(long, help = "model name")]
    name: String,

    #[arg(long, help = "text")]
    text: Option<String>,

    #[arg(long, help = "stdin", default_value_t = false)]
    stdin: bool,
}

impl Speak {
    pub async fn execute(&self) -> Result<(), Exception> {
        if !self.stdin && self.text.is_none() {
            return Err(Exception::ValidationError("must specify --stdin or --text".to_string()));
        }

        let speech = tts::load(&self.conf, &self.name).await?;

        let mut buffer = String::new();
        let text = if self.stdin {
            stdin().read_to_string(&mut buffer).await?;
            info!("text={}", buffer);
            &buffer
        } else {
            self.text.as_ref().unwrap()
        };

        let audio = speech.synthesize(text).await?;

        play(audio).await?;

        Ok(())
    }
}

async fn play(audio: Vec<u8>) -> Result<(), Exception> {
    let temp_file = temp_dir().join(format!("{}.wav", Uuid::new_v4()));
    fs::write(&temp_file, &audio).await?;
    info!("play audio file, file={}", temp_file.to_string_lossy());
    let mut command = Command::new("afplay").args([temp_file.to_string_lossy().to_string()]).spawn()?;
    let _ = command.wait().await;
    fs::remove_file(temp_file).await?;
    Ok(())
}
