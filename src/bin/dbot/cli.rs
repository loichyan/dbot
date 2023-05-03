use crate::options::Options;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(flatten)]
    pub options: Options,
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Clean and create target files.
    Apply {},
    /// Remove all target files created last time.
    Clean {},
    /// List all managed target files.
    Ls {},
}
