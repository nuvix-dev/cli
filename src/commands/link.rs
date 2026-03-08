use crate::config::ProjectConfig;
use anyhow::Result;
use std::path::Path;

pub fn run(project_dir: &Path, url: String, token: String) -> Result<()> {
    let mut cfg = ProjectConfig::load_from(project_dir)?;

    cfg.remote = Some(crate::config::RemoteSection {
        url: url.clone(),
        token,
    });
    cfg.save(project_dir)?;

    println!("Linked project '{}' to {}", cfg.project.name, url);

    Ok(())
}
