use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::complete::Complete;
use command::completion::Completion;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

pub mod agent;
mod command;

#[derive(Parser)]
#[command(author, version)]
#[command(about = "puppet ai")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
pub enum Command {
    #[command(about = "interactive chat")]
    Chat(Chat),
    #[command(about = "complete prompt file")]
    Complete(Complete),
    #[command(about = "generate shell completion")]
    Completion(Completion),
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = filter::Targets::new().with_default(LevelFilter::INFO);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_line_number(true)
                .with_thread_ids(true)
                .with_filter(filter),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Chat(command) => command.execute().await,
        Command::Complete(command) => command.execute().await,
        Command::Completion(command) => command.execute(),
    }
}
