use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::complete::Complete;
use command::completion::Completion;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod command;
mod config;

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
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().compact().with_line_number(true).with_thread_ids(true))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Chat(command) => command.execute().await,
        Command::Complete(command) => command.execute().await,
        Command::Completion(command) => command.execute(),
    }
}
