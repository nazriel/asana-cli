use std::{path::PathBuf, time::Duration};

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "asana",
    version,
    about = "Work with Asana from the terminal",
    long_about = "An agent-friendly CLI for reading Asana and safely calling the complete REST API.\n\nAuthenticate with `asana auth login` or set ASANA_ACCESS_TOKEN for one process."
)]
pub struct Cli {
    /// Emit structured JSON on stdout.
    #[arg(long, global = true)]
    pub json: bool,

    /// Select a named authentication and configuration profile.
    #[arg(long, global = true, value_name = "NAME")]
    pub profile: Option<String>,

    /// Override the configured Asana workspace GID for this command.
    #[arg(long, global = true, value_name = "GID")]
    pub workspace: Option<String>,

    /// Override the default response fields with a comma-separated list.
    #[arg(long, global = true, value_name = "FIELDS")]
    pub fields: Option<String>,

    /// Consume every collection page, subject to --max-items.
    #[arg(long, global = true)]
    pub all: bool,

    /// Maximum items returned when --all is active.
    #[arg(long, global = true, default_value_t = 10_000, requires = "all")]
    pub max_items: usize,

    /// Show the normalized mutation without sending it.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Confirm destructive or high-impact operations non-interactively.
    #[arg(long, global = true)]
    pub yes: bool,

    /// Permit `asana api` to call a path absent from the vendored API catalog.
    #[arg(long, global = true)]
    pub allow_unknown_endpoint: bool,

    /// Per-request timeout, for example 30s or 2m.
    #[arg(long, global = true, default_value = "60s", value_parser = parse_duration)]
    pub request_timeout: Duration,

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
    /// Work with project and task templates.
    ProjectTemplate {
        #[command(subcommand)]
        command: ProjectTemplateCommand,
    },
    /// Work with reusable task templates.
    TaskTemplate {
        #[command(subcommand)]
        command: TaskTemplateCommand,
    },
    /// Read and manage Asana users.
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Read and manage Asana teams.
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Inspect project, team, and workspace memberships.
    Membership {
        #[command(subcommand)]
        command: MembershipCommand,
    },
    /// Request and decide access to Asana resources.
    AccessRequest {
        #[command(subcommand)]
        command: AccessRequestCommand,
    },
    /// Read, create, and delete status updates.
    StatusUpdate {
        #[command(subcommand)]
        command: StatusUpdateCommand,
    },
    /// Manage custom-field schemas, options, and settings.
    CustomField {
        #[command(subcommand)]
        command: CustomFieldCommand,
    },
    /// Inspect workspace custom types.
    CustomType {
        #[command(subcommand)]
        command: CustomTypeCommand,
    },
    /// Manage portfolios and their relationships.
    Portfolio {
        #[command(subcommand)]
        command: PortfolioCommand,
    },
    /// Manage goals, metrics, followers, and custom fields.
    Goal {
        #[command(subcommand)]
        command: GoalCommand,
    },
    /// Inspect and update supporting-work relationships for goals.
    GoalRelationship {
        #[command(subcommand)]
        command: GoalRelationshipCommand,
    },
    /// Inspect and update project placement inside portfolios.
    ProjectPortfolioSetting {
        #[command(subcommand)]
        command: ProjectPortfolioSettingCommand,
    },
    /// Read sections in a project.
    Section {
        #[command(subcommand)]
        command: SectionCommand,
    },
    /// Read and search tasks.
    Task {
        #[command(subcommand)]
        command: Box<TaskCommand>,
    },
    /// Inspect and download task attachments.
    Attachment {
        #[command(subcommand)]
        command: AttachmentCommand,
    },
    /// Create, inspect, update, and delete tags.
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Inspect, update, and delete stories/comments by GID.
    Story {
        #[command(subcommand)]
        command: StoryCommand,
    },
    /// Resolve Asana objects quickly by name.
    Typeahead(TypeaheadArgs),
    /// List reactions on a story or status update.
    Reaction(ReactionListArgs),
    /// Resolve user task-list resources.
    UserTaskList {
        #[command(subcommand)]
        command: UserTaskListCommand,
    },
    /// Manage named authentication/configuration profiles.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Call a REST API endpoint directly with standard safety policies.
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
    /// Inspect and wait for asynchronous Asana jobs.
    Job {
        #[command(subcommand)]
        command: JobCommand,
    },
    /// Install shell completion for the current user.
    Setup(SetupArgs),
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Shell for which completion should be installed. Inferred from $SHELL when omitted.
    #[arg(long, value_enum)]
    pub shell: Option<CompletionShell>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Fish,
    Zsh,
}

impl Cli {
    /// Build the Clap command with every subcommand group sorted alphabetically.
    pub fn command_sorted() -> clap::Command {
        let mut command = Self::command();
        command.build();
        sort_subcommands(command)
    }
}

