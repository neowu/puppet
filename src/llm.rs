use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use anyhow::Result;
use futures::Stream;
use log::info;
use tokio::sync::mpsc::Receiver;

use crate::llm::config::Config;
use crate::util::json;

pub mod config;
pub mod function;

#[derive(Debug)]
pub struct ChatOption {
    pub temperature: f32,
}

pub struct TextStream {
    rx: Receiver<String>,
}

impl TextStream {
    pub fn new(rx: Receiver<String>) -> Self {
        TextStream { rx }
    }
}

impl Stream for TextStream {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(context)
    }
}

pub fn load(path: Option<&Path>) -> Result<Config> {
    let default_config_path = format!("{}/.config/puppet/llm.json", env!("HOME"));
    let path = path.unwrap_or(Path::new(&default_config_path));
    info!("load config, path={}", path.to_string_lossy());
    let content = fs::read_to_string(path)?;
    let config: Config = json::from_json(&content)?;
    Ok(config)
}
