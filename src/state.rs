use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const STATE_DIR: &str = ".nuvix";
pub const STATE_FILE: &str = "state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliState {
    pub local_running: bool,
}

impl Default for CliState {
    fn default() -> Self {
        Self {
            local_running: false,
        }
    }
}

impl CliState {
    pub fn load_or_default(project_dir: &Path) -> Result<Self> {
        let state_path = state_path(project_dir);
        if !state_path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read state file: {}", state_path.display()))?;
        let state = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse state file: {}", state_path.display()))?;
        Ok(state)
    }

    pub fn save(&self, project_dir: &Path) -> Result<()> {
        let dir = project_dir.join(STATE_DIR);
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create state dir: {}", dir.display()))?;

        let state_path = dir.join(STATE_FILE);
        let raw = serde_json::to_string_pretty(self).context("failed to serialize state")?;

        fs::write(&state_path, raw)
            .with_context(|| format!("failed to write state file: {}", state_path.display()))
    }
}

pub fn state_path(project_dir: &Path) -> std::path::PathBuf {
    project_dir.join(STATE_DIR).join(STATE_FILE)
}
