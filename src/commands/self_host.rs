use crate::cli::{SelfHostDownArgs, SelfHostInitArgs, SelfHostStatusArgs, SelfHostUpArgs};
use crate::config::{ProjectConfig, SelfHostProjectSection, SelfHostSection};
use crate::global_config::{GlobalConfig, GlobalProjectProfile};
use crate::state::CliState;
use anyhow::{Context, Result, bail};
use dialoguer::{Confirm, Input, Password, theme::ColorfulTheme};
use rand::Rng;
use rand::distr::Alphanumeric;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_DOCKER_DIR: &str = "docker";
const DEFAULT_ENV_FILE_NAME: &str = ".env";
const DEFAULT_EXAMPLE_FILE_NAME: &str = ".env.example";
const DEFAULT_DOCKER_REPO: &str = "https://github.com/Nuvix-dev/docker";
const DEFAULT_API_PORT: u16 = 4000;
const DEFAULT_CONSOLE_API_PORT: u16 = 4100;
const DEFAULT_CONSOLE_PORT: u16 = 3000;
const DEFAULT_DATABASE_PORT: u16 = 5432;

pub fn init(project_dir: &Path, args: SelfHostInitArgs) -> Result<()> {
    let mut cfg = ProjectConfig::load_or_new(project_dir)?;

    let initial_project_id = args
        .project_id
        .clone()
        .unwrap_or_else(|| cfg.project.name.clone());

    let base_docker_dir = cfg
        .self_host
        .as_ref()
        .and_then(|v| v.projects.get(&initial_project_id))
        .map(|v| v.docker_dir.clone())
        .or_else(|| {
            cfg.self_host
                .as_ref()
                .and_then(|v| v.docker_dir.as_ref().cloned())
        })
        .unwrap_or_else(|| project_dir.join(DEFAULT_DOCKER_DIR));

    let docker_dir = resolve_path(
        project_dir,
        args.docker_dir.clone().unwrap_or(base_docker_dir),
    );

    if !docker_dir.exists() {
        let repo = args
            .docker_repo
            .clone()
            .unwrap_or_else(|| DEFAULT_DOCKER_REPO.to_string());
        clone_docker_repo(&repo, &docker_dir)?;
    } else if args.pull {
        pull_docker_repo_if_git(&docker_dir)?;
    }

    let env_values = if args.non_interactive {
        env_values_from_non_interactive(&cfg, &args)?
    } else {
        env_values_from_interactive(&cfg, &args)?
    };

    let project_id = env_values
        .get("NUVIX_PROJECT_ID")
        .cloned()
        .context("missing NUVIX_PROJECT_ID in computed env values")?;

    let existing_env_file = cfg
        .self_host
        .as_ref()
        .and_then(|v| v.projects.get(&project_id))
        .map(|v| v.env_file.clone())
        .or_else(|| {
            cfg.self_host
                .as_ref()
                .and_then(|v| v.env_file.as_ref().cloned())
        });

    let env_file = resolve_path(
        project_dir,
        args.env_file
            .clone()
            .or(existing_env_file)
            .unwrap_or_else(|| docker_dir.join(DEFAULT_ENV_FILE_NAME)),
    );

    write_aligned_env_file(&docker_dir, &env_file, &env_values, args.force)?;

    let self_host = cfg.self_host.get_or_insert(SelfHostSection {
        default_project_id: None,
        projects: BTreeMap::new(),
        docker_dir: None,
        env_file: None,
    });

    self_host.projects.insert(
        project_id.clone(),
        SelfHostProjectSection {
            docker_dir: docker_dir.clone(),
            env_file: env_file.clone(),
        },
    );
    self_host.default_project_id = Some(project_id.clone());
    self_host.docker_dir = None;
    self_host.env_file = None;

    cfg.save_to(project_dir, true)?;
    register_global_project_profile(&project_id, &docker_dir, &env_file, &env_values)?;

    let mut state = CliState::load_or_default(project_dir)?;
    state.local_running = false;
    state.save(project_dir)?;

    println!("Self-host configuration initialized.");
    println!("Project id: {}", project_id);
    println!("Docker directory: {}", docker_dir.display());
    println!("Env file: {}", env_file.display());
    println!(
        "Ports => api: {}, console-api: {}, console: {}, db: {}",
        env_values["NUVIX_API_PORT"],
        env_values["NUVIX_CONSOLE_API_PORT"],
        env_values["NUVIX_CONSOLE_PORT"],
        env_values["NUVIX_DATABASE_PORT"]
    );
    println!("API endpoint: {}", env_values["NUVIX_API_ENDPOINT"]);
    println!(
        "Console API endpoint: {}",
        env_values["NUVIX_CONSOLE_API_ENDPOINT"]
    );
    println!("Console URL: {}", env_values["NUVIX_CONSOLE_URL"]);
    println!("Next: nuvix local up --project-id {}", project_id);

    Ok(())
}

