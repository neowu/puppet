use std::collections::HashMap;
use std::error::Error;

use futures::stream::StreamExt;
use reqwest_eventsource::Event;
use tokio::sync::mpsc::channel;

use crate::openai::chat_completion::ChatRequest;
use crate::openai::chat_completion::ChatRequestMessage;
use crate::openai::chat_completion::ChatResponse;
use crate::openai::chat_completion::Role;
use crate::openai::Client;
use crate::util::json;

pub struct ChatGPT {
    pub client: Client,
    pub messages: Vec<ChatRequestMessage>,
    pub functions: HashMap<String, Box<dyn Fn(String) -> String>>,
}

pub trait ChatHandler {
    fn on_event(&self, event: &ChatEvent);
}

pub enum ChatEvent {
    Delta(String),
    Error(String),
    End,
}

impl ChatGPT {
    pub fn new(client: Client, system_message: Option<String>) -> Self {
        let mut chatgpt = ChatGPT {
            client,
            messages: vec![],
            functions: HashMap::new(),
        };
        if let Some(message) = system_message {
            chatgpt.messages.push(ChatRequestMessage::new(Role::System, &message));
        }
        chatgpt
    }

    pub async fn chat(&self, message: &str, handler: &dyn ChatHandler) -> Result<(), Box<dyn Error>> {
        let mut request = ChatRequest::new();
        request.messages.push(ChatRequestMessage::new(Role::User, message));
        let mut source = self.client.post_sse(&request).await?;
        let (tx, mut rx) = channel(64);
        tokio::spawn(async move {
            while let Some(event) = source.next().await {
                match event {
                    Ok(Event::Open) => {}
                    Ok(Event::Message(message)) => {
                        let data = message.data;

                        if data == "[DONE]" {
                            source.close();
                            tx.send(ChatEvent::End).await.unwrap();
                            break;
                        }

                        let response: ChatResponse = json::from_json(&data).unwrap();
                        if response.choices.is_empty() {
                            continue;
                        }
                        let content = response.choices.first().unwrap().delta.as_ref().unwrap();
                        if let Some(value) = content.content.as_ref() {
                            tx.send(ChatEvent::Delta(value.to_string())).await.unwrap();
                        }
                    }
                    Err(err) => {
                        tx.send(ChatEvent::Error(err.to_string())).await.unwrap();
                        source.close();
                    }
                }
            }
        });

        let mut assistant_message = String::new();
        while let Some(event) = rx.recv().await {
            handler.on_event(&event);
            if let ChatEvent::Delta(data) = event {
                assistant_message.push_str(&data);
            }
        }
        request.messages.push(ChatRequestMessage::new(Role::Assistant, &assistant_message));

        Ok(())
    }
}
