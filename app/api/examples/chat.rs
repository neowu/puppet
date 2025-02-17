use std::env;
use std::path::Path;

use anyhow::Result;
use framework::json::load_file;
use openai::chat::Chat;
use openai::function::FunctionStore;
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
        // env::var("GCLOUD_AUTH_TOKEN").expect("require GCLOUD_AUTH_TOKEN env"),
        model["api_key"].as_str().unwrap().to_string(),
        model["model"].as_str().unwrap().to_string(),
        FunctionStore::default(),
    );

    // chat.option(ChatOption {
    //     response_format: Arc::new(ResponseFormat::json_schema(json!({
    //         "name": "anwser",
    //         "schema": {
    //             "type": "object",
    //             "properties": {
    //                 "anwser": {
    //                     "type": "string",
    //                     "description": "anwser",
    //                 }
    //             }
    //         }
    //     }))),
    //     ..Default::default()
    // });

    chat.add_user_message(
        r#"
        ```
        class User {
          firstName: string = "";
          lastName: string = "";
          username: string = "";
        }

        export default User;
        ```
        Replace the username property with an email property. Respond only with code, and with no markdown formatting.

        "#
        .to_string(),
        vec![],
    )?;

    let response = chat
        .generate(Some(
            r#"
        class User {
          firstName: string = "";
          lastName: string = "";
          email: string = "";
        }

        export default User;
        "#
            .to_string(),
        ))
        .await?;
    println!("{response}");

    // let mut stream = chat.generate_stream().await?;
    // while let Some(text) = stream.next().await {
    //     print!("{text}");
    //     stdout().flush()?;
    // }

    Ok(())
}