fn sort_subcommands(command: clap::Command) -> clap::Command {
    command.mut_subcommands(|subcommand| sort_subcommands(subcommand).display_order(0))
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List configured profiles.
    List,
    /// Select the default profile.
    Use {
        /// Profile name.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// Send a GET request.
    Get(ApiReadArgs),
    /// Send a POST request.
    Post(ApiWriteArgs),
    /// Send a PUT request.
    Put(ApiWriteArgs),
    /// Send a DELETE request.
    Delete(ApiDeleteArgs),
    /// Send a multipart POST request containing a file.
    Upload(ApiUploadArgs),
}

#[derive(Debug, Args)]
pub struct ApiReadArgs {
    /// Relative API path, for example /tasks/123.
    pub path: String,

    /// Query parameter as KEY=VALUE. Repeat for multiple parameters.
    #[arg(long = "query", value_parser = crate::input::parse_key_value)]
    pub query: Vec<(String, String)>,
}

#[derive(Debug, Args)]
pub struct ApiWriteArgs {
    /// Relative API path, for example /tasks.
    pub path: String,

    /// JSON object, @file, or - for stdin. It is wrapped in Asana's data envelope.
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,

    /// Query parameter as KEY=VALUE. Repeat for multiple parameters.
    #[arg(long = "query", value_parser = crate::input::parse_key_value)]
    pub query: Vec<(String, String)>,
}

#[derive(Debug, Args)]
pub struct ApiDeleteArgs {
    /// Relative API path, for example /tasks/123.
    pub path: String,

    /// Query parameter as KEY=VALUE. Repeat for multiple parameters.
    #[arg(long = "query", value_parser = crate::input::parse_key_value)]
    pub query: Vec<(String, String)>,
}

#[derive(Debug, Args)]
pub struct ApiUploadArgs {
    /// Relative API path, normally /attachments.
    pub path: String,

    /// File to upload.
    #[arg(long)]
    pub file: PathBuf,

    /// Multipart file field name.
    #[arg(long, default_value = "file")]
    pub file_field: String,

    /// Multipart text field as KEY=VALUE. Repeat for multiple fields.
    #[arg(long = "field", value_parser = crate::input::parse_key_value)]
    pub form_fields: Vec<(String, String)>,

    /// Query parameter as KEY=VALUE. Repeat for multiple parameters.
    #[arg(long = "query", value_parser = crate::input::parse_key_value)]
    pub query: Vec<(String, String)>,
}

#[derive(Debug, Subcommand)]
pub enum JobCommand {
    /// Get one asynchronous job.
    Get { gid: String },
    /// Poll a job until it reaches a terminal state.
    Wait {
        gid: String,
        /// Maximum time to wait, for example 30s or 10m.
        #[arg(long, default_value = "10m", value_parser = parse_duration)]
        timeout: Duration,
        /// Delay between polls.
        #[arg(long, default_value = "2s", value_parser = parse_duration)]
        interval: Duration,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Open the Asana developer console to create a personal access token, then store it in the operating-system keyring.
    Login {
        /// Print the developer console URL instead of opening it in a browser.
        #[arg(long)]
        no_browser: bool,
    },
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
    /// Get one workspace.
    Get { gid: String },
    /// Rename a workspace.
    Update(WorkspaceUpdateArgs),
    /// Add a user to a workspace by email.
    UserAdd(WorkspaceUserArgs),
    /// Remove a user from a workspace.
    UserRemove(WorkspaceUserArgs),
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// List projects in the selected workspace.
    List(ProjectListArgs),
    /// Search projects in the selected workspace.
    Search(ProjectSearchArgs),
    /// Get one project.
    Get { gid: String },
    /// Create a project in a workspace or team.
    Create(ProjectCreateArgs),
    /// Patch a project.
    Update(ProjectUpdateArgs),
    /// Delete a project.
    Delete { gid: String },
    /// Duplicate a project asynchronously.
    Duplicate(ProjectDuplicateArgs),
    /// Get task-count metadata for a project.
    TaskCounts { gid: String },
    /// Add or remove project members.
    Member(ProjectRelationArgs),
    /// Add or remove project followers.
    Follower(ProjectRelationArgs),
    /// Attach or remove a project custom field.
    CustomField(ProjectCustomFieldArgs),
    /// List custom-field settings attached to a project.
    CustomFields {
        gid: String,
        #[command(flatten)]
        pagination: PaginationArgs,
    },
    /// Work with the project brief.
    Brief {
        #[command(subcommand)]
        command: ProjectBriefCommand,
    },
    /// Save a project as an asynchronous template job.
    SaveAsTemplate(ProjectSaveTemplateArgs),
}

#[derive(Debug, Args)]
pub struct WorkspaceUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct WorkspaceUserArgs {
    pub gid: String,
    #[arg(long)]
    pub user: String,
}

#[derive(Debug, Args)]
pub struct ProjectSearchArgs {
    pub text: String,
    #[arg(long)]
    pub team: Option<String>,
    #[arg(long)]
    pub completed: Option<bool>,
    /// Additional verified API query parameter as KEY=VALUE.
    #[arg(long = "filter", value_parser = crate::input::parse_key_value)]
    pub filters: Vec<(String, String)>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct ProjectCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub team: Option<String>,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub notes: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long)]
    pub default_view: Option<String>,
    #[arg(long)]
    pub public: Option<bool>,
    #[arg(long)]
    pub archived: Option<bool>,
    #[arg(long = "custom-field", value_parser = crate::input::parse_json_assignment)]
    pub custom_fields: Vec<(String, serde_json::Value)>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub notes: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long)]
    pub default_view: Option<String>,
    #[arg(long)]
    pub public: Option<bool>,
    #[arg(long, conflicts_with = "unarchive")]
    pub archive: bool,
    #[arg(long, conflicts_with = "archive")]
    pub unarchive: bool,
    #[arg(long = "custom-field", value_parser = crate::input::parse_json_assignment)]
    pub custom_fields: Vec<(String, serde_json::Value)>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectDuplicateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: String,
    #[arg(long = "include")]
    pub include: Vec<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectRelationArgs {
    pub gid: String,
    #[arg(long, value_delimiter = ',', required_unless_present = "remove")]
    pub add: Vec<String>,
    #[arg(long, value_delimiter = ',', conflicts_with = "add")]
    pub remove: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ProjectCustomFieldArgs {
    pub gid: String,
    pub custom_field: String,
    #[arg(long, conflicts_with = "remove")]
    pub important: bool,
    #[arg(long)]
    pub remove: bool,
}

#[derive(Debug, Subcommand)]
pub enum ProjectBriefCommand {
    Get { project: String },
    Create(ProjectBriefCreateArgs),
    Update(ProjectBriefUpdateArgs),
    Delete { gid: String },
}

#[derive(Debug, Args)]
pub struct ProjectBriefCreateArgs {
    #[arg(long)]
    pub project: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "html_text")]
    pub text: Option<String>,
    #[arg(long, conflicts_with = "text")]
    pub html_text: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectBriefUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "html_text")]
    pub text: Option<String>,
    #[arg(long, conflicts_with = "text")]
    pub html_text: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectSaveTemplateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub team: Option<String>,
    #[arg(long, conflicts_with = "private", required_unless_present = "private")]
    pub public: bool,
    #[arg(long, conflicts_with = "public")]
    pub private: bool,
}

