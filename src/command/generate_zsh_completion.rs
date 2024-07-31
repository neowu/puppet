use std::io;

use anyhow::Result;
use clap::Args;
use clap::CommandFactory;
use clap_complete::generate;
use clap_complete::shells::Zsh;

use crate::Cli;

const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Args)]
pub struct GenerateZshCompletion {}

impl GenerateZshCompletion {
    pub fn execute(&self) -> Result<()> {
        generate(Zsh, &mut Cli::command(), CARGO_PKG_NAME, &mut io::stdout());
        Ok(())
    }
}
