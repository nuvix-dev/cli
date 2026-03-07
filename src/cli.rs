use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "nuvix")]
#[command(about = "CLI for managing Nuvix BaaS projects")]
#[command(version)]
pub struct Cli {
    /// Path to project directory (defaults to current directory)
    #[arg(global = true, short, long)]
    pub project_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Manage self-hosted Nuvix deployment
    SelfHost {
        #[command(subcommand)]
        command: SelfHostCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SelfHostCommand {
    /// Initialize self-host config and write .env file
    Init(SelfHostInitArgs),
    /// Start self-hosted services via Docker Compose
    Up(SelfHostUpArgs),
    /// Stop self-hosted services
    Down(SelfHostDownArgs),
    /// Show current self-host status
    Status(SelfHostStatusArgs),
}

#[derive(Debug, Args)]
pub struct SelfHostInitArgs {
    /// Run without interactive prompts (all values must be provided via flags)
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,

    /// Overwrite existing env file
    #[arg(long, default_value_t = false)]
    pub force: bool,

    /// Pull latest changes if docker_dir already exists and is a git repository
    #[arg(long, default_value_t = true)]
    pub pull: bool,

    /// Path to Nuvix docker directory (relative to project_dir or absolute)
    #[arg(long)]
    pub docker_dir: Option<PathBuf>,

    /// Docker repository URL to clone when docker_dir does not exist
    #[arg(long)]
    pub docker_repo: Option<String>,

    /// Output env file path (relative to project_dir or absolute)
    #[arg(long)]
    pub env_file: Option<PathBuf>,

    #[arg(long)]
    pub project_id: Option<String>,

    #[arg(long)]
    pub host: Option<String>,

    #[arg(long)]
    pub console_url: Option<String>,

    #[arg(long)]
    pub api_endpoint: Option<String>,

    #[arg(long)]
    pub console_api_endpoint: Option<String>,

    #[arg(long)]
    pub admin_email: Option<String>,

    #[arg(long)]
    pub admin_password: Option<String>,

    #[arg(long)]
    pub jwt_secret: Option<String>,

    #[arg(long)]
    pub database_password: Option<String>,

    #[arg(long)]
    pub encryption_key: Option<String>,

    #[arg(long)]
    pub redis_host: Option<String>,

    #[arg(long)]
    pub redis_port: Option<u16>,

    #[arg(long)]
    pub redis_password: Option<String>,
}

#[derive(Debug, Args)]
pub struct SelfHostUpArgs {
    /// Run Docker Compose in detached mode
    #[arg(long, default_value_t = true)]
    pub detach: bool,

    /// Target project id from self-host project dictionary
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct SelfHostDownArgs {
    /// Target project id from self-host project dictionary
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct SelfHostStatusArgs {
    /// Target project id from self-host project dictionary
    #[arg(long)]
    pub project_id: Option<String>,
}