#[derive(Debug, Subcommand)]
pub enum ProjectTemplateCommand {
    List(ProjectTemplateListArgs),
    Get { gid: String },
    Delete { gid: String },
    Instantiate(ProjectTemplateInstantiateArgs),
}

#[derive(Debug, Args)]
pub struct ProjectTemplateListArgs {
    #[arg(long)]
    pub team: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct ProjectTemplateInstantiateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub team: Option<String>,
    #[arg(long)]
    pub public: Option<bool>,
    #[arg(long = "requested-date", value_parser = parse_gid_date)]
    pub requested_dates: Vec<(String, String)>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum TaskTemplateCommand {
    List(TaskTemplateListArgs),
    Get { gid: String },
    Delete { gid: String },
    Instantiate(TaskTemplateInstantiateArgs),
}

#[derive(Debug, Args)]
pub struct TaskTemplateListArgs {
    #[arg(long)]
    pub project: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct TaskTemplateInstantiateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    List(UserListArgs),
    Get(UserGetArgs),
    Update(UserUpdateArgs),
    Favorites(UserFavoritesArgs),
}

#[derive(Debug, Args)]
pub struct UserGetArgs {
    pub gid: String,
    /// Use the workspace-scoped user endpoint.
    #[arg(long)]
    pub in_workspace: bool,
}

#[derive(Debug, Args)]
pub struct UserListArgs {
    #[arg(long)]
    pub team: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct UserUpdateArgs {
    pub gid: String,
    /// Update workspace-scoped user fields such as custom fields.
    #[arg(long)]
    pub in_workspace: bool,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct UserFavoritesArgs {
    pub gid: String,
    #[arg(long)]
    #[arg(long, default_value = "project")]
    pub resource_type: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    List(TeamListArgs),
    Get { gid: String },
    Create(TeamCreateArgs),
    Update(TeamUpdateArgs),
    UserAdd(TeamUserArgs),
    UserRemove(TeamUserArgs),
}

#[derive(Debug, Args)]
pub struct TeamListArgs {
    #[arg(long)]
    pub user: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct TeamCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, conflicts_with = "html_description")]
    pub description: Option<String>,
    #[arg(long, conflicts_with = "description")]
    pub html_description: Option<String>,
    #[arg(long)]
    pub organization: Option<String>,
    #[arg(long)]
    pub visibility: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct TeamUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, conflicts_with = "html_description")]
    pub description: Option<String>,
    #[arg(long, conflicts_with = "description")]
    pub html_description: Option<String>,
    #[arg(long)]
    pub visibility: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct TeamUserArgs {
    pub gid: String,
    #[arg(long)]
    pub user: String,
}

#[derive(Debug, Subcommand)]
pub enum MembershipCommand {
    List(GenericMembershipListArgs),
    Get { gid: String },
    Create(GenericMembershipCreateArgs),
    Update(GenericMembershipUpdateArgs),
    Delete { gid: String },
    Project(MembershipListArgs),
    Portfolio(MembershipListArgs),
    Team(MembershipListArgs),
    Workspace(MembershipListArgs),
}

#[derive(Debug, Args)]
pub struct GenericMembershipListArgs {
    #[arg(long)]
    pub parent: Option<String>,
    #[arg(long)]
    pub member: Option<String>,
    #[arg(long)]
    pub resource_subtype: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct GenericMembershipCreateArgs {
    #[arg(long)]
    pub parent: String,
    #[arg(long)]
    pub member: String,
    #[arg(long)]
    pub access_level: Option<String>,
}

#[derive(Debug, Args)]
pub struct GenericMembershipUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub access_level: String,
}

#[derive(Debug, Args)]
pub struct MembershipListArgs {
    #[arg(long)]
    pub gid: Option<String>,
    #[arg(long, conflicts_with = "user")]
    pub parent: Option<String>,
    #[arg(long, conflicts_with = "parent")]
    pub user: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Subcommand)]
pub enum AccessRequestCommand {
    List(AccessRequestListArgs),
    Create(AccessRequestCreateArgs),
    Approve { gid: String },
    Reject { gid: String },
}

#[derive(Debug, Args)]
pub struct AccessRequestListArgs {
    #[arg(long)]
    pub target: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct AccessRequestCreateArgs {
    #[arg(long)]
    pub target: String,
    #[arg(long)]
    pub message: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum StatusUpdateCommand {
    List(StatusUpdateListArgs),
    Get { gid: String },
    Create(StatusUpdateCreateArgs),
    Delete { gid: String },
}

#[derive(Debug, Args)]
pub struct StatusUpdateListArgs {
    #[arg(long)]
    pub parent: String,
    #[arg(long, value_parser = parse_datetime)]
    pub created_since: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct StatusUpdateCreateArgs {
    #[arg(long)]
    pub parent: String,
    #[arg(long)]
    pub status_type: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "html_text")]
    pub text: Option<String>,
    #[arg(long, conflicts_with = "text")]
    pub html_text: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum CustomFieldCommand {
    List(PaginationArgs),
    Create(CustomFieldCreateArgs),
    Get {
        gid: String,
    },
    Update(CustomFieldUpdateArgs),
    Delete {
        gid: String,
    },
    EnumOption {
        #[command(subcommand)]
        command: EnumOptionCommand,
    },
    Settings(CustomFieldSettingsArgs),
}

#[derive(Debug, Args)]
pub struct CustomFieldCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long = "type")]
    pub resource_subtype: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=6))]
    pub precision: Option<u8>,
    #[arg(long)]
    pub format: Option<String>,
    #[arg(long)]
    pub currency_code: Option<String>,
    #[arg(long)]
    pub custom_label: Option<String>,
    #[arg(long)]
    pub notifications: Option<bool>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct CustomFieldUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=6))]
    pub precision: Option<u8>,
    #[arg(long)]
    pub format: Option<String>,
    #[arg(long)]
    pub currency_code: Option<String>,
    #[arg(long)]
    pub custom_label: Option<String>,
    #[arg(long)]
    pub notifications: Option<bool>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum EnumOptionCommand {
    Create(EnumOptionCreateArgs),
    Update(EnumOptionUpdateArgs),
    Move(EnumOptionMoveArgs),
}

