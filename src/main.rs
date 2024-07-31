use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::complete::Complete;
use command::generate_zsh_completion::GenerateZshCompletion;
use command::speak::Speak;

mod azure;
mod command;
mod gcloud;
mod llm;
mod provider;
mod tts;
mod util;

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
    #[command(about = "speak")]
    Speech(Speak),
    #[command(about = "complete prompt file")]
    Complete(Complete),
    #[command(about = "generate zsh completion")]
    GenerateZshCompletion(GenerateZshCompletion),
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();
    let cli = Cli::parse();
    match cli.command {
        Command::Chat(command) => command.execute().await,
        Command::Speech(command) => command.execute().await,
        Command::Complete(command) => command.execute().await,
        Command::GenerateZshCompletion(command) => command.execute(),
    }
}
