use crate::config::ProjectConfig;
use anyhow::Result;
use std::path::Path;

pub fn run(project_dir: &Path, name: Option<String>, force: bool) -> Result<()> {
    let project_name = name.unwrap_or_else(|| {
        project_dir
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("nuvix-project")
            .to_string()
    });

    let cfg = ProjectConfig::new(project_name.clone());
    cfg.save_to(project_dir, force)?;

    println!("Initialized Nuvix project '{}'", project_name);
    println!("Created config: {}", project_dir.join("nuvix.toml").display());

    Ok(())
}
