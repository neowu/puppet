use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::generate_zsh_completion::GenerateZshCompletion;
use util::exception::Exception;

mod bot;
mod command;
mod gcloud;
mod openai;
mod util;

#[derive(Parser)]
#[command(author, version)]
#[command(about = "Puppet AI")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
pub enum Command {
    #[command(about = "Chat")]
    Chat(Chat),
    #[command(about = "Generate zsh completion")]
    GenerateZshCompletion(GenerateZshCompletion),
}

#[tokio::main]
async fn main() -> Result<(), Exception> {
    tracing_subscriber::fmt().with_thread_ids(true).init();
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Chat(command)) => command.execute().await,
        Some(Command::GenerateZshCompletion(command)) => command.execute(),
        None => panic!("not implemented"),
    }
}
