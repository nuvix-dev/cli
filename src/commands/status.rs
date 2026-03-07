use crate::config::ProjectConfig;
use crate::state::{CliState, state_path};
use anyhow::Result;
use std::path::Path;

pub fn run(project_dir: &Path) -> Result<()> {
    let cfg = ProjectConfig::load_from(project_dir)?;
    let state = CliState::load_or_default(project_dir)?;

    println!("Project: {}", cfg.project.name);
    println!("Config: {}", project_dir.join("nuvix.toml").display());
    println!("State: {}", state_path(project_dir).display());
    println!("Local stack running: {}", state.local_running);
    println!("Local API URL: http://localhost:{}", cfg.local.api_port);

    if let Some(remote) = cfg.remote {
        println!("Remote linked: yes");
        println!("Remote URL: {}", remote.url);
    } else {
        println!("Remote linked: no");
    }

    Ok(())
}