#[derive(Debug, Args)]
pub struct EnumOptionCreateArgs {
    #[arg(long)]
    pub custom_field: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long, conflicts_with = "insert_after")]
    pub insert_before: Option<String>,
    #[arg(long, conflicts_with = "insert_before")]
    pub insert_after: Option<String>,
}

#[derive(Debug, Args)]
pub struct EnumOptionUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Args)]
pub struct EnumOptionMoveArgs {
    pub gid: String,
    #[arg(long)]
    pub custom_field: String,
    #[arg(
        long,
        conflicts_with = "after_enum_option",
        required_unless_present = "after_enum_option"
    )]
    pub before_enum_option: Option<String>,
    #[arg(long, conflicts_with = "before_enum_option")]
    pub after_enum_option: Option<String>,
}

#[derive(Debug, Args)]
pub struct CustomFieldSettingsArgs {
    #[arg(value_enum)]
    pub parent_type: StrategicParentType,
    pub parent_gid: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StrategicParentType {
    Project,
    Portfolio,
    Goal,
    Team,
}

impl StrategicParentType {
    pub fn path(self) -> &'static str {
        match self {
            Self::Project => "projects",
            Self::Portfolio => "portfolios",
            Self::Goal => "goals",
            Self::Team => "teams",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CustomTypeCommand {
    List(CustomTypeListArgs),
    Get { gid: String },
}

#[derive(Debug, Args)]
pub struct CustomTypeListArgs {
    #[arg(long)]
    pub project: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Subcommand)]
pub enum PortfolioCommand {
    List(PortfolioListArgs),
    Get {
        gid: String,
    },
    Create(PortfolioCreateArgs),
    Update(PortfolioUpdateArgs),
    Delete {
        gid: String,
    },
    Duplicate(PortfolioDuplicateArgs),
    Item(PortfolioItemArgs),
    Member(PortfolioMemberArgs),
    CustomField(StrategicCustomFieldArgs),
    CustomFields {
        gid: String,
        #[command(flatten)]
        pagination: PaginationArgs,
    },
}

#[derive(Debug, Args)]
pub struct PortfolioListArgs {
    #[arg(long, default_value = "me")]
    pub owner: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct PortfolioCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long, value_parser = parse_date)]
    pub start_on: Option<String>,
    #[arg(long, value_parser = parse_date)]
    pub due_on: Option<String>,
    #[arg(long)]
    pub privacy_setting: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct PortfolioUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long, value_parser = parse_date, conflicts_with = "clear_start")]
    pub start_on: Option<String>,
    #[arg(long, value_parser = parse_date, conflicts_with = "clear_due")]
    pub due_on: Option<String>,
    #[arg(long, conflicts_with = "start_on")]
    pub clear_start: bool,
    #[arg(long, conflicts_with = "due_on")]
    pub clear_due: bool,
    #[arg(long, conflicts_with = "unarchive")]
    pub archive: bool,
    #[arg(long, conflicts_with = "archive")]
    pub unarchive: bool,
    #[arg(long = "custom-field", value_parser = crate::input::parse_json_assignment)]
    pub custom_fields: Vec<(String, serde_json::Value)>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct PortfolioDuplicateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub include: Vec<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct PortfolioItemArgs {
    pub gid: String,
    #[arg(long)]
    pub add: Option<String>,
    #[arg(long, conflicts_with = "add")]
    pub remove: Option<String>,
    #[arg(long, conflicts_with = "insert_after", requires = "add")]
    pub insert_before: Option<String>,
    #[arg(long, conflicts_with = "insert_before", requires = "add")]
    pub insert_after: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct PortfolioMemberArgs {
    pub gid: String,
    #[arg(long, value_delimiter = ',', required_unless_present = "remove")]
    pub add: Vec<String>,
    #[arg(long, value_delimiter = ',', conflicts_with = "add")]
    pub remove: Vec<String>,
}

#[derive(Debug, Args)]
pub struct StrategicCustomFieldArgs {
    pub gid: String,
    pub custom_field: String,
    #[arg(long, conflicts_with = "important")]
    pub remove: bool,
    #[arg(long, conflicts_with = "remove")]
    pub important: bool,
}

#[derive(Debug, Subcommand)]
pub enum GoalCommand {
    List(GoalListArgs),
    Get {
        gid: String,
    },
    Create(GoalCreateArgs),
    Update(GoalUpdateArgs),
    Delete {
        gid: String,
    },
    Metric {
        #[command(subcommand)]
        command: GoalMetricCommand,
    },
    Follower(GoalFollowerArgs),
    Parents {
        gid: String,
        #[command(flatten)]
        pagination: PaginationArgs,
    },
    CustomField(StrategicCustomFieldArgs),
    CustomFields {
        gid: String,
        #[command(flatten)]
        pagination: PaginationArgs,
    },
}

#[derive(Debug, Args)]
pub struct GoalListArgs {
    #[arg(long)]
    pub portfolio: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub task: Option<String>,
    #[arg(long)]
    pub team: Option<String>,
    #[arg(long)]
    pub workspace_level: Option<bool>,
    #[arg(long, value_delimiter = ',')]
    pub time_periods: Vec<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct GoalCreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub team: Option<String>,
    #[arg(long)]
    pub time_period: Option<String>,
    #[arg(long, value_parser = parse_date)]
    pub due_on: Option<String>,
    #[arg(long, value_parser = parse_date)]
    pub start_on: Option<String>,
    #[arg(long, conflicts_with = "html_notes")]
    pub notes: Option<String>,
    #[arg(long, conflicts_with = "notes")]
    pub html_notes: Option<String>,
    #[arg(long)]
    pub workspace_level: Option<bool>,
    #[arg(long = "follower")]
    pub followers: Vec<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct GoalUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, conflicts_with = "clear_owner")]
    pub owner: Option<String>,
    #[arg(long, conflicts_with = "clear_team")]
    pub team: Option<String>,
    #[arg(long, conflicts_with = "clear_time_period")]
    pub time_period: Option<String>,
    #[arg(long, value_parser = parse_date, conflicts_with = "clear_due")]
    pub due_on: Option<String>,
    #[arg(long, value_parser = parse_date, conflicts_with = "clear_start")]
    pub start_on: Option<String>,
    #[arg(long, conflicts_with_all = ["html_notes", "clear_notes"])]
    pub notes: Option<String>,
    #[arg(long, conflicts_with_all = ["notes", "clear_notes"])]
    pub html_notes: Option<String>,
    #[arg(long)]
    pub clear_owner: bool,
    #[arg(long)]
    pub clear_team: bool,
    #[arg(long)]
    pub clear_time_period: bool,
    #[arg(long)]
    pub clear_due: bool,
    #[arg(long)]
    pub clear_start: bool,
    #[arg(long, conflicts_with_all = ["notes", "html_notes"])]
    pub clear_notes: bool,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long = "custom-field", value_parser = crate::input::parse_json_assignment)]
    pub custom_fields: Vec<(String, serde_json::Value)>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum GoalMetricCommand {
    Set(GoalMetricSetArgs),
    Update(GoalMetricUpdateArgs),
}

