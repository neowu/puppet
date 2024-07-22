use std::borrow::Cow;

use base64::prelude::BASE64_STANDARD;
use base64::DecodeError;
use base64::Engine;
use log::info;
use serde::Deserialize;
use serde::Serialize;

use super::token;
use crate::util::exception::Exception;
use crate::util::http_client;
use crate::util::json;

pub struct GCloudTTS {
    pub endpoint: String,
    pub project: String,
    pub voice: String,
}

impl GCloudTTS {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, Exception> {
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

        Ok(content)
    }
}

#[derive(Debug, Serialize)]
struct SynthesizeRequest<'a> {
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
    input: Input<'a>,
    voice: Voice<'a>,
}

#[derive(Debug, Serialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: String,
    #[serde(rename = "effectsProfileId")]
    effects_profile_id: Vec<String>,
    pitch: i64,
    #[serde(rename = "speakingRate")]
    speaking_rate: i64,
}

#[derive(Debug, Serialize)]
struct Input<'a> {
    text: Cow<'a, str>,
}

#[derive(Debug, Serialize)]
struct Voice<'a> {
    #[serde(rename = "languageCode")]
    language_code: String,
    name: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
struct SynthesizeResponse {
    #[serde(rename = "audioContent")]
    audio_content: String,
}

impl From<DecodeError> for Exception {
    fn from(err: DecodeError) -> Self {
        Exception::unexpected(err)
    }
}
