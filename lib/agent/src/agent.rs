use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use framework::exception;
use framework::exception::Exception;
use framework::fs::path::PathExt;
use framework::json;
use futures::Stream;
use openai::chat::Chat;
use openai::chat_api::ChatRequestMessage;
use openai::chat_api::Role;
use serde::Deserialize;
use tracing::info;

use crate::function::FunctionRegistry;

#[derive(Deserialize, Debug)]
struct Config {
    models: HashMap<String, ModelConfig>,
    agents: HashMap<String, AgentConfig>,
}

#[derive(Deserialize, Debug)]
struct ModelConfig {
    url: String,
    api_key: String,
    model: String,
}

#[derive(Deserialize, Debug)]
struct AgentConfig {
    model: String,
    system_message: Option<String>,
    top_p: Option<f32>,
    temperature: Option<f32>,
    functions: Option<Vec<String>>,
}

pub struct Agent {
    chats: HashMap<String, Chat>,
    messages: Arc<Mutex<Vec<ChatRequestMessage>>>,
}

impl Agent {
    pub fn load(path: &Path, registry: &FunctionRegistry) -> Result<Agent, Exception> {
        info!("load config, path={}", path.to_string_lossy());
        let content = fs::read_to_string(path)?;
        let config: Config = json::from_json(&content)?;

        let mut agent = Agent {
            chats: HashMap::new(),
            messages: Arc::new(Mutex::new(vec![])),
        };

        let mut found_main: bool = false;

        for (name, agent_config) in config.agents {
            if name == "main" {
                found_main = true;
            }
            let model_config = config
                .models
                .get(&agent_config.model)
                .ok_or_else(|| exception!(message = format!("can not find model, name={}", agent_config.model)))?;
            info!("create chat, name={name}");
            let chat = create_chat(&agent_config, model_config, registry)?;
            agent.chats.insert(name, chat);
        }

        if !found_main {
            return Err(exception!(message = "main agent is not found"));
        }

        Ok(agent)
    }

    pub async fn chat(&self, agent: Option<String>) -> Result<impl Stream<Item = String>, Exception> {
        let agent = agent.unwrap_or("main".to_string());
        let chat = self
            .chats
            .get(&agent)
            .ok_or_else(|| exception!(message = format!("agent not found, name={agent}")))?;
        chat.generate_stream(self.messages.clone()).await
    }

    pub fn add_user_message(&mut self, message: String, files: Vec<&Path>) -> Result<(), Exception> {
        self.messages
            .lock()
            .unwrap()
            .push(ChatRequestMessage::new_user_message(message, image_urls(files)?));
        Ok(())
    }

    pub fn add_assistant_message(&mut self, message: String) {
        self.messages
            .lock()
            .unwrap()
            .push(ChatRequestMessage::new_message(Role::Assistant, message));
    }
}

fn create_chat(
    agent_config: &AgentConfig,
    model_config: &ModelConfig,
    registry: &FunctionRegistry,
) -> Result<Chat, Exception> {
    let function_store = registry.create_store(agent_config.functions.as_ref().unwrap_or(&vec![]))?;

    let mut chat = Chat::new(
        model_config.url.to_string(),
        model_config.api_key.to_string(),
        model_config.model.to_string(),
        function_store,
    );

    if let Some(ref message) = agent_config.system_message {
        chat.config.system_message = Some(message.to_string());
    }
    if let Some(temperature) = agent_config.temperature {
        chat.config.temperature = Some(temperature);
    }
    if let Some(top_p) = agent_config.top_p {
        chat.config.top_p = Some(top_p);
    }

    Ok(chat)
}

fn image_urls(files: Vec<&Path>) -> Result<Vec<String>, Exception> {
    let mut image_urls = Vec::with_capacity(files.len());
    for file in files {
        image_urls.push(base64_image_url(file)?)
    }
    Ok(image_urls)
}

fn base64_image_url(path: &Path) -> Result<String, Exception> {
    let extension = path.file_extension()?;
    let content = fs::read(path)?;
    let mime_type = match extension {
        "jpg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        "pdf" => Ok("application/pdf".to_string()),
        _ => Err(exception!(
            message = format!("not supported extension, path={}", path.to_string_lossy())
        )),
    }?;
    Ok(format!("data:{mime_type};base64,{}", BASE64_STANDARD.encode(content)))
}