#[derive(Debug, Args)]
pub struct GoalMetricSetArgs {
    pub gid: String,
    #[arg(long)]
    pub unit: Option<String>,
    #[arg(long, value_parser = parse_finite_number)]
    pub initial: Option<f64>,
    #[arg(long, value_parser = parse_finite_number)]
    pub target: Option<f64>,
    #[arg(long, value_parser = parse_finite_number)]
    pub current: Option<f64>,
    #[arg(long)]
    #[arg(value_parser = clap::value_parser!(u8).range(0..=6))]
    pub precision: Option<u8>,
    #[arg(long)]
    pub currency_code: Option<String>,
    #[arg(long)]
    pub progress_source: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct GoalMetricUpdateArgs {
    pub gid: String,
    #[arg(long, value_parser = parse_finite_number)]
    pub current: f64,
}

#[derive(Debug, Args)]
pub struct GoalFollowerArgs {
    pub gid: String,
    #[arg(long, value_delimiter = ',', required_unless_present = "remove")]
    pub add: Vec<String>,
    #[arg(long, value_delimiter = ',', conflicts_with = "add")]
    pub remove: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum GoalRelationshipCommand {
    List(GoalRelationshipListArgs),
    Get { gid: String },
    Add(GoalRelationshipAddArgs),
    Update(GoalRelationshipUpdateArgs),
    Remove(GoalRelationshipRemoveArgs),
}

#[derive(Debug, Args)]
pub struct GoalRelationshipListArgs {
    #[arg(long)]
    pub supported_goal: String,
    #[arg(long)]
    pub resource_subtype: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct GoalRelationshipAddArgs {
    #[arg(long)]
    pub goal: String,
    #[arg(long)]
    pub supporting_resource: String,
    #[arg(long)]
    #[arg(value_parser = parse_contribution)]
    pub contribution_weight: Option<f64>,
    #[arg(long, conflicts_with = "insert_after")]
    pub insert_before: Option<String>,
    #[arg(long, conflicts_with = "insert_before")]
    pub insert_after: Option<String>,
}

#[derive(Debug, Args)]
pub struct GoalRelationshipUpdateArgs {
    pub gid: String,
    #[arg(long)]
    #[arg(value_parser = parse_contribution)]
    pub contribution_weight: f64,
}

#[derive(Debug, Args)]
pub struct GoalRelationshipRemoveArgs {
    #[arg(long)]
    pub goal: String,
    #[arg(long)]
    pub supporting_resource: String,
}

#[derive(Debug, Subcommand)]
pub enum ProjectPortfolioSettingCommand {
    List(ProjectPortfolioSettingListArgs),
    Get { gid: String },
    Update(ProjectPortfolioSettingUpdateArgs),
}

#[derive(Debug, Args)]
pub struct ProjectPortfolioSettingListArgs {
    #[arg(
        long,
        conflicts_with = "portfolio",
        required_unless_present = "portfolio"
    )]
    pub project: Option<String>,
    #[arg(long, conflicts_with = "project")]
    pub portfolio: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct ProjectPortfolioSettingUpdateArgs {
    pub gid: String,
    #[arg(
        long,
        conflicts_with = "no_inherit_access",
        required_unless_present = "no_inherit_access"
    )]
    pub inherit_access: bool,
    #[arg(long, conflicts_with = "inherit_access")]
    pub no_inherit_access: bool,
}

#[derive(Debug, Subcommand)]
pub enum SectionCommand {
    /// List sections in a project.
    List(SectionListArgs),
    /// Get one section.
    Get { gid: String },
    /// Create a section in a project.
    Create(SectionCreateArgs),
    /// Rename a section.
    Update(SectionUpdateArgs),
    /// Delete an empty section.
    Delete { gid: String },
    /// Move a section before or after another section.
    Move(SectionMoveArgs),
    /// Add or reposition a task in a section.
    TaskAdd(SectionTaskAddArgs),
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
pub struct SectionCreateArgs {
    #[arg(long)]
    pub project: String,
    #[arg(long)]
    pub name: String,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct SectionUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct SectionMoveArgs {
    pub gid: String,
    #[arg(long)]
    pub project: String,
    #[arg(
        long,
        conflicts_with = "after_section",
        required_unless_present = "after_section"
    )]
    pub before_section: Option<String>,
    #[arg(long, conflicts_with = "before_section")]
    pub after_section: Option<String>,
}

#[derive(Debug, Args)]
pub struct SectionTaskAddArgs {
    pub gid: String,
    #[arg(long)]
    pub task: String,
    #[arg(long, conflicts_with_all = ["insert_after", "at_start", "at_end"])]
    pub insert_before: Option<String>,
    #[arg(long, conflicts_with_all = ["insert_before", "at_start", "at_end"])]
    pub insert_after: Option<String>,
    #[arg(long, conflicts_with_all = ["insert_before", "insert_after", "at_end"])]
    pub at_start: bool,
    #[arg(long, conflicts_with_all = ["insert_before", "insert_after", "at_start"])]
    pub at_end: bool,
}

