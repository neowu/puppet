use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub enum Provider {
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "gcloud")]
    GCloud,
}