pub fn up(project_dir: &Path, args: SelfHostUpArgs) -> Result<()> {
    let cfg = ProjectConfig::load_from(project_dir)?;
    let (project_id, project) = resolve_project_entry(&cfg, args.project_id.as_deref())?;

    ensure_exists(&project.docker_dir, "docker directory")?;
    ensure_exists(&project.env_file, "env file")?;

    let mut command = Command::new("docker");
    command
        .arg("compose")
        .arg("--env-file")
        .arg(&project.env_file)
        .arg("up");

    if args.detach {
        command.arg("-d");
    }

    let status = command
        .current_dir(&project.docker_dir)
        .status()
        .context("failed to run docker compose up")?;

    if !status.success() {
        bail!("docker compose up failed with status: {status}");
    }

    let mut state = CliState::load_or_default(project_dir)?;
    state.local_running = true;
    state.save(project_dir)?;

    println!("Self-host stack is up for project '{}'.", project_id);
    Ok(())
}

pub fn down(project_dir: &Path, args: SelfHostDownArgs) -> Result<()> {
    let cfg = ProjectConfig::load_from(project_dir)?;
    let (project_id, project) = resolve_project_entry(&cfg, args.project_id.as_deref())?;

    ensure_exists(&project.docker_dir, "docker directory")?;
    ensure_exists(&project.env_file, "env file")?;

    let status = Command::new("docker")
        .arg("compose")
        .arg("--env-file")
        .arg(&project.env_file)
        .arg("down")
        .current_dir(&project.docker_dir)
        .status()
        .context("failed to run docker compose down")?;

    if !status.success() {
        bail!("docker compose down failed with status: {status}");
    }

    let mut state = CliState::load_or_default(project_dir)?;
    state.local_running = false;
    state.save(project_dir)?;

    println!("Self-host stack is down for project '{}'.", project_id);
    Ok(())
}

