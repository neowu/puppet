use std::env;

pub mod gemini;
mod gemini_api;
pub mod tts;

pub fn token() -> String {
    env::var("GCLOUD_AUTH_TOKEN").expect("please set GCLOUD_AUTH_TOKEN env")
}
