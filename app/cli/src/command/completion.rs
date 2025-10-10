use std::io;

use clap::Args;
use clap::CommandFactory;
use clap_complete::Shell;
use clap_complete::generate;
use framework::exception;
use framework::exception::Exception;

use crate::Cli;

const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Args)]
pub struct Completion;

impl Completion {
    pub fn execute(&self) -> Result<(), Exception> {
        let shell = Shell::from_env().ok_or_else(|| exception!(message = "unknown shell"))?;
        generate(shell, &mut Cli::command(), CARGO_PKG_NAME, &mut io::stdout());
        Ok(())
    }
}