pub fn status(project_dir: &Path, args: SelfHostStatusArgs) -> Result<()> {
    let cfg = ProjectConfig::load_from(project_dir)?;
    let (project_id, project) = resolve_project_entry(&cfg, args.project_id.as_deref())?;
    let state = CliState::load_or_default(project_dir)?;

    println!("Project: {}", cfg.project.name);
    println!("Self-host project id: {}", project_id);
    println!("Docker directory: {}", project.docker_dir.display());
    println!("Env file: {}", project.env_file.display());
    println!("CLI known running state: {}", state.local_running);

    if !project.docker_dir.exists() || !project.env_file.exists() {
        println!("Docker status: unavailable (missing docker dir or env file)");
        return Ok(());
    }

    let output = Command::new("docker")
        .arg("compose")
        .arg("--env-file")
        .arg(&project.env_file)
        .arg("ps")
        .current_dir(&project.docker_dir)
        .output()
        .context("failed to run docker compose ps")?;

    if output.status.success() {
        println!(
            "Docker status:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    } else {
        println!(
            "Docker status check failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn resolve_project_entry(
    cfg: &ProjectConfig,
    requested_project_id: Option<&str>,
) -> Result<(String, SelfHostProjectSection)> {
    let self_host = cfg
        .self_host
        .as_ref()
        .context("local is not initialized. Run: nuvix local init")?;

    if let Some(project_id) = requested_project_id {
        if let Some(project) = self_host.projects.get(project_id) {
            return Ok((project_id.to_string(), project.clone()));
        }
        bail!("project id '{}' is not configured", project_id);
    }

    if let Some(default_project_id) = &self_host.default_project_id {
        if let Some(project) = self_host.projects.get(default_project_id) {
            return Ok((default_project_id.clone(), project.clone()));
        }
    }

    if self_host.projects.len() == 1 {
        if let Some((project_id, project)) = self_host.projects.iter().next() {
            return Ok((project_id.clone(), project.clone()));
        }
    }

    if let (Some(docker_dir), Some(env_file)) = (&self_host.docker_dir, &self_host.env_file) {
        return Ok((
            "default".to_string(),
            SelfHostProjectSection {
                docker_dir: docker_dir.clone(),
                env_file: env_file.clone(),
            },
        ));
    }

    bail!("multiple local projects found; pass --project-id")
}

fn env_values_from_non_interactive(
    cfg: &ProjectConfig,
    args: &SelfHostInitArgs,
) -> Result<BTreeMap<String, String>> {
    let project_id = required(
        args.project_id
            .clone()
            .or_else(|| Some(cfg.project.name.clone())),
        "--project-id",
    )?;
    let host = required(args.host.clone(), "--host")?;
    let api_port = args.api_port.unwrap_or(DEFAULT_API_PORT);
    let console_api_port = args.console_api_port.unwrap_or(DEFAULT_CONSOLE_API_PORT);
    let console_port = args.console_port.unwrap_or(DEFAULT_CONSOLE_PORT);
    let database_port = args.database_port.unwrap_or(DEFAULT_DATABASE_PORT);
    validate_unique_ports([api_port, console_api_port, console_port, database_port])?;
    let admin_email = required(args.admin_email.clone(), "--admin-email")?;

    let db_password = required(args.database_password.clone(), "--database-password")?;
    let admin_password = required(args.admin_password.clone(), "--admin-password")?;
    let jwt_secret = required(args.jwt_secret.clone(), "--jwt-secret")?;
    let encryption_key = required(args.encryption_key.clone(), "--encryption-key")?;
    let redis_password = required(args.redis_password.clone(), "--redis-password")?;

    let mut vars = base_defaults();
    vars.insert("NUVIX_PROJECT_ID".to_string(), project_id);
    vars.insert("NUVIX_HOST".to_string(), host);
    vars.insert("NUVIX_API_PORT".to_string(), api_port.to_string());
    vars.insert(
        "NUVIX_CONSOLE_API_PORT".to_string(),
        console_api_port.to_string(),
    );
    vars.insert("NUVIX_CONSOLE_PORT".to_string(), console_port.to_string());
    vars.insert("NUVIX_DATABASE_PORT".to_string(), database_port.to_string());
    vars.insert("NUVIX_ADMIN_EMAIL".to_string(), admin_email);
    vars.insert(
        "NUVIX_CONSOLE_URL".to_string(),
        format!("http://{}:{}", vars["NUVIX_HOST"], console_port),
    );
    vars.insert(
        "NUVIX_API_ENDPOINT".to_string(),
        format!("http://{}:{}/v1", vars["NUVIX_HOST"], api_port),
    );
    vars.insert(
        "NUVIX_CONSOLE_API_ENDPOINT".to_string(),
        format!("http://{}:{}", vars["NUVIX_HOST"], console_api_port),
    );
    vars.insert(
        "NUVIX_CORS_ORIGIN".to_string(),
        vars["NUVIX_CONSOLE_URL"].clone(),
    );

    vars.insert("NUVIX_DATABASE_PASSWORD".to_string(), db_password);
    vars.insert("NUVIX_ADMIN_PASSWORD".to_string(), admin_password);
    vars.insert("NUVIX_JWT_SECRET".to_string(), jwt_secret);
    vars.insert("NUVIX_ENCRYPTION_KEY".to_string(), encryption_key);
    vars.insert("NUVIX_REDIS_PASSWORD".to_string(), redis_password);

    vars.insert(
        "NUVIX_REDIS_HOST".to_string(),
        args.redis_host
            .clone()
            .unwrap_or_else(|| "redis".to_string()),
    );
    vars.insert(
        "NUVIX_REDIS_PORT".to_string(),
        args.redis_port.unwrap_or(6379).to_string(),
    );

    Ok(vars)
}

fn env_values_from_interactive(
    cfg: &ProjectConfig,
    args: &SelfHostInitArgs,
) -> Result<BTreeMap<String, String>> {
    let theme = ColorfulTheme::default();
    let default_project_id = args
        .project_id
        .clone()
        .unwrap_or_else(|| cfg.project.name.clone());

    let project_id: String = Input::with_theme(&theme)
        .with_prompt("NUVIX_PROJECT_ID")
        .default(default_project_id)
        .interact_text()?;

    let nuvix_host: String = Input::with_theme(&theme)
        .with_prompt("NUVIX_HOST")
        .default(args.host.clone().unwrap_or_else(|| "localhost".to_string()))
        .interact_text()?;

    let mut reserved_ports = HashSet::new();
    let api_port_default = choose_default_port(
        args.api_port.unwrap_or(DEFAULT_API_PORT),
        &mut reserved_ports,
    );
    let api_port: u16 = Input::with_theme(&theme)
        .with_prompt("NUVIX_API_PORT")
        .default(api_port_default)
        .interact_text()?;
    reserved_ports.insert(api_port);

    let console_api_port_default = choose_default_port(
        args.console_api_port.unwrap_or(DEFAULT_CONSOLE_API_PORT),
        &mut reserved_ports,
    );
    let console_api_port: u16 = Input::with_theme(&theme)
        .with_prompt("NUVIX_CONSOLE_API_PORT")
        .default(console_api_port_default)
        .interact_text()?;
    reserved_ports.insert(console_api_port);

    let console_port_default = choose_default_port(
        args.console_port.unwrap_or(DEFAULT_CONSOLE_PORT),
        &mut reserved_ports,
    );
    let console_port: u16 = Input::with_theme(&theme)
        .with_prompt("NUVIX_CONSOLE_PORT")
        .default(console_port_default)
        .interact_text()?;
    reserved_ports.insert(console_port);

    let database_port_default = choose_default_port(
        args.database_port.unwrap_or(DEFAULT_DATABASE_PORT),
        &mut reserved_ports,
    );
    let database_port: u16 = Input::with_theme(&theme)
        .with_prompt("NUVIX_DATABASE_PORT")
        .default(database_port_default)
        .interact_text()?;
    validate_unique_ports([api_port, console_api_port, console_port, database_port])?;

    let admin_email: String = Input::with_theme(&theme)
        .with_prompt("NUVIX_ADMIN_EMAIL")
        .default(
            args.admin_email
                .clone()
                .unwrap_or_else(|| "admin@nuvix.in".to_string()),
        )
        .interact_text()?;

    let admin_password: String = match args.admin_password.clone() {
        Some(value) => value,
        None => Password::with_theme(&theme)
            .with_prompt("NUVIX_ADMIN_PASSWORD")
            .with_confirmation("Confirm admin password", "Passwords did not match")
            .interact()?,
    };

    let db_password: String = match args.database_password.clone() {
        Some(value) => value,
        None => Password::with_theme(&theme)
            .with_prompt("NUVIX_DATABASE_PASSWORD")
            .with_confirmation("Confirm database password", "Passwords did not match")
            .interact()?,
    };

    let redis_password: String = match args.redis_password.clone() {
        Some(value) => value,
        None => Password::with_theme(&theme)
            .with_prompt("NUVIX_REDIS_PASSWORD")
            .with_confirmation("Confirm redis password", "Passwords did not match")
            .interact()?,
    };

    let jwt_secret = args.jwt_secret.clone().unwrap_or_else(generate_secret);
    let encryption_key = args
        .encryption_key
        .clone()
        .unwrap_or_else(generate_secret_32);

    println!("Generated NUVIX_JWT_SECRET and NUVIX_ENCRYPTION_KEY automatically.");

    if !Confirm::with_theme(&theme)
        .with_prompt("Write docker .env now?")
        .default(true)
        .interact()?
    {
        bail!("aborted by user");
    }

    let mut vars = base_defaults();
    vars.insert("NUVIX_PROJECT_ID".to_string(), project_id);
    vars.insert("NUVIX_HOST".to_string(), nuvix_host);
    vars.insert("NUVIX_API_PORT".to_string(), api_port.to_string());
    vars.insert(
        "NUVIX_CONSOLE_API_PORT".to_string(),
        console_api_port.to_string(),
    );
    vars.insert("NUVIX_CONSOLE_PORT".to_string(), console_port.to_string());
    vars.insert("NUVIX_DATABASE_PORT".to_string(), database_port.to_string());
    vars.insert(
        "NUVIX_CONSOLE_URL".to_string(),
        format!("http://{}:{}", vars["NUVIX_HOST"], console_port),
    );
    vars.insert(
        "NUVIX_API_ENDPOINT".to_string(),
        format!("http://{}:{}/v1", vars["NUVIX_HOST"], api_port),
    );
    vars.insert(
        "NUVIX_CONSOLE_API_ENDPOINT".to_string(),
        format!("http://{}:{}", vars["NUVIX_HOST"], console_api_port),
    );
    vars.insert(
        "NUVIX_CORS_ORIGIN".to_string(),
        vars["NUVIX_CONSOLE_URL"].clone(),
    );
    vars.insert("NUVIX_ADMIN_EMAIL".to_string(), admin_email);
    vars.insert("NUVIX_ADMIN_PASSWORD".to_string(), admin_password);
    vars.insert("NUVIX_DATABASE_PASSWORD".to_string(), db_password);
    vars.insert("NUVIX_REDIS_PASSWORD".to_string(), redis_password);
    vars.insert("NUVIX_JWT_SECRET".to_string(), jwt_secret);
    vars.insert("NUVIX_ENCRYPTION_KEY".to_string(), encryption_key);

    Ok(vars)
}

fn base_defaults() -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    vars.insert("NODE_ENV".to_string(), "production".to_string());
    vars.insert("NUVIX_NAME".to_string(), "Nuvix".to_string());
    vars.insert("NUVIX_PROJECT_ID".to_string(), "default".to_string());
    vars.insert("NUVIX_HOST".to_string(), "localhost".to_string());
    vars.insert(
        "NUVIX_CONSOLE_URL".to_string(),
        "http://localhost:3000".to_string(),
    );
    vars.insert("NUVIX_FORCE_HTTPS".to_string(), "disabled".to_string());
    vars.insert("NUVIX_API_PORT".to_string(), "4000".to_string());
    vars.insert("NUVIX_CONSOLE_API_PORT".to_string(), "4100".to_string());
    vars.insert("NUVIX_CONSOLE_PORT".to_string(), "3000".to_string());
    vars.insert(
        "NUVIX_API_ENDPOINT".to_string(),
        "http://localhost:4000/v1".to_string(),
    );
    vars.insert(
        "NUVIX_CONSOLE_API_ENDPOINT".to_string(),
        "http://localhost:4100".to_string(),
    );
    vars.insert(
        "NUVIX_ADMIN_EMAIL".to_string(),
        "admin@nuvix.in".to_string(),
    );
    vars.insert(
        "NUVIX_SMTP_EMAIL_FROM".to_string(),
        "noreply@nuvix.in".to_string(),
    );
    vars.insert("NUVIX_SMTP_SENDER".to_string(), "Nuvix System".to_string());
    vars.insert(
        "NUVIX_SMTP_REPLY_TO".to_string(),
        "support@nuvix.in".to_string(),
    );
    vars.insert(
        "NUVIX_SYSTEM_EMAIL_ADDRESS".to_string(),
        "support@nuvix.in".to_string(),
    );
    vars.insert(
        "NUVIX_EMAIL_TEAM".to_string(),
        "team@localhost.test".to_string(),
    );
    vars.insert("NUVIX_EMAIL_SECURITY".to_string(), "".to_string());
    vars.insert("NUVIX_REDIS_HOST".to_string(), "redis".to_string());
    vars.insert("NUVIX_REDIS_PORT".to_string(), "6379".to_string());
    vars.insert("NUVIX_REDIS_USER".to_string(), "default".to_string());
    vars.insert("NUVIX_REDIS_DB".to_string(), "0".to_string());
    vars.insert("NUVIX_REDIS_SECURE".to_string(), "false".to_string());
    vars.insert("NUVIX_DATABASE_HOST".to_string(), "localhost".to_string());
    vars.insert("NUVIX_DATABASE_PORT".to_string(), "5432".to_string());
    vars.insert("NUVIX_DATABASE_USER".to_string(), "postgres".to_string());
    vars.insert("NUVIX_DATABASE_SSL".to_string(), "false".to_string());
    vars.insert(
        "NUVIX_DATABASE_MAX_CONNECTIONS".to_string(),
        "60".to_string(),
    );
    vars.insert(
        "NUVIX_DATABASE_QUERY_TIMEOUT".to_string(),
        "30000".to_string(),
    );
    vars.insert(
        "NUVIX_DATABASE_IDLE_TIMEOUT".to_string(),
        "30000".to_string(),
    );
    vars.insert(
        "NUVIX_DATABASE_CONNECTION_TIMEOUT".to_string(),
        "10000".to_string(),
    );
    vars.insert(
        "NUVIX_DATABASE_STATEMENT_TIMEOUT".to_string(),
        "30000".to_string(),
    );
    vars.insert(
        "NUVIX_CORS_ORIGIN".to_string(),
        "http://localhost:3000".to_string(),
    );
    vars.insert("NUVIX_CORS_HEADERS".to_string(), "".to_string());
    vars.insert("NUVIX_COOKIE_DOMAIN".to_string(), "localhost".to_string());
    vars.insert("NUVIX_COOKIE_SAMESITE".to_string(), "lax".to_string());
    vars.insert("NUVIX_COOKIE_NAME".to_string(), "session".to_string());
    vars.insert("NUVIX_SMTP_HOST".to_string(), "".to_string());
    vars.insert("NUVIX_SMTP_PORT".to_string(), "587".to_string());
    vars.insert("NUVIX_SMTP_SECURE".to_string(), "false".to_string());
    vars.insert("NUVIX_SMTP_USER".to_string(), "".to_string());
    vars.insert("NUVIX_SMTP_PASSWORD".to_string(), "".to_string());
    vars.insert("NUVIX_ASSETS_ROOT".to_string(), "assets".to_string());
    vars.insert("NUVIX_ASSETS_PUBLIC".to_string(), "public".to_string());
    vars.insert(
        "NUVIX_STORAGE_MAX_SIZE".to_string(),
        "5000000000".to_string(),
    );
    vars.insert("NUVIX_STORAGE_LIMIT".to_string(), "10000000000".to_string());
    vars.insert("NUVIX_LIMIT_PAGING".to_string(), "12".to_string());
    vars.insert("NUVIX_LIMIT_USERS".to_string(), "10000".to_string());
    vars.insert(
        "NUVIX_LIMIT_USER_PASSWORD_HISTORY".to_string(),
        "10".to_string(),
    );
    vars.insert(
        "NUVIX_LIMIT_USER_SESSIONS_MAX".to_string(),
        "100".to_string(),
    );
    vars.insert(
        "NUVIX_LIMIT_USER_SESSIONS_DEFAULT".to_string(),
        "10".to_string(),
    );
    vars.insert("NUVIX_BATCH_SIZE".to_string(), "2000".to_string());
    vars.insert("NUVIX_BATCH_INTERVAL_MS".to_string(), "5000".to_string());
    vars.insert("NUVIX_CACHE_UPDATE".to_string(), "86400".to_string());
    vars.insert("NUVIX_ENABLE_API_LOGS".to_string(), "true".to_string());
    vars.insert("NUVIX_ENABLE_STATS".to_string(), "true".to_string());
    vars.insert("NUVIX_ENABLE_THROTTLING".to_string(), "true".to_string());
    vars.insert("NUVIX_LOG_LEVELS".to_string(), "log,error,warn".to_string());
    vars.insert("NUVIX_DEBUG_COLORS".to_string(), "true".to_string());
    vars.insert("NUVIX_DEBUG_JSON".to_string(), "false".to_string());
    vars.insert("NUVIX_DEBUG_ERRORS".to_string(), "false".to_string());
    vars.insert(
        "NUVIX_DEBUG_FALLBACK_COOKIES".to_string(),
        "false".to_string(),
    );
    vars
}

fn write_aligned_env_file(
    docker_dir: &Path,
    env_file: &Path,
    overrides: &BTreeMap<String, String>,
    force: bool,
) -> Result<()> {
    if env_file.exists() && !force {
        bail!(
            "env file already exists at {}. Use --force to overwrite.",
            env_file.display()
        );
    }

    let example_file = docker_dir.join(DEFAULT_EXAMPLE_FILE_NAME);
    let content = if example_file.exists() {
        let template = fs::read_to_string(&example_file)
            .with_context(|| format!("failed to read {}", example_file.display()))?;
        merge_template_with_overrides(&template, overrides)
    } else {
        render_plain_env(overrides)
    };

    if let Some(parent) = env_file.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    fs::write(env_file, content)
        .with_context(|| format!("failed to write env file: {}", env_file.display()))
}

fn merge_template_with_overrides(template: &str, overrides: &BTreeMap<String, String>) -> String {
    let mut output = Vec::new();

    for line in template.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            output.push(line.to_string());
            continue;
        }

        if let Some((key, _value)) = line.split_once('=') {
            let key = key.trim();
            if let Some(new_value) = overrides.get(key) {
                output.push(format!("{}={}", key, format_env_value(new_value)));
                continue;
            }
        }

        output.push(line.to_string());
    }

    output.push(String::new());
    output.join("\n")
}

