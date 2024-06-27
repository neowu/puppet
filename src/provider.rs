use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub enum Provider {
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "gcloud")]
    GCloud,
}
