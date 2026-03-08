use crate::cli::{AuthLoginArgs, AuthLogoutArgs, AuthStatusArgs};
use crate::client;
use crate::global_config::GlobalConfig;
use anyhow::{Context, Result, bail};
use dialoguer::Password;

pub fn login(args: AuthLoginArgs) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;
    let project_id = global.resolve_project_id(args.project_id.as_deref())?;

    let profile = global
        .projects
        .get_mut(&project_id)
        .with_context(|| format!("project profile '{}' not found", project_id))?;

    let console_api_url = client::ensure_console_api_url(profile)?;
    let password = match args.password {
        Some(value) => value,
        None => Password::new().with_prompt("Password").interact()?,
    };

    let nc_session = client::NuvixClient::login_email(&console_api_url, &args.email, &password)?;

    profile.auth_email = Some(args.email);
    profile.nc_session = Some(nc_session);
    global.current_project_id = Some(project_id.clone());
    global.save()?;

    println!("Login successful for project '{}'.", project_id);
    Ok(())
}

pub fn status(args: AuthStatusArgs) -> Result<()> {
    let global = GlobalConfig::load_or_default()?;
    let project_id = global.resolve_project_id(args.project_id.as_deref())?;

    let profile = global
        .projects
        .get(&project_id)
        .with_context(|| format!("project profile '{}' not found", project_id))?;

    let has_session = profile
        .nc_session
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    println!("Project: {}", project_id);
    println!("Authenticated: {}", has_session);
    println!(
        "Auth email: {}",
        profile.auth_email.as_deref().unwrap_or("<unset>")
    );

    Ok(())
}

pub fn logout(args: AuthLogoutArgs) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;
    let project_id = global.resolve_project_id(args.project_id.as_deref())?;

    let profile = global
        .projects
        .get_mut(&project_id)
        .with_context(|| format!("project profile '{}' not found", project_id))?;

    if profile.nc_session.is_none() {
        bail!("no active session stored for project '{}'", project_id);
    }

    profile.nc_session = None;
    global.save()?;

    println!("Logged out from project '{}'.", project_id);
    Ok(())
}
