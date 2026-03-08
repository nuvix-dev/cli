use clap::{Args, Parser, Subcommand, ValueEnum};
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
    /// Initialize Nuvix config in a client project directory
    Init(InitArgs),
    /// Manage local Nuvix deployment
    Local {
        #[command(subcommand)]
        command: LocalCommand,
    },
    /// Manage global project profiles
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Authenticate against a Nuvix project profile
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Generate client artifacts from Nuvix instance
    Gen {
        #[command(subcommand)]
        command: GenCommand,
    },
    /// Manage SQL migrations
    Migration {
        #[command(subcommand)]
        command: MigrationCommand,
    },
    /// Manage document schemas and collections
    #[command(alias = "collection")]
    Collections {
        #[command(subcommand)]
        command: CollectionsCommand,
    },
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Project ID to write into nuvix/config.toml (will prompt if omitted)
    #[arg(long)]
    pub project_id: Option<String>,

    /// Overwrite existing nuvix/config.toml if present
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Debug, Subcommand)]
pub enum LocalCommand {
    /// Initialize local config and write .env file
    Init(SelfHostInitArgs),
    /// Start local services via Docker Compose
    Up(SelfHostUpArgs),
    /// Stop local services
    Down(SelfHostDownArgs),
    /// Show current local status
    Status(SelfHostStatusArgs),
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// Set API and Console URLs for a project profile
    SetUrls(ProjectSetUrlsArgs),
    /// Set current active project profile
    Use(ProjectUseArgs),
    /// Show profiles or current profile details
    Show(ProjectShowArgs),
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Login with email/password and store nc_session
    Login(AuthLoginArgs),
    /// Show current auth status for a project profile
    Status(AuthStatusArgs),
    /// Remove stored nc_session for a project profile
    Logout(AuthLogoutArgs),
}

#[derive(Debug, Subcommand)]
pub enum GenCommand {
    /// Generate database types and write to file
    Types(GenTypesArgs),
}

#[derive(Debug, Subcommand)]
pub enum MigrationCommand {
    /// Create a new migration SQL file
    New(MigrationNewArgs),
    /// Pull remote schema snapshot (managed + unmanaged)
    Pull(MigrationPullArgs),
    /// Apply pending migrations to database
    Up(MigrationUpArgs),
    /// Show migration status
    Status(MigrationStatusArgs),
}

#[derive(Debug, Subcommand)]
pub enum CollectionsCommand {
    /// Create a new document schema JSON file
    Init(CollectionsInitArgs),
    /// List document schema files
    List(CollectionsListArgs),
    /// Print a schema JSON file
    Show(CollectionsShowArgs),
    /// Add a collection in a schema file
    AddCollection(CollectionsAddCollectionArgs),
    /// Remove a collection from a schema file
    RemoveCollection(CollectionsRemoveCollectionArgs),
    /// Add an attribute to a collection
    AddAttribute(CollectionsAddAttributeArgs),
    /// Add an index to a collection
    AddIndex(CollectionsAddIndexArgs),
    /// Pull collections/attributes/indexes from remote schema
    Pull(CollectionsPullArgs),
    /// Push local schema file to remote document schema
    Push(CollectionsPushArgs),
    /// Validate one or all schema files
    Validate(CollectionsValidateArgs),
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
    pub api_port: Option<u16>,

    #[arg(long)]
    pub console_api_port: Option<u16>,

    #[arg(long)]
    pub console_port: Option<u16>,

    #[arg(long)]
    pub database_port: Option<u16>,

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

    /// Target project id from local project dictionary
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct SelfHostDownArgs {
    /// Target project id from local project dictionary
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct SelfHostStatusArgs {
    /// Target project id from local project dictionary
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectSetUrlsArgs {
    #[arg(long)]
    pub project_id: String,
    #[arg(long)]
    pub api_url: String,
    #[arg(long)]
    pub console_api_url: String,
    #[arg(long)]
    pub console_url: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectUseArgs {
    #[arg(long)]
    pub project_id: String,
}

#[derive(Debug, Args)]
pub struct ProjectShowArgs {
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long, default_value_t = false)]
    pub list: bool,
}

#[derive(Debug, Args)]
pub struct AuthLoginArgs {
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long)]
    pub email: String,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Debug, Args)]
pub struct AuthStatusArgs {
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct AuthLogoutArgs {
    #[arg(long)]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TypeLanguage {
    Typescript,
    Go,
    Swift,
}

impl TypeLanguage {
    pub fn as_endpoint_segment(&self) -> &'static str {
        match self {
            Self::Typescript => "typescript",
            Self::Go => "go",
            Self::Swift => "swift",
        }
    }

    pub fn default_filename(&self) -> &'static str {
        match self {
            Self::Typescript => "database.types.ts",
            Self::Go => "database.types.go",
            Self::Swift => "DatabaseTypes.swift",
        }
    }
}

#[derive(Debug, Args)]
pub struct GenTypesArgs {
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long, value_enum)]
    pub language: TypeLanguage,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct MigrationNewArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MigrationUpArgs {
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long)]
    pub database_url: Option<String>,
}

#[derive(Debug, Args)]
pub struct MigrationStatusArgs {
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long)]
    pub database_url: Option<String>,
}

#[derive(Debug, Args)]
pub struct MigrationPullArgs {
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long)]
    pub database_url: Option<String>,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DocumentAttributeType {
    String,
    Integer,
    Float,
    Boolean,
    Datetime,
    Timestamptz,
    Email,
    Url,
    Ip,
    Enum,
    Relationship,
}

impl DocumentAttributeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::Timestamptz => "timestamptz",
            Self::Email => "email",
            Self::Url => "url",
            Self::Ip => "ip",
            Self::Enum => "enum",
            Self::Relationship => "relationship",
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DocumentIndexType {
    Key,
    Unique,
    Fulltext,
}

impl DocumentIndexType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Unique => "unique",
            Self::Fulltext => "fulltext",
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum IndexOrder {
    Asc,
    Desc,
}

impl IndexOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

#[derive(Debug, Args)]
pub struct CollectionsInitArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub force: bool,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CollectionsListArgs {
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CollectionsShowArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CollectionsAddCollectionArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CollectionsRemoveCollectionArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CollectionsAddAttributeArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub collection: Option<String>,
    #[arg(long)]
    pub key: Option<String>,
    #[arg(long, value_enum)]
    pub attribute_type: Option<DocumentAttributeType>,
    #[arg(long, default_value_t = false)]
    pub required: bool,
    #[arg(long, default_value_t = false)]
    pub array: bool,
    #[arg(long)]
    pub size: Option<u32>,
    #[arg(long)]
    pub default: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub elements: Vec<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CollectionsAddIndexArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub collection: Option<String>,
    #[arg(long)]
    pub key: Option<String>,
    #[arg(long, value_enum)]
    pub index_type: Option<DocumentIndexType>,
    #[arg(long, value_delimiter = ',')]
    pub attributes: Vec<String>,
    #[arg(long, value_enum, value_delimiter = ',')]
    pub orders: Vec<IndexOrder>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CollectionsValidateArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CollectionsPullArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CollectionsPushArgs {
    #[arg(long)]
    pub schema: Option<String>,
    #[arg(long)]
    pub project_id: Option<String>,
    #[arg(long)]
    pub dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}