#[derive(Debug, Args)]
pub struct ProjectListArgs {
    /// List projects for this team instead of the selected workspace.
    #[arg(long, conflicts_with = "task")]
    pub team: Option<String>,

    /// List projects containing this task.
    #[arg(long, conflicts_with = "team")]
    pub task: Option<String>,

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
    /// Get a task by its workspace custom-ID shortcode.
    GetCustom { custom_id: String },
    /// List human comments on a task.
    Comments(TaskCommentsArgs),
    /// Create a task.
    Create(TaskCreateArgs),
    /// Patch a task.
    Update(TaskUpdateArgs),
    /// Delete a task to Asana trash.
    Delete { gid: String },
    /// Duplicate a task asynchronously.
    Duplicate(TaskDuplicateArgs),
    /// Work with task comments.
    Comment {
        #[command(subcommand)]
        command: TaskCommentCommand,
    },
    /// Work with subtasks.
    Subtask {
        #[command(subcommand)]
        command: TaskSubtaskCommand,
    },
    /// Set or clear a task parent.
    Parent {
        #[command(subcommand)]
        command: TaskParentCommand,
    },
    /// Work with task dependencies.
    Dependency {
        #[command(subcommand)]
        command: TaskDependencyCommand,
    },
    /// Work with task dependents.
    Dependent {
        #[command(subcommand)]
        command: TaskDependentCommand,
    },
    /// Add or remove task tags.
    Tag {
        #[command(subcommand)]
        command: TaskTagCommand,
    },
    /// Add, remove, or reposition project membership.
    Project {
        #[command(subcommand)]
        command: TaskProjectCommand,
    },
    /// Add or remove task followers.
    Follower {
        #[command(subcommand)]
        command: TaskFollowerCommand,
    },
}

#[derive(Debug, Args)]
pub struct TaskCreateArgs {
    /// Task name. Required unless supplied through --data.
    #[arg(long, required_unless_present = "data")]
    pub name: Option<String>,
    /// Assignee GID, email, or `me`.
    #[arg(long)]
    pub assignee: Option<String>,
    /// Due date without a time.
    #[arg(long, value_parser = parse_date, conflicts_with = "due_at")]
    pub due_on: Option<String>,
    /// Due date-time in ISO-8601 format.
    #[arg(long, conflicts_with = "due_on", value_parser = parse_datetime)]
    pub due_at: Option<String>,
    /// Start date without a time.
    #[arg(long, value_parser = parse_date, conflicts_with = "start_at")]
    pub start_on: Option<String>,
    /// Start date-time in ISO-8601 format.
    #[arg(long, conflicts_with = "start_on", value_parser = parse_datetime)]
    pub start_at: Option<String>,
    /// Plain-text task description.
    #[arg(long, conflicts_with_all = ["notes_file", "html_notes", "html_notes_file"])]
    pub notes: Option<String>,
    /// Read a plain-text description from a file or `-` for stdin.
    #[arg(long, conflicts_with_all = ["notes", "html_notes", "html_notes_file"])]
    pub notes_file: Option<String>,
    /// Asana rich-text description wrapped in `<body>`.
    #[arg(long, conflicts_with_all = ["notes", "notes_file", "html_notes_file"])]
    pub html_notes: Option<String>,
    /// Read Asana rich text from a file or `-` for stdin.
    #[arg(long, conflicts_with_all = ["notes", "notes_file", "html_notes"])]
    pub html_notes_file: Option<String>,
    /// Create the task as a subtask of this task GID.
    #[arg(long, value_parser = parse_datetime)]
    pub parent: Option<String>,
    /// Add the task to a project. Repeat for multiple projects.
    #[arg(long)]
    pub project: Vec<String>,
    /// Place the task in this section. Requires exactly one --project.
    #[arg(long, requires = "project")]
    pub section: Option<String>,
    /// Add a follower. Repeat for multiple users.
    #[arg(long)]
    pub follower: Vec<String>,
    /// Add a tag. Repeat for multiple tags.
    #[arg(long)]
    pub tag: Vec<String>,
    /// Set a custom field as GID=JSON_VALUE. Repeat as needed.
    #[arg(long, value_parser = crate::input::parse_json_assignment)]
    pub custom_field: Vec<(String, serde_json::Value)>,
    /// Additional create fields as JSON, @file, or stdin (`-`).
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskUpdateArgs {
    /// Task GID.
    pub gid: String,
    /// Refuse the update unless the current modified_at matches this value.
    #[arg(long)]
    pub expect_modified_at: Option<String>,
    /// New task name.
    #[arg(long)]
    pub name: Option<String>,
    /// New assignee GID, email, or `me`.
    #[arg(long, conflicts_with = "clear_assignee")]
    pub assignee: Option<String>,
    /// Remove the current assignee.
    #[arg(long, conflicts_with = "assignee")]
    pub clear_assignee: bool,
    /// Set a due date without a time.
    #[arg(long, value_parser = parse_date, conflicts_with_all = ["due_at", "clear_due"])]
    pub due_on: Option<String>,
    /// Set a due date-time.
    #[arg(long, conflicts_with_all = ["due_on", "clear_due"], value_parser = parse_datetime)]
    pub due_at: Option<String>,
    /// Remove both due date representations.
    #[arg(long, conflicts_with_all = ["due_on", "due_at"])]
    pub clear_due: bool,
    #[arg(long, value_parser = parse_date, conflicts_with_all = ["start_at", "clear_start"])]
    pub start_on: Option<String>,
    #[arg(long, conflicts_with_all = ["start_on", "clear_start"], value_parser = parse_datetime)]
    pub start_at: Option<String>,
    #[arg(long, conflicts_with_all = ["start_on", "start_at"])]
    pub clear_start: bool,
    #[arg(long, conflicts_with_all = ["notes_file", "html_notes", "html_notes_file", "clear_notes"])]
    pub notes: Option<String>,
    #[arg(long, conflicts_with_all = ["notes", "html_notes", "html_notes_file", "clear_notes"])]
    pub notes_file: Option<String>,
    #[arg(long, conflicts_with_all = ["notes", "notes_file", "html_notes_file", "clear_notes"])]
    pub html_notes: Option<String>,
    #[arg(long, conflicts_with_all = ["notes", "notes_file", "html_notes", "clear_notes"])]
    pub html_notes_file: Option<String>,
    #[arg(long, conflicts_with_all = ["notes", "notes_file", "html_notes", "html_notes_file"])]
    pub clear_notes: bool,
    /// Mark the task complete.
    #[arg(long, conflicts_with = "incomplete")]
    pub complete: bool,
    /// Reopen the task.
    #[arg(long, conflicts_with = "complete")]
    pub incomplete: bool,
    /// Set a custom field as GID=JSON_VALUE.
    #[arg(long, value_parser = crate::input::parse_json_assignment)]
    pub set_custom_field: Vec<(String, serde_json::Value)>,
    /// Clear a custom-field value by field GID.
    #[arg(long)]
    pub clear_custom_field: Vec<String>,
    /// Additional patch fields as JSON, @file, or stdin (`-`).
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskDuplicateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub include: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    /// Limit results to one project GID.
    #[arg(long)]
    pub project: Option<String>,

    /// Limit results to one section GID.
    #[arg(long, conflicts_with_all = ["project", "assignee"])]
    pub section: Option<String>,

    /// Limit results to one tag GID.
    #[arg(long, conflicts_with_all = ["project", "section", "assignee"])]
    pub tag: Option<String>,

    /// Limit results to one user-task-list GID.
    #[arg(long, conflicts_with_all = ["project", "section", "tag", "assignee"])]
    pub user_task_list: Option<String>,

    /// Limit results to an assignee GID, email, or `me`.
    #[arg(long, conflicts_with_all = ["section", "tag", "user_task_list"])]
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

    /// Limit results to tasks in a section. Repeat or comma-separate GIDs.
    #[arg(long, value_delimiter = ',')]
    pub section: Vec<String>,

    /// Limit results to tasks with a tag. Repeat or comma-separate GIDs.
    #[arg(long, value_delimiter = ',')]
    pub tag: Vec<String>,

    /// Limit results to tasks followed by a user.
    #[arg(long, value_delimiter = ',')]
    pub follower: Vec<String>,

    /// Dynamic custom-field filter: FIELD_GID.OPERATOR=VALUE.
    #[arg(long)]
    pub custom_field: Vec<String>,

    /// Custom task type status filter: CUSTOM_TYPE_GID=STATUS_OPTION_GID.
    #[arg(long)]
    pub custom_type_status: Option<String>,

    /// Additional official search query parameter as KEY=VALUE.
    #[arg(long = "filter", value_parser = crate::input::parse_key_value)]
    pub filter: Vec<(String, String)>,

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
pub enum TaskCommentCommand {
    /// List human comments on a task.
    List(TaskCommentsArgs),
    /// Add a comment to a task.
    Add(CommentCreateArgs),
    /// Update a comment by story GID.
    Update(StoryUpdateArgs),
    /// Delete a comment by story GID.
    Delete { gid: String },
}

#[derive(Debug, Args)]
pub struct CommentCreateArgs {
    pub task_gid: String,
    #[arg(long, conflicts_with_all = ["text_file", "html_text", "html_text_file"], required_unless_present_any = ["text_file", "html_text", "html_text_file"])]
    pub text: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "html_text", "html_text_file"])]
    pub text_file: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "text_file", "html_text_file"])]
    pub html_text: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "text_file", "html_text"])]
    pub html_text_file: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum TaskSubtaskCommand {
    List(RelationListArgs),
    Create(Box<TaskSubtaskCreateArgs>),
}

