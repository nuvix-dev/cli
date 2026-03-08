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
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return Ok(Path::new(&xdg).join("nuvix").join(GLOBAL_CONFIG_FILE));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            if !app_data.trim().is_empty() {
                return Ok(Path::new(&app_data).join("nuvix").join(GLOBAL_CONFIG_FILE));
            }
        }
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME/USERPROFILE is not set")?;
    Ok(Path::new(&home)
        .join(GLOBAL_CONFIG_DIR)
        .join(GLOBAL_CONFIG_FILE))
}

pub fn load_session(_project_id: &str, profile: &GlobalProjectProfile) -> Option<String> {
    profile
        .nc_session
        .as_ref()
        .filter(|v| !v.trim().is_empty())
        .cloned()
}

pub fn store_session(project_id: &str, session: &str) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;
    let profile = global
        .projects
        .entry(project_id.to_string())
        .or_insert_with(GlobalProjectProfile::default);
    profile.nc_session = Some(session.to_string());
    global.save()
}

pub fn clear_session(project_id: &str) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;
    if let Some(profile) = global.projects.get_mut(project_id) {
        profile.nc_session = None;
    }
    global.save()
}
