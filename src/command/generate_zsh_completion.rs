use std::error::Error;
use std::io;

use clap::{Args, CommandFactory};
use clap_complete::{generate, shells::Zsh};

use crate::Cli;

const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Args)]
#[command(about = "Generate zsh completion")]
pub struct GenerateZshCompletion {}

impl GenerateZshCompletion {
    pub fn execute(&self) -> Result<(), Box<dyn Error>> {
        generate(Zsh, &mut Cli::command(), CARGO_PKG_NAME, &mut io::stdout());
        Ok(())
    }
}