#[derive(Debug, Args)]
pub struct TaskSubtaskCreateArgs {
    pub task_gid: String,
    #[command(flatten)]
    pub task: TaskCreateArgs,
}

#[derive(Debug, Subcommand)]
pub enum TaskParentCommand {
    Set(TaskParentSetArgs),
    Clear { task_gid: String },
}

#[derive(Debug, Args)]
pub struct TaskParentSetArgs {
    pub task_gid: String,
    #[arg(long)]
    pub parent: String,
    #[arg(long, conflicts_with = "insert_after")]
    pub insert_before: Option<String>,
    #[arg(long, conflicts_with = "insert_before")]
    pub insert_after: Option<String>,
}

#[derive(Debug, Args)]
pub struct RelationListArgs {
    pub task_gid: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct RelationChangeArgs {
    pub task_gid: String,
    #[arg(required = true)]
    pub related_gid: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum TaskDependencyCommand {
    List(RelationListArgs),
    Add(RelationChangeArgs),
    Remove(RelationChangeArgs),
}

#[derive(Debug, Subcommand)]
pub enum TaskDependentCommand {
    List(RelationListArgs),
    Add(RelationChangeArgs),
    Remove(RelationChangeArgs),
}

#[derive(Debug, Subcommand)]
pub enum TaskTagCommand {
    List(RelationListArgs),
    Add(TaskSingleRelationArgs),
    Remove(TaskSingleRelationArgs),
}

#[derive(Debug, Args)]
pub struct TaskSingleRelationArgs {
    pub task_gid: String,
    pub related_gid: String,
}

#[derive(Debug, Subcommand)]
pub enum TaskProjectCommand {
    List(RelationListArgs),
    Add(TaskProjectAddArgs),
    Remove(TaskSingleRelationArgs),
}

#[derive(Debug, Args)]
pub struct TaskProjectAddArgs {
    pub task_gid: String,
    pub project_gid: String,
    #[arg(long)]
    pub section: Option<String>,
    #[arg(long, conflicts_with_all = ["insert_after", "at_start", "at_end"])]
    pub insert_before: Option<String>,
    #[arg(long, conflicts_with_all = ["insert_before", "at_start", "at_end"])]
    pub insert_after: Option<String>,
    #[arg(long, conflicts_with_all = ["insert_before", "insert_after", "at_end"], requires = "section")]
    pub at_start: bool,
    #[arg(long, conflicts_with_all = ["insert_before", "insert_after", "at_start"], requires = "section")]
    pub at_end: bool,
}

#[derive(Debug, Subcommand)]
pub enum TaskFollowerCommand {
    Add(TaskUsersArgs),
    Remove(TaskUsersArgs),
}

#[derive(Debug, Args)]
pub struct TaskUsersArgs {
    pub task_gid: String,
    #[arg(required = true)]
    pub user: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum AttachmentCommand {
    /// Get attachment metadata by GID.
    Get { gid: String },
    /// List attachments on a task, project, or project brief.
    List(AttachmentListArgs),
    /// Download a single attachment by GID.
    Download(AttachmentDownloadArgs),
    /// Download every attachment on a task into a directory.
    DownloadAll(AttachmentDownloadAllArgs),
    /// Upload a local file to a task, project, or project brief.
    Upload(AttachmentUploadArgs),
    /// Attach an external URL.
    AttachUrl(AttachmentUrlArgs),
    /// Delete an attachment.
    Delete { gid: String },
}

#[derive(Debug, Args)]
pub struct AttachmentListArgs {
    /// Parent task, project, or project-brief GID.
    pub parent_gid: String,

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

#[derive(Debug, Args)]
pub struct AttachmentUploadArgs {
    #[arg(long)]
    pub parent: String,
    #[arg(long)]
    pub file: PathBuf,
}

#[derive(Debug, Args)]
pub struct AttachmentUrlArgs {
    #[arg(long)]
    pub parent: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub url: String,
    #[arg(long)]
    pub connect_to_app: bool,
}

#[derive(Debug, Subcommand)]
pub enum TagCommand {
    List(TagListArgs),
    Get { gid: String },
    Create(TagCreateArgs),
    Update(TagUpdateArgs),
    Delete { gid: String },
}

#[derive(Debug, Args)]
pub struct TagListArgs {
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct TagCreateArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct TagUpdateArgs {
    pub gid: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum StoryCommand {
    List(StoryListArgs),
    Create(StoryCreateArgs),
    Get { gid: String },
    Update(StoryUpdateArgs),
    Delete { gid: String },
}

#[derive(Debug, Args)]
pub struct StoryListArgs {
    #[arg(long, conflicts_with = "goal", required_unless_present = "goal")]
    pub task: Option<String>,
    #[arg(long, conflicts_with = "task")]
    pub goal: Option<String>,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Args)]
pub struct StoryCreateArgs {
    #[arg(long, conflicts_with = "goal", required_unless_present = "goal")]
    pub task: Option<String>,
    #[arg(long, conflicts_with = "task")]
    pub goal: Option<String>,
    #[arg(long, conflicts_with_all = ["text_file", "html_text", "html_text_file"], required_unless_present_any = ["text_file", "html_text", "html_text_file"])]
    pub text: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "html_text", "html_text_file"])]
    pub text_file: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "text_file", "html_text_file"])]
    pub html_text: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "text_file", "html_text"])]
    pub html_text_file: Option<String>,
}

