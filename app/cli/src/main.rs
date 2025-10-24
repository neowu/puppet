use clap::Parser;
use clap::Subcommand;
use command::complete::Complete;
use command::completion::Completion;
use framework::exception::Exception;
use framework::log;
use framework::log::ConsoleAppender;

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
    #[command(about = "complete prompt file")]
    Complete(Complete),
    #[command(about = "generate shell completion")]
    Completion(Completion),
}

#[tokio::main]
async fn main() -> Result<(), Exception> {
    log::init_with_action(ConsoleAppender);

    let cli = Cli::parse();
    match cli.command {
        Command::Complete(command) => command.execute().await,
        Command::Completion(command) => command.execute(),
    }
}
