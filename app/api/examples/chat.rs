use std::env;
use std::path::Path;

use anyhow::Result;
use framework::json::load_file;
use openai::chat::Chat;
use tracing::Level;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(Level::TRACE)
        .with_line_number(true)
        .with_thread_ids(true)
        .init();

    let path = env::args().nth(1).expect("env json path is required");
    let config: serde_json::Value = load_file(Path::new(&path))?;
    let model = &config["models"]["gpt4o"];

    let mut chat = Chat::new(
        model["url"].as_str().unwrap().to_string(),
        model["api_key"].as_str().unwrap().to_string(),
        model["model"].as_str().unwrap().to_string(),
        Option::None,
    );

    chat.add_user_message("hello".to_string(), &[])?;

    let response = chat.generate().await?;
    println!("{response}");

    // let mut stream = chat.generate_stream().await?;
    // while let Some(text) = stream.next().await {
    //     print!("{text}");
    //     stdout().flush()?;
    // }

    Ok(())
}
