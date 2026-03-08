mod cli;
mod client;
mod commands;
mod config;
mod global_config;
mod state;

use anyhow::Result;
use clap::Parser;
use cli::{
    AuthCommand, Cli, CollectionsCommand, Commands, GenCommand, LocalCommand, MigrationCommand,
    ProjectCommand,
};
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
        Commands::Init(args) => commands::init_project::run(&project_dir, args),
        Commands::Local { command } => match command {
            LocalCommand::Init(args) => commands::self_host::init(&project_dir, args),
            LocalCommand::Up(args) => commands::self_host::up(&project_dir, args),
            LocalCommand::Down(args) => commands::self_host::down(&project_dir, args),
            LocalCommand::Status(args) => commands::self_host::status(&project_dir, args),
        },
        Commands::Project { command } => match command {
            ProjectCommand::SetUrls(args) => commands::project::set_urls(args),
            ProjectCommand::Use(args) => commands::project::use_project(args),
            ProjectCommand::Show(args) => commands::project::show(args),
        },
        Commands::Auth { command } => match command {
            AuthCommand::Login(args) => commands::auth::login(args),
            AuthCommand::Status(args) => commands::auth::status(args),
            AuthCommand::Logout(args) => commands::auth::logout(args),
        },
        Commands::Gen { command } => match command {
            GenCommand::Types(args) => commands::typegen::types(&project_dir, args),
        },
        Commands::Migration { command } => match command {
            MigrationCommand::New(args) => commands::migration::new_migration(&project_dir, args),
            MigrationCommand::Pull(args) => commands::migration::pull(&project_dir, args),
            MigrationCommand::Up(args) => commands::migration::up(&project_dir, args),
            MigrationCommand::Status(args) => commands::migration::status(&project_dir, args),
        },
        Commands::Collections { command } => match command {
            CollectionsCommand::Init(args) => commands::collections::init(&project_dir, args),
            CollectionsCommand::List(args) => commands::collections::list(&project_dir, args),
            CollectionsCommand::Show(args) => commands::collections::show(&project_dir, args),
            CollectionsCommand::AddCollection(args) => {
                commands::collections::add_collection(&project_dir, args)
            }
            CollectionsCommand::RemoveCollection(args) => {
                commands::collections::remove_collection(&project_dir, args)
            }
            CollectionsCommand::AddAttribute(args) => {
                commands::collections::add_attribute(&project_dir, args)
            }
            CollectionsCommand::AddIndex(args) => {
                commands::collections::add_index(&project_dir, args)
            }
            CollectionsCommand::Pull(args) => commands::collections::pull(&project_dir, args),
            CollectionsCommand::Push(args) => commands::collections::push(&project_dir, args),
            CollectionsCommand::Validate(args) => {
                commands::collections::validate(&project_dir, args)
            }
        },
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
