use std::path::PathBuf;

use anyhow::Result;
use axum::Router;
use clap::Parser;
use config::Config;
use framework::json::load_file;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub mod config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, help = "conf path")]
    conf: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().compact().with_line_number(true).with_thread_ids(true))
        .init();

    let cli = Cli::parse();
    let conf = cli.conf.unwrap_or_else(|| PathBuf::from("./api.json"));
    let _: Config = load_file(&conf)?;

    let app = Router::new();

    framework::web::start_http_server(app).await?;
    framework::task::shutdown().await;

    Ok(())
}
