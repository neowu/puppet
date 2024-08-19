use anyhow::anyhow;
use anyhow::Result;

use crate::util::http_client::HTTP_CLIENT;

pub struct AzureTTS {
    pub endpoint: String,
    pub resource: String,
    pub api_key: String,
    pub voice: String,
}

impl AzureTTS {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let body = format!(
            r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xmlns:mstts="https://www.w3.org/2001/mstts" xml:lang="en-US">
                <voice name="{}"><mstts:express-as style="narration-relaxed"><![CDATA[
            {text}
            ]]></mstts:express-as></voice></speak>"#,
            self.voice
        );

        let response = HTTP_CLIENT
            .post(&self.endpoint)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("User-Agent", &self.resource)
            .header("X-Microsoft-OutputFormat", "riff-44100hz-16bit-mono-pcm")
            .header("Content-Type", "application/ssml+xml")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        if status != 200 {
            let response_text = response.text().await?;
            return Err(anyhow!("failed to call azure api, status={status}, response={response_text}"));
        }

        Ok(response.bytes().await?.to_vec())
    }
}
