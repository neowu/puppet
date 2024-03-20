use clap::Parser;
use clap::Subcommand;
use command::chat::Chat;
use command::generate_zsh_completion::GenerateZshCompletion;
use std::error::Error;

mod chatgpt;
mod command;
mod config;
mod openai;
mod util;

#[derive(Parser)]
#[command(author, version)]
#[command(about = "Puppet AI")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
pub enum Commands {
    Chat(Chat),
    GenerateZshCompletion(GenerateZshCompletion),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::GenerateZshCompletion(command)) => command.execute(),
        Some(Commands::Chat(command)) => command.execute().await,
        None => panic!("not implemented"),
    }
}
