mod cli;
mod commands;
mod config;
mod state;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, SelfHostCommand};
use std::env;
use std::path::PathBuf;

fn resolve_project_dir(input: Option<PathBuf>) -> Result<PathBuf> {
    match input {
        Some(path) => Ok(path),
        None => Ok(env::current_dir()?),
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = resolve_project_dir(cli.project_dir)?;

    match cli.command {
        Commands::SelfHost { command } => match command {
            SelfHostCommand::Init(args) => commands::self_host::init(&project_dir, args),
            SelfHostCommand::Up(args) => commands::self_host::up(&project_dir, args),
            SelfHostCommand::Down(args) => commands::self_host::down(&project_dir, args),
            SelfHostCommand::Status(args) => commands::self_host::status(&project_dir, args),
        },
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
