use crate::config::ProjectConfig;
use crate::state::CliState;
use anyhow::Result;
use std::path::Path;

pub fn run(project_dir: &Path) -> Result<()> {
    let cfg = ProjectConfig::load_from(project_dir)?;
    let mut state = CliState::load_or_default(project_dir)?;

    state.local_running = true;
    state.save(project_dir)?;

    println!("Starting local Nuvix stack for '{}'", cfg.project.name);
    println!("API will be available at http://localhost:{}", cfg.local.api_port);
    println!("Database will be available at localhost:{}", cfg.local.db_port);
    println!("(placeholder) local runtime marked as running");

    Ok(())
}
