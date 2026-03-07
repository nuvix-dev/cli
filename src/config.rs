use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "nuvix.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectSection,
    pub local: LocalSection,
    #[serde(default)]
    pub remote: Option<RemoteSection>,
    #[serde(default)]
    pub self_host: Option<SelfHostSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSection {
    pub api_port: u16,
    pub db_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSection {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfHostSection {
    #[serde(default)]
    pub default_project_id: Option<String>,
    #[serde(default)]
    pub projects: BTreeMap<String, SelfHostProjectSection>,
    #[serde(default)]
    pub docker_dir: Option<PathBuf>,
    #[serde(default)]
    pub env_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfHostProjectSection {
    pub docker_dir: PathBuf,
    pub env_file: PathBuf,
}

impl ProjectConfig {
    pub fn new(project_name: String) -> Self {
        Self {
            project: ProjectSection { name: project_name },
            local: LocalSection {
                api_port: 54321,
                db_port: 54322,
            },
            remote: None,
            self_host: None,
        }
    }

    pub fn load_or_new(project_dir: &Path) -> Result<Self> {
        let config_path = project_dir.join(CONFIG_FILE);
        if !config_path.exists() {
            let name = project_dir
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("nuvix-project")
                .to_string();
            return Ok(Self::new(name));
        }

        Self::load_from(project_dir)
    }

    pub fn load_from(project_dir: &Path) -> Result<Self> {
        let config_path = project_dir.join(CONFIG_FILE);
        let raw = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file: {}", config_path.display()))?;
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse config file: {}", config_path.display()))
    }

    pub fn save_to(&self, project_dir: &Path, force: bool) -> Result<()> {
        let config_path = project_dir.join(CONFIG_FILE);

        if config_path.exists() && !force {
            bail!(
                "config file already exists at {}. Use --force to overwrite.",
                config_path.display()
            );
        }

        let raw = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&config_path, raw)
            .with_context(|| format!("failed to write config file: {}", config_path.display()))
    }
}