#[derive(Debug, Args)]
pub struct StoryUpdateArgs {
    pub gid: String,
    #[arg(long, conflicts_with_all = ["text_file", "html_text", "html_text_file"])]
    pub text: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "html_text", "html_text_file"])]
    pub text_file: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "text_file", "html_text_file"])]
    pub html_text: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "text_file", "html_text"])]
    pub html_text_file: Option<String>,
    #[arg(long, conflicts_with = "unpin")]
    pub pin: bool,
    #[arg(long, conflicts_with = "pin")]
    pub unpin: bool,
    #[arg(long, value_name = "JSON|@FILE|-")]
    pub data: Option<String>,
}

#[derive(Debug, Args)]
pub struct TypeaheadArgs {
    pub resource_type: TypeaheadResource,
    #[arg(default_value = "")]
    pub query: String,
    #[arg(long, default_value_t = 20, value_parser = parse_limit)]
    pub count: u8,
}

#[derive(Debug, Args)]
pub struct ReactionListArgs {
    #[arg(long)]
    pub target: String,
    #[arg(long)]
    pub emoji_base: String,
    #[command(flatten)]
    pub pagination: PaginationArgs,
}

#[derive(Debug, Subcommand)]
pub enum UserTaskListCommand {
    Get {
        gid: String,
    },
    ForUser {
        #[arg(default_value = "me")]
        user: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeaheadResource {
    Actor,
    Agent,
    CustomField,
    Goal,
    Project,
    ProjectTemplate,
    Portfolio,
    Tag,
    Task,
    Team,
    User,
}

impl TypeaheadResource {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Actor => "actor",
            Self::Agent => "agent",
            Self::CustomField => "custom_field",
            Self::Goal => "goal",
            Self::Project => "project",
            Self::ProjectTemplate => "project_template",
            Self::Portfolio => "portfolio",
            Self::Tag => "tag",
            Self::Task => "task",
            Self::Team => "team",
            Self::User => "user",
        }
    }
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
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| value.to_owned())
        .map_err(|_| "date must be a valid calendar date in YYYY-MM-DD format".to_owned())
}

fn parse_datetime(value: &str) -> Result<String, String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| value.to_owned())
        .map_err(|_| {
            "date-time must use RFC 3339 format, for example 2026-07-11T12:30:00Z".to_owned()
        })
}

fn parse_gid_date(value: &str) -> Result<(String, String), String> {
    let (gid, date) = crate::input::parse_key_value(value)?;
    parse_date(&date)?;
    Ok((gid, date))
}

fn parse_contribution(value: &str) -> Result<f64, String> {
    let value = value
        .parse::<f64>()
        .map_err(|_| "contribution weight must be a number from 0 through 1".to_owned())?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err("contribution weight must be a number from 0 through 1".to_owned())
    }
}

fn parse_finite_number(value: &str) -> Result<f64, String> {
    let value = value
        .parse::<f64>()
        .map_err(|_| "value must be a finite number".to_owned())?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err("value must be a finite number".to_owned())
    }
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    humantime::parse_duration(value).map_err(|error| format!("invalid duration: {error}"))
}
