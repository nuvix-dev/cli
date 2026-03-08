use crate::cli::{ProjectSetUrlsArgs, ProjectShowArgs, ProjectUseArgs};
use crate::global_config::{GlobalConfig, GlobalProjectProfile};
use anyhow::{Context, Result, bail};

pub fn set_urls(args: ProjectSetUrlsArgs) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;

    let profile = global
        .projects
        .entry(args.project_id.clone())
        .or_insert_with(GlobalProjectProfile::default);

    profile.api_url = Some(args.api_url);
    profile.console_api_url = Some(args.console_api_url);
    if let Some(console_url) = args.console_url {
        profile.console_url = Some(console_url);
    }

    if global.current_project_id.is_none() {
        global.current_project_id = Some(args.project_id.clone());
    }

    global.save()?;

    println!("Updated project profile '{}'.", args.project_id);
    Ok(())
}

pub fn use_project(args: ProjectUseArgs) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;
    if !global.projects.contains_key(&args.project_id) {
        bail!(
            "project profile '{}' not found. Create it with `nuvix project set-urls` or `nuvix self-host init`.",
            args.project_id
        );
    }

    global.current_project_id = Some(args.project_id.clone());
    global.save()?;

    println!("Current project set to '{}'.", args.project_id);
    Ok(())
}

pub fn show(args: ProjectShowArgs) -> Result<()> {
    let global = GlobalConfig::load_or_default()?;

    if args.list {
        if global.projects.is_empty() {
            println!("No project profiles found.");
            return Ok(());
        }

        println!("Project profiles:");
        for project_id in global.projects.keys() {
            let marker = if global.current_project_id.as_deref() == Some(project_id.as_str()) {
                "*"
            } else {
                " "
            };
            println!("{} {}", marker, project_id);
        }
        return Ok(());
    }

    let project_id = global.resolve_project_id(args.project_id.as_deref())?;
    let profile = global
        .projects
        .get(&project_id)
        .with_context(|| format!("project profile '{}' not found", project_id))?;

    println!("Project: {}", project_id);
    println!(
        "Current: {}",
        global.current_project_id.as_deref() == Some(project_id.as_str())
    );
    println!(
        "API URL: {}",
        profile.api_url.as_deref().unwrap_or("<unset>")
    );
    println!(
        "Console API URL: {}",
        profile.console_api_url.as_deref().unwrap_or("<unset>")
    );
    println!(
        "Console URL: {}",
        profile.console_url.as_deref().unwrap_or("<unset>")
    );
    println!(
        "Self-host docker dir: {}",
        profile
            .self_host_docker_dir
            .as_ref()
            .map(|v| v.display().to_string())
            .unwrap_or_else(|| "<unset>".to_string())
    );
    println!(
        "Self-host env file: {}",
        profile
            .self_host_env_file
            .as_ref()
            .map(|v| v.display().to_string())
            .unwrap_or_else(|| "<unset>".to_string())
    );

    Ok(())
}
