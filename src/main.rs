use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::generate_zsh_completion::GenerateZshCompletion;
use command::speak::Speak;
use util::exception::Exception;

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
    command: Option<Command>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
pub enum Command {
    #[command(about = "chat")]
    Chat(Chat),
    #[command(about = "speak")]
    Speech(Speak),
    #[command(about = "generate zsh completion")]
    GenerateZshCompletion(GenerateZshCompletion),
}

#[tokio::main]
async fn main() -> Result<(), Exception> {
    tracing_subscriber::fmt().with_thread_ids(true).init();
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Chat(command)) => command.execute().await,
        Some(Command::Speech(command)) => command.execute().await,
        Some(Command::GenerateZshCompletion(command)) => command.execute(),
        None => panic!("not implemented"),
    }
}
