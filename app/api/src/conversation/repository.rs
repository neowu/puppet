use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use duckdb::params;
use duckdb::types::FromSqlError;
use duckdb::Connection;
use duckdb::Row;
use framework::json::from_json;
use framework::json::to_json;
use serde::Deserialize;
use serde::Serialize;

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"BEGIN;
                CREATE SEQUENCE IF NOT EXISTS id_seq START 1;
                CREATE TABLE IF NOT EXISTS conversation (id INTEGER PRIMARY KEY, summary VARCHAR, messages JSON, created_time TIMESTAMP);
                COMMIT;"#,
    )?;
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Conversation {
    pub id: u32,
    pub summary: String,
    pub messages: Vec<Message>,
    pub created_time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub role: String,
    pub message: String,
}

pub fn create_conversation(conn: Arc<Mutex<Connection>>) -> Result<Conversation> {
    let conn = conn.lock().unwrap();
    let id: u32 = conn.query_row("SELECT nextval('id_seq')", [], |row| row.get(0))?;
    let now = Utc::now();
    conn.execute(
        "INSERT INTO conversation (id, summary, messages, created_time) VALUES (?, ?, ?, ?)",
        params![id, "New conversation", "[]", now.clone()],
    )?;
    Ok(Conversation {
        id,
        summary: "New conversation".to_string(),
        messages: vec![],
        created_time: now,
    })
}

pub fn list_conversations(conn: Arc<Mutex<Connection>>) -> Result<Vec<Conversation>> {
    let conn = conn.lock().unwrap();
    let mut statement = conn.prepare("SELECT id, summary, messages, created_time FROM conversation")?;
    let rows = statement.query_map([], conversation_row_map)?;
    rows.into_iter().map(|row| row.map_err(|e| e.into())).collect()
}

pub fn get_conversation(conn: Arc<Mutex<Connection>>, id: u32) -> Result<Conversation> {
    let conn = conn.lock().unwrap();
    let coversation = conn.query_row(
        "SELECT id, summary, messages, created_time FROM conversation WHERE id = ?",
        [id],
        conversation_row_map,
    )?;
    Ok(coversation)
}

fn conversation_row_map(row: &Row<'_>) -> duckdb::Result<Conversation> {
    let messages_json = row.get::<_, String>(2);
    let messages = from_json(&messages_json?).map_err(|e| FromSqlError::Other(e.to_string().into()))?;
    Ok(Conversation {
        id: row.get(0)?,
        summary: row.get(1)?,
        messages,
        created_time: row.get(3)?,
    })
}

pub fn save_conversation(conn: Arc<Mutex<Connection>>, conversation: Conversation) -> Result<()> {
    let conn = conn.lock().unwrap();
    conn.execute(
        "INSERT INTO conversation (id, summary, messages, created_time) VALUES (?, ?, ?, ?)",
        params![
            conversation.id,
            conversation.summary,
            to_json(&conversation.messages)?,
            conversation.created_time
        ],
    )?;
    Ok(())
}
