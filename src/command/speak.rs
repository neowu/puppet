use std::path::PathBuf;

use clap::arg;
use clap::Args;
use tokio::io::stdin;
use tokio::io::AsyncReadExt;

use crate::gcloud::synthesize;
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

        let config = tts::load(&self.conf).await?;
        let model = config
            .models
            .get(&self.name)
            .ok_or_else(|| Exception::ValidationError(format!("can not find model, name={}", self.name)))?;

        let mut buffer = String::new();
        let text = if self.stdin {
            stdin().read_to_string(&mut buffer).await?;
            &buffer
        } else {
            self.text.as_ref().unwrap()
        };

        let gcloud = synthesize::GCloud {
            endpoint: model.endpoint.to_string(),
            project: model.params.get("project").unwrap().to_string(),
            voice: model.params.get("voice").unwrap().to_string(),
        };

        gcloud.synthesize(text).await?;

        Ok(())
    }
}
