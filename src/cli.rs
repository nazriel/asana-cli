use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "asana",
    version,
    about = "Read Asana data from the terminal",
    long_about = "A small, agent-friendly CLI for reading tasks, comments, projects, and attachments from Asana.\n\nAuthenticate with `asana auth login` or set ASANA_ACCESS_TOKEN for one process."
)]
pub struct Cli {
    /// Emit structured JSON on stdout.
    #[arg(long, global = true)]
    pub json: bool,

    /// Override the configured Asana workspace GID for this command.
    #[arg(long, global = true, value_name = "GID")]
    pub workspace: Option<String>,

    /// Print basic request diagnostics to stderr. Secrets are always redacted.
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage the locally stored Asana personal access token.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// List and select Asana workspaces.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Read projects in a workspace.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Read sections in a project.
    Section {
        #[command(subcommand)]
        command: SectionCommand,
    },
    /// Read and search tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Inspect and download task attachments.
    Attachment {
        #[command(subcommand)]
        command: AttachmentCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Store a personal access token in the operating-system keyring.
    Login,
    /// Show the authenticated user and token source.
    Status,
    /// Remove the locally stored token from the operating-system keyring.
    Logout,
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// List workspaces visible to the authenticated user.
    List(PaginationArgs),
    /// Verify and save the default workspace GID.
    Use {
        /// Workspace or organization GID.
        gid: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// List projects in the selected workspace.
    List(ProjectListArgs),
}

#[derive(Debug, Subcommand)]
pub enum SectionCommand {
    /// List sections in a project.
    List(SectionListArgs),
}

#[derive(Debug, Args)]
pub struct SectionListArgs {
    /// Project GID.
    #[arg(long, value_name = "GID")]
    pub project: String,

    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct ProjectListArgs {
    /// Return only archived projects.
    #[arg(long, conflicts_with = "all_projects")]
    pub archived: bool,

    /// Do not filter projects by archived state.
    #[arg(long, conflicts_with = "archived")]
    pub all_projects: bool,

    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// List tasks. With no filter, lists the current user's open tasks.
    List(TaskListArgs),
    /// Search tasks in the selected workspace.
    Search(TaskSearchArgs),
    /// Show one task, optionally including all comments or attachment metadata.
    Get(TaskGetArgs),
    /// List human comments on a task.
    Comments(TaskCommentsArgs),
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    /// Limit results to one project GID.
    #[arg(long)]
    pub project: Option<String>,

    /// Limit results to one section GID.
    #[arg(long, conflicts_with_all = ["project", "assignee"])]
    pub section: Option<String>,

    /// Limit results to an assignee GID, email, or `me`.
    #[arg(long, conflicts_with = "section")]
    pub assignee: Option<String>,

    /// Include completed tasks. By default only open tasks are returned.
    #[arg(long)]
    pub include_completed: bool,

    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct TaskSearchArgs {
    /// Text searched in the task name and description.
    pub query: String,

    /// Limit results to an assignee GID, email, or `me`.
    #[arg(long)]
    pub assignee: Option<String>,

    /// Limit results to a project GID.
    #[arg(long)]
    pub project: Option<String>,

    /// Return only completed tasks.
    #[arg(long, conflicts_with = "incomplete")]
    pub completed: bool,

    /// Return only incomplete tasks.
    #[arg(long, conflicts_with = "completed")]
    pub incomplete: bool,

    /// Return tasks due before this ISO-8601 date (YYYY-MM-DD).
    #[arg(long, value_parser = parse_date)]
    pub due_before: Option<String>,

    /// Return tasks due after this ISO-8601 date (YYYY-MM-DD).
    #[arg(long, value_parser = parse_date)]
    pub due_after: Option<String>,

    /// Return tasks created before this ISO-8601 date (YYYY-MM-DD).
    #[arg(long, value_parser = parse_date)]
    pub created_before: Option<String>,

    /// Return tasks created after this ISO-8601 date (YYYY-MM-DD).
    #[arg(long, value_parser = parse_date)]
    pub created_after: Option<String>,

    /// Return tasks modified before this ISO-8601 date (YYYY-MM-DD).
    #[arg(long, value_parser = parse_date)]
    pub modified_before: Option<String>,

    /// Return tasks modified after this ISO-8601 date (YYYY-MM-DD).
    #[arg(long, value_parser = parse_date)]
    pub modified_after: Option<String>,

    /// Result sort order accepted by Asana's search endpoint.
    #[arg(long, value_enum)]
    pub sort: Option<TaskSort>,

    /// Sort ascending instead of the Asana default descending order.
    #[arg(long, requires = "sort")]
    pub ascending: bool,

    #[command(flatten)]
    pub limit: LimitArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TaskSort {
    Relevance,
    DueDate,
    CreatedAt,
    CompletedAt,
    ModifiedAt,
}

impl TaskSort {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::DueDate => "due_date",
            Self::CreatedAt => "created_at",
            Self::CompletedAt => "completed_at",
            Self::ModifiedAt => "modified_at",
        }
    }
}

#[derive(Debug, Args)]
pub struct TaskGetArgs {
    /// Task GID.
    pub gid: String,

    /// Include all human comments in the response.
    #[arg(long)]
    pub comments: bool,

    /// Include all attachment metadata in the response.
    #[arg(long)]
    pub attachments: bool,
}

#[derive(Debug, Args)]
pub struct TaskCommentsArgs {
    /// Task GID.
    pub gid: String,

    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Subcommand)]
pub enum AttachmentCommand {
    /// List attachments on a task.
    List(AttachmentListArgs),
    /// Download a single attachment by GID.
    Download(AttachmentDownloadArgs),
    /// Download every attachment on a task into a directory.
    DownloadAll(AttachmentDownloadAllArgs),
}

#[derive(Debug, Args)]
pub struct AttachmentListArgs {
    /// Task GID.
    pub task_gid: String,

    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct AttachmentDownloadArgs {
    /// Attachment GID.
    pub attachment_gid: String,

    /// Output file path. Defaults to a safe version of the attachment's name.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub struct AttachmentDownloadAllArgs {
    /// Task GID.
    pub task_gid: String,

    /// Output directory. It is created when it does not exist.
    #[arg(short, long, default_value = ".", value_name = "DIRECTORY")]
    pub directory: std::path::PathBuf,
}

#[derive(Debug, Args, Clone)]
pub struct PaginationArgs {
    /// Items per page (1-100).
    #[arg(long, default_value_t = 50, value_parser = parse_limit)]
    pub limit: u8,

    /// Offset from a prior collection response.
    #[arg(long)]
    pub offset: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct LimitArgs {
    /// Maximum result count (1-100).
    #[arg(long, default_value_t = 50, value_parser = parse_limit)]
    pub limit: u8,
}

fn parse_limit(value: &str) -> Result<u8, String> {
    let parsed = value
        .parse::<u8>()
        .map_err(|_| "limit must be an integer from 1 to 100".to_owned())?;

    if (1..=100).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("limit must be an integer from 1 to 100".to_owned())
    }
}

fn parse_date(value: &str) -> Result<String, String> {
    let valid = value.len() == 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());

    if valid {
        Ok(value.to_owned())
    } else {
        Err("date must use ISO-8601 format YYYY-MM-DD".to_owned())
    }
}