fn render_plain_env(values: &BTreeMap<String, String>) -> String {
    let mut lines = Vec::new();
    for (key, value) in values {
        lines.push(format!("{}={}", key, format_env_value(value)));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_env_value(value: &str) -> String {
    if value.contains(' ') || value.contains('#') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn required(value: Option<String>, flag: &str) -> Result<String> {
    value.with_context(|| format!("missing required value for {flag} in --non-interactive mode"))
}

fn validate_unique_ports<const N: usize>(ports: [u16; N]) -> Result<()> {
    let mut seen = HashSet::new();
    for port in ports {
        if !seen.insert(port) {
            bail!(
                "port conflict detected: port {} is used more than once",
                port
            );
        }
    }
    Ok(())
}

fn choose_default_port(preferred: u16, reserved: &mut HashSet<u16>) -> u16 {
    let mut candidate = preferred;
    loop {
        if !reserved.contains(&candidate) && is_port_available(candidate) {
            return candidate;
        }
        candidate = candidate.saturating_add(1);
        if candidate == 0 {
            return preferred;
        }
    }
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn resolve_path(project_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

fn ensure_exists(path: &Path, label: &str) -> Result<()> {
    if !path.exists() {
        bail!("{label} not found: {}", path.display());
    }
    Ok(())
}

fn clone_docker_repo(repo: &str, docker_dir: &Path) -> Result<()> {
    let parent = docker_dir
        .parent()
        .context("invalid docker directory path (missing parent)")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create directory: {}", parent.display()))?;

    println!("Docker directory not found. Cloning {} ...", repo);

    let status = Command::new("git")
        .arg("clone")
        .arg(repo)
        .arg(docker_dir)
        .status()
        .context("failed to run git clone")?;

    if !status.success() {
        bail!("git clone failed with status: {status}");
    }

    Ok(())
}

fn pull_docker_repo_if_git(docker_dir: &Path) -> Result<()> {
    let is_repo = Command::new("git")
        .arg("-C")
        .arg(docker_dir)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .context("failed to detect git repository for docker_dir")?;

    if !is_repo.status.success() {
        return Ok(());
    }

    println!("Updating docker directory with git pull --ff-only ...");

    let status = Command::new("git")
        .arg("-C")
        .arg(docker_dir)
        .arg("pull")
        .arg("--ff-only")
        .status()
        .context("failed to run git pull")?;

    if !status.success() {
        bail!("git pull failed with status: {status}");
    }

    Ok(())
}

fn register_global_project_profile(
    project_id: &str,
    docker_dir: &Path,
    env_file: &Path,
    env_values: &BTreeMap<String, String>,
) -> Result<()> {
    let mut global = GlobalConfig::load_or_default()?;
    let profile = global
        .projects
        .entry(project_id.to_string())
        .or_insert_with(GlobalProjectProfile::default);

    profile.api_url = env_values.get("NUVIX_API_ENDPOINT").cloned();
    profile.console_api_url = env_values.get("NUVIX_CONSOLE_API_ENDPOINT").cloned();
    profile.console_url = env_values.get("NUVIX_CONSOLE_URL").cloned();
    profile.self_host_docker_dir = Some(docker_dir.to_path_buf());
    profile.self_host_env_file = Some(env_file.to_path_buf());

    global.current_project_id = Some(project_id.to_string());
    global.save()
}

fn generate_secret() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

fn generate_secret_32() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}
