use std::borrow::Cow;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use framework::exception;
use framework::exception::Exception;
use framework::fs::path::PathExt;
use tracing::debug;

use crate::openai::chat_api::ChatRequestMessage;
use crate::openai::chat_api::ResponseFormat;
use crate::openai::chat_api::Role;

#[derive(Default)]
pub struct Session {
    pub(crate) messages: Vec<ChatRequestMessage>,
    pub functions: Option<Vec<String>>,
    pub top_p: Option<f32>,
    pub temperature: Option<f32>,
    pub response_format: Option<ResponseFormat>,
    pub max_completion_tokens: Option<i32>,
}

pub enum Message {
    SystemMessage(String),
    UserMessage(String),
    AssistantMessage(String),
    Images(Vec<PathBuf>),
    Files(Vec<PathBuf>),
}

impl Session {
    pub fn add_message(&mut self, message: Message) -> Result<(), Exception> {
        self.messages.push(match message {
            Message::SystemMessage(value) => {
                debug!("[chat] system: {value}");
                ChatRequestMessage::new_message(Role::System, value.to_string())
            }
            Message::UserMessage(value) => {
                debug!("[chat] user: {value}");
                ChatRequestMessage::new_message(Role::User, value.to_string())
            }
            Message::AssistantMessage(value) => {
                debug!("[chat] assistant: {value}");
                ChatRequestMessage::new_message(Role::Assistant, value.to_string())
            }
            Message::Images(paths) => {
                let path_values: Vec<Cow<str>> = paths.iter().map(|path| path.to_string_lossy()).collect();
                debug!("[chat] images: paths={path_values:?}");
                let url = paths
                    .into_iter()
                    .map(|path| base64_image_url(&path))
                    .collect::<Result<Vec<String>, Exception>>()?;
                ChatRequestMessage::new_user_images(url)
            }
            Message::Files(paths) => {
                let path_values: Vec<Cow<str>> = paths.iter().map(|path| path.to_string_lossy()).collect();
                debug!("[chat] files: paths={path_values:?}");
                ChatRequestMessage::new_user_files(paths)?
            }
        });
        Ok(())
    }
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
