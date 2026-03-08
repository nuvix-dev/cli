use crate::cli::InitArgs;
use anyhow::{Context, Result, bail};
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const NUVIX_DIR: &str = "nuvix";
const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Serialize, Deserialize)]
struct LocalProjectConfig {
    project_id: String,
}

pub fn run(project_dir: &Path, args: InitArgs) -> Result<()> {
    let project_id = match args.project_id {
        Some(v) => v,
        None => Input::new()
            .with_prompt("Project ID")
            .interact_text()
            .context("failed to read project id")?,
    };

    let nuvix_dir = project_dir.join(NUVIX_DIR);
    let config_path = nuvix_dir.join(CONFIG_FILE);

    if config_path.exists() && !args.force {
        bail!(
            "config already exists at {}. Use --force to overwrite.",
            config_path.display()
        );
    }

    fs::create_dir_all(&nuvix_dir)
        .with_context(|| format!("failed to create directory: {}", nuvix_dir.display()))?;

    let cfg = LocalProjectConfig {
        project_id: project_id.clone(),
    };
    let raw = toml::to_string_pretty(&cfg).context("failed to serialize project config")?;
    fs::write(&config_path, raw)
        .with_context(|| format!("failed to write config file: {}", config_path.display()))?;

    println!("Initialized Nuvix project config.");
    println!("Project ID: {}", project_id);
    println!("Config: {}", config_path.display());

    Ok(())
}
