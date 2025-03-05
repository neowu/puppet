use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use axum::Router;
use clap::Parser;
use config::Config;
use duckdb::Connection;
use framework::json::load_file;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub mod config;
pub mod conversation;
pub mod web;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,
}

#[derive(Clone)]
pub struct ApiState {
    db: Arc<Mutex<Connection>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_line_number(true)
                .with_thread_ids(true),
        )
        .init();

    let cli = Cli::parse();
    let conf = cli.conf.unwrap_or_else(|| PathBuf::from("./api.json"));
    let conf: Config = load_file(&conf)?;

    let conn = Connection::open(conf.db_path)?;
    let state = ApiState {
        db: Arc::new(Mutex::new(conn.try_clone()?)),
    };
    conversation::init(&conn)?;

    let app = Router::new();
    let app = app.merge(conversation::routes());
    let app = app.with_state(state);

    framework::web::server::start_http_server(app).await?;
    framework::task::shutdown().await;

    Ok(())
}
