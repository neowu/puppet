use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::generate_zsh_completion::GenerateZshCompletion;
use command::server::Server;
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
    Chat(Chat),
    Server(Server),
    GenerateZshCompletion(GenerateZshCompletion),
}

#[tokio::main]
async fn main() -> Result<(), Exception> {
    tracing_subscriber::fmt().with_thread_ids(true).init();
    let cli = Cli::parse();
    match cli.command {
        Some(Command::GenerateZshCompletion(command)) => command.execute(),
        Some(Command::Chat(command)) => command.execute().await,
        Some(Command::Server(command)) => command.execute().await,
        None => panic!("not implemented"),
    }
}
