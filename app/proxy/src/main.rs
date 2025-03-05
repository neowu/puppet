use std::path::PathBuf;

use anyhow::Result;
use axum::Router;
use clap::Parser;
use clap::command;
use config::Config;
use framework::json;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

mod config;
mod proxy;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, help = "conf path")]
    conf: PathBuf,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?
        .add_directive("proxy=trace".parse()?)
        .add_directive("framework=trace".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .with_line_number(true)
        .with_thread_ids(true)
        .init();

    let cli = Cli::parse();
    let config: Config = json::load_file(&cli.conf)?;
    let state = AppState { config };

    let app: Router<AppState> = Router::new();
    let app = app.merge(proxy::routes());
    let app = app.with_state(state);

    framework::web::server::start_http_server(app).await?;
    framework::task::shutdown().await;

    Ok(())
}
