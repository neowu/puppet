use std::env;
use std::fs::read_to_string;
use std::io::stdout;
use std::io::Write;

use anyhow::Result;
use framework::json::from_json;
use futures::StreamExt;
use openai::chat::Chat;

#[tokio::main]
async fn main() -> Result<()> {
    let path = env::args().nth(1).expect("env json path is required");
    let config: serde_json::Value = from_json(&read_to_string(path)?)?;
    let model = &config["models"]["gpt4o"];

    let mut chat = Chat::new(
        model["url"].to_string(),
        model["api_key"].to_string(),
        model["model"].to_string(),
        Option::None,
    );

    chat.add_user_message("hello".to_string(), &[])?;
    let mut stream = chat.generate().await?;
    while let Some(text) = stream.next().await {
        print!("{text}");
        stdout().flush()?;
    }

    Ok(())
}
