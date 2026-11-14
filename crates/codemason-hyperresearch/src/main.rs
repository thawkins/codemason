//! `hyperresearch` binary entry point.
//!
//! Contains no business logic beyond argument parsing and dispatch to
//! [`codemason_hyperresearch::cli::run`] (FR-002).

#![deny(unsafe_code)]

use clap::Parser;
use codemason_hyperresearch::cli::args::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    codemason_hyperresearch::cli::run(cli)?;
    Ok(())
}
