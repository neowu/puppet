use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SynthesizeRequest<'a> {
    #[serde(rename = "audioConfig")]
    pub audio_config: AudioConfig,
    pub input: Input<'a>,
    pub voice: Voice<'a>,
}

#[derive(Debug, Serialize)]
pub struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    pub audio_encoding: String,
    #[serde(rename = "effectsProfileId")]
    pub effects_profile_id: Vec<String>,
    pub pitch: i64,
    #[serde(rename = "speakingRate")]
    pub speaking_rate: i64,
}

#[derive(Debug, Serialize)]
pub struct Input<'a> {
    pub text: Cow<'a, str>,
}

#[derive(Debug, Serialize)]
pub struct Voice<'a> {
    #[serde(rename = "languageCode")]
    pub language_code: String,
    pub name: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
pub struct SynthesizeResponse {
    #[serde(rename = "audioContent")]
    pub audio_content: String,
}
