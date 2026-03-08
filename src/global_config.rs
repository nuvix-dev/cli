use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const GLOBAL_CONFIG_DIR: &str = ".config/nuvix";
const GLOBAL_CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub current_project_id: Option<String>,
    #[serde(default)]
    pub projects: BTreeMap<String, GlobalProjectProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalProjectProfile {
    #[serde(default)]
    pub api_url: Option<String>,
    #[serde(default)]
    pub console_api_url: Option<String>,
    #[serde(default)]
    pub console_url: Option<String>,
    #[serde(default)]
    pub self_host_docker_dir: Option<PathBuf>,
    #[serde(default)]
    pub self_host_env_file: Option<PathBuf>,
    #[serde(default)]
    pub auth_email: Option<String>,
    #[serde(default)]
    pub nc_session: Option<String>,
}

impl GlobalConfig {
    pub fn load_or_default() -> Result<Self> {
        let path = global_config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read global config: {}", path.display()))?;
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse global config: {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = global_config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir: {}", parent.display()))?;
        }

        let raw = toml::to_string_pretty(self).context("failed to serialize global config")?;
        fs::write(&path, raw)
            .with_context(|| format!("failed to write global config: {}", path.display()))
    }

    pub fn resolve_project_id(&self, requested: Option<&str>) -> Result<String> {
        if let Some(project_id) = requested {
            return Ok(project_id.to_string());
        }

        if let Some(current) = &self.current_project_id {
            return Ok(current.clone());
        }

        if self.projects.len() == 1 {
            if let Some((project_id, _)) = self.projects.iter().next() {
                return Ok(project_id.clone());
            }
        }

        anyhow::bail!(
            "project id is required; set current project with `nuvix project use --project-id <id>`"
        )
    }
}

pub fn global_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(Path::new(&home)
        .join(GLOBAL_CONFIG_DIR)
        .join(GLOBAL_CONFIG_FILE))
}
