use crate::cli::GenTypesArgs;
use crate::client::{NuvixClient, ensure_console_url};
use crate::global_config::GlobalConfig;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

pub fn types(project_dir: &Path, args: GenTypesArgs) -> Result<()> {
    let global = GlobalConfig::load_or_default()?;
    let project_id = global.resolve_project_id(args.project_id.as_deref())?;
    let profile = global
        .projects
        .get(&project_id)
        .with_context(|| format!("project profile '{}' not found", project_id))?;

    let session = profile
        .nc_session
        .as_ref()
        .filter(|v| !v.is_empty())
        .cloned()
        .context("missing nc_session. Run `nuvix auth login` first")?;

    let console_url = ensure_console_url(profile)?;
    let endpoint = format!(
        "database/generators/{}",
        args.language.as_endpoint_segment()
    );

    let client = NuvixClient::new(console_url, Some(session))?;
    let response = client
        .get(&endpoint)
        .send()
        .context("failed to call types generator endpoint")?;

    if !response.status().is_success() {
        bail!("type generation failed with status {}", response.status());
    }

    let generated = response
        .text()
        .context("failed to read generated type output")?;

    let output_path = args
        .output
        .unwrap_or_else(|| default_output_path(project_dir, args.language.default_filename()));

    if output_path.exists() && !args.force {
        bail!(
            "output file already exists at {}. Use --force to overwrite.",
            output_path.display()
        );
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory: {}", parent.display()))?;
    }

    fs::write(&output_path, generated)
        .with_context(|| format!("failed to write generated file: {}", output_path.display()))?;

    println!("Generated {} types.", args.language.as_endpoint_segment());
    println!("Project: {}", project_id);
    println!("Output: {}", output_path.display());

    Ok(())
}

fn default_output_path(project_dir: &Path, file_name: &str) -> PathBuf {
    project_dir.join("nuvix").join("types").join(file_name)
}
