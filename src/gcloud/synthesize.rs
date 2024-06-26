use std::borrow::Cow;
use std::env::temp_dir;

use base64::prelude::BASE64_STANDARD;
use base64::DecodeError;
use base64::Engine;
use tokio::fs;
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

use super::token;
use crate::gcloud::synthesize_api::AudioConfig;
use crate::gcloud::synthesize_api::Input;
use crate::gcloud::synthesize_api::SynthesizeRequest;
use crate::gcloud::synthesize_api::SynthesizeResponse;
use crate::gcloud::synthesize_api::Voice;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct GCloud {
    pub endpoint: String,
    pub project: String,
    pub voice: String,
}

impl GCloud {
    pub async fn synthesize(&self, text: &str) -> Result<(), Exception> {
        info!("call gcloud synthesize api, endpoint={}", self.endpoint);
        let request = SynthesizeRequest {
            audio_config: AudioConfig {
                audio_encoding: "LINEAR16".to_string(),
                effects_profile_id: vec!["headphone-class-device".to_string()],
                pitch: 0,
                speaking_rate: 1,
            },
            input: Input { text: Cow::from(text) },
            voice: Voice {
                language_code: "en-US".to_string(),
                name: Cow::from(&self.voice),
            },
        };

        let body = json::to_json(&request)?;
        let response = http_client::http_client()
            .post(&self.endpoint)
            .bearer_auth(token())
            .header("x-goog-user-project", &self.project)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        if status != 200 {
            let response_text = response.text().await?;
            return Err(Exception::ExternalError(format!(
                "failed to call gcloud api, status={status}, response={response_text}"
            )));
        }

        let response_body = response.text_with_charset("utf-8").await?;
        let response: SynthesizeResponse = json::from_json(&response_body)?;
        let content = BASE64_STANDARD.decode(response.audio_content)?;

        play(content).await?;

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

impl From<DecodeError> for Exception {
    fn from(err: DecodeError) -> Self {
        Exception::unexpected(err)
    }
}
