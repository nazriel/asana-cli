use std::{thread, time::Instant};

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    auth,
    cli::*,
    client::{AsanaClient, Page, TaskListOptions, TaskSearchOptions},
    config::{self, Config},
    daily, download,
    error::{Error, Result},
    input,
    models::{ApiResponse, AuthStatus, Collection, ProfileStatus, TaskWithRelations},
    output, parity,
    safety::{self, MutationPreview},
    setup,
};

struct GlobalOptions {
    json: bool,
    profile_override: Option<String>,
    workspace_override: Option<String>,
    fields: Option<String>,
    all: bool,
    max_items: usize,
    dry_run: bool,
    yes: bool,
    allow_unknown_endpoint: bool,
    request_timeout: std::time::Duration,
    verbose: bool,
}

pub fn run(cli: Cli) -> Result<()> {
    if let Command::Setup(args) = &cli.command {
        return setup::run(args.shell, cli.json);
    }

    let options = GlobalOptions {
        json: cli.json,
        profile_override: cli.profile,
        workspace_override: cli.workspace,
        fields: cli.fields,
        all: cli.all,
        max_items: cli.max_items,
        dry_run: cli.dry_run,
        yes: cli.yes,
        allow_unknown_endpoint: cli.allow_unknown_endpoint,
        request_timeout: cli.request_timeout,
        verbose: cli.verbose,
    };
    let mut config = Config::load()?;
    let profile = config
        .selected_profile(options.profile_override.as_deref())
        .to_owned();
    config::validate_profile_name(&profile)?;

    match cli.command {
        Command::Auth { command } => run_auth(command, &options, &mut config, &profile),
        Command::Profile { command } => run_profile(command, &options, &mut config, &profile),
        Command::Api { command } if options.dry_run && !matches!(command, ApiCommand::Get(_)) => {
            let client = AsanaClient::new_with_options(
                String::new(),
                options.verbose,
                options.fields.clone(),
                options.request_timeout,
            )?;
            run_api(command, &client, &options)
        }
        command if options.dry_run && command_is_mutation(&command) => {
            let client = AsanaClient::new_with_options(
                String::new(),
                options.verbose,
                options.fields.clone(),
                options.request_timeout,
            )?;
            run_with_client(command, &options, &mut config, &profile, &client)
        }
        command => run_authenticated(command, &options, &mut config, &profile),
    }
}

fn run_auth(
    command: AuthCommand,
    options: &GlobalOptions,
    config: &mut Config,
    profile: &str,
) -> Result<()> {
    match command {
        AuthCommand::Login { no_browser } => {
            let user = auth::login(
                profile,
                no_browser,
                options.verbose,
                options.request_timeout,
            )?;
            config.profile_mut(profile);
            config.save()?;
            if options.json {
                output::emit_json(&user)
            } else {
                output::print_login(&user);
                Ok(())
            }
        }
        AuthCommand::Status => {
            let (token, token_source) = auth::resolve_token(profile)?;
            let client = AsanaClient::new_with_options(
                token,
                options.verbose,
                options.fields.clone(),
                options.request_timeout,
            )?;
            let status = AuthStatus {
                user: client.current_user()?,
                token_source: token_source.as_str().to_owned(),
                profile: profile.to_owned(),
                workspace_gid: config.profile(profile).workspace_gid,
            };
            if options.json {
                output::emit_json(&status)
            } else {
                output::print_auth_status(&status);
                Ok(())
            }
        }
        AuthCommand::Logout => {
            auth::logout(profile)?;
            if options.json {
                output::emit_json(&serde_json::json!({
                    "logged_out": true,
                    "profile": profile,
                }))
            } else {
                println!("Removed the locally stored Asana token for profile {profile:?}.");
                println!("ASANA_ACCESS_TOKEN, if set, remains active for its process.");
                Ok(())
            }
        }
    }
}

fn run_profile(
    command: ProfileCommand,
    options: &GlobalOptions,
    config: &mut Config,
    selected_profile: &str,
) -> Result<()> {
    match command {
        ProfileCommand::List => {
            let profiles = config
                .profile_names()
                .into_iter()
                .map(|name| ProfileStatus {
                    active: name == selected_profile,
                    workspace_gid: config.profile(&name).workspace_gid,
                    name,
                })
                .collect::<Vec<_>>();
            if options.json {
                output::emit_json(&Collection {
                    data: profiles,
                    next_offset: None,
                })
            } else {
                output::print_profiles(&profiles);
                Ok(())
            }
        }
        ProfileCommand::Use { name } => {
            config::validate_profile_name(&name)?;
            config.profile_mut(&name);
            config.active_profile = Some(name.clone());
            config.save()?;
            let result = ProfileStatus {
                workspace_gid: config.profile(&name).workspace_gid,
                name,
                active: true,
            };
            if options.json {
                output::emit_json(&result)
            } else {
                println!("Selected profile: {}", result.name);
                Ok(())
            }
        }
    }
}

fn run_authenticated(
    command: Command,
    options: &GlobalOptions,
    config: &mut Config,
    profile: &str,
) -> Result<()> {
    let (token, _) = auth::resolve_token(profile)?;
    let client = AsanaClient::new_with_options(
        token,
        options.verbose,
        options.fields.clone(),
        options.request_timeout,
    )?;

    run_with_client(command, options, config, profile, &client)
}

fn run_with_client(
    command: Command,
    options: &GlobalOptions,
    config: &mut Config,
    profile: &str,
    client: &AsanaClient,
) -> Result<()> {
    match command {
        Command::Workspace { command } => match command {
            WorkspaceCommand::List(args) => {
                let collection =
                    collect_pages(&args, options, |page| client.list_workspaces(page))?;
                emit_collection(&collection, options.json, output::print_workspaces)
            }
            WorkspaceCommand::Use { gid } => {
                let workspace = client.get_workspace(&gid)?;
                config.profile_mut(profile).workspace_gid = Some(workspace.gid.clone());
                config.save()?;
                if options.json {
                    output::emit_json(&workspace)
                } else {
                    println!(
                        "Selected workspace for profile {profile:?}: {} ({})",
                        workspace.name.as_deref().unwrap_or("unnamed workspace"),
                        workspace.gid
                    );
                    Ok(())
                }
            }
            WorkspaceCommand::Get { gid } => emit_raw_read(
                client.raw_json(Method::GET, &format!("workspaces/{gid}"), &[], None)?,
                options,
            ),
            WorkspaceCommand::Update(args) => {
                let data = mutation_data(args.data.as_deref(), |data| {
                    insert_optional_string(data, "name", args.name);
                })?;
                execute_write(
                    Method::PUT,
                    &format!("workspaces/{}", args.gid),
                    data,
                    "workspace.update",
                    false,
                    client,
                    require_admin_context(options)?,
                )
            }
            WorkspaceCommand::UserAdd(args) => execute_write(
                Method::POST,
                &format!("workspaces/{}/addUser", args.gid),
                keyed_value("user", Value::String(args.user)),
                "workspace.user.add",
                true,
                client,
                require_admin_context(options)?,
            ),
            WorkspaceCommand::UserRemove(args) => execute_write(
                Method::POST,
                &format!("workspaces/{}/removeUser", args.gid),
                keyed_value("user", Value::String(args.user)),
                "workspace.user.remove",
                true,
                client,
                require_admin_context(options)?,
            ),
        },
        Command::Project { command } => run_project(command, client, config, profile, options),
        Command::ProjectTemplate { command } => {
            run_project_template(command, client, config, profile, options)
        }
        Command::TaskTemplate { command } => run_task_template(command, client, options),
        Command::User { command } => run_user(command, client, config, profile, options),
        Command::Team { command } => run_team(command, client, config, profile, options),
        Command::Membership { command } => {
            run_membership(command, client, config, profile, options)
        }
        Command::AccessRequest { command } => run_access_request(command, client, options),
        Command::StatusUpdate { command } => run_status_update(command, client, options),
        Command::CustomField { command } => {
            run_custom_field(command, client, config, profile, options)
        }
        Command::CustomType { command } => run_custom_type(command, client, options),
        Command::Portfolio { command } => run_portfolio(command, client, config, profile, options),
        Command::Goal { command } => run_goal(command, client, config, profile, options),
        Command::GoalRelationship { command } => run_goal_relationship(command, client, options),
        Command::ProjectPortfolioSetting { command } => {
            run_project_portfolio_setting(command, client, options)
        }
        Command::Section { command } => match command {
            SectionCommand::List(args) => {
                let collection = collect_pages(&args.pagination, options, |page| {
                    client.list_sections(&args.project, page)
                })?;
                emit_collection(&collection, options.json, output::print_sections)
            }
            SectionCommand::Get { gid } => emit_raw_read(
                client.raw_json(Method::GET, &format!("sections/{gid}"), &[], None)?,
                options,
            ),
            SectionCommand::Create(args) => {
                let data = daily::section_create(&args)?;
                execute_write(
                    Method::POST,
                    &format!("projects/{}/sections", args.project),
                    data,
                    "section.create",
                    false,
                    client,
                    options,
                )
            }
            SectionCommand::Update(args) => execute_write(
                Method::PUT,
                &format!("sections/{}", args.gid),
                serde_json::json!({ "name": args.name }),
                "section.update",
                false,
                client,
                options,
            ),
            SectionCommand::Delete { gid } => execute_write(
                Method::DELETE,
                &format!("sections/{gid}"),
                serde_json::json!({}),
                "section.delete",
                true,
                client,
                options,
            ),
            SectionCommand::Move(args) => {
                let mut data = serde_json::Map::new();
                data.insert("section".to_owned(), Value::String(args.gid));
                if let Some(gid) = args.before_section {
                    data.insert("before_section".to_owned(), Value::String(gid));
                }
                if let Some(gid) = args.after_section {
                    data.insert("after_section".to_owned(), Value::String(gid));
                }
                execute_write(
                    Method::POST,
                    &format!("projects/{}/sections/insert", args.project),
                    Value::Object(data),
                    "section.move",
                    false,
                    client,
                    options,
                )
            }
            SectionCommand::TaskAdd(args) => {
                let mut data = serde_json::Map::new();
                data.insert("task".to_owned(), Value::String(args.task));
                insert_optional_string(&mut data, "insert_before", args.insert_before);
                insert_optional_string(&mut data, "insert_after", args.insert_after);
                if args.at_start {
                    data.insert("insert_after".to_owned(), Value::Null);
                } else if args.at_end {
                    data.insert("insert_before".to_owned(), Value::Null);
                }
                execute_write(
                    Method::POST,
                    &format!("sections/{}/addTask", args.gid),
                    Value::Object(data),
                    "section.task.add",
                    false,
                    client,
                    options,
                )
            }
        },
        Command::Task { command } => run_task(*command, client, config, profile, options),
        Command::Attachment { command } => run_attachment(command, client, options),
        Command::Tag { command } => run_tag(command, client, config, profile, options),
        Command::Story { command } => run_story(command, client, options),
        Command::Typeahead(args) => run_typeahead(args, client, config, profile, options),
        Command::Reaction(args) => emit_raw_collection(
            "reactions",
            vec![
                ("target".to_owned(), args.target),
                ("emoji_base".to_owned(), args.emoji_base),
            ],
            &args.pagination,
            client,
            options,
        ),
        Command::UserTaskList { command } => match command {
            UserTaskListCommand::Get { gid } => emit_raw_read(
                client.raw_json(Method::GET, &format!("user_task_lists/{gid}"), &[], None)?,
                options,
            ),
            UserTaskListCommand::ForUser { user } => {
                let workspace = config::require_workspace(
                    options.workspace_override.as_deref(),
                    config,
                    profile,
                )?;
                emit_raw_read(
                    client.raw_json(
                        Method::GET,
                        &format!("users/{user}/user_task_list"),
                        &[("workspace".to_owned(), workspace)],
                        None,
                    )?,
                    options,
                )
            }
        },
        Command::Api { command } => run_api(command, client, options),
        Command::Job { command } => run_job(command, client, options),
        Command::Auth { .. } | Command::Profile { .. } | Command::Setup(_) => {
            unreachable!("commands handled before authentication")
        }
    }
}

fn run_project(
    command: ProjectCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        ProjectCommand::List(args) => {
            let archived = if args.all_projects {
                None
            } else {
                Some(args.archived)
            };
            if args.team.is_some() || args.task.is_some() {
                let path = match (args.team, args.task) {
                    (Some(team), None) => format!("teams/{team}/projects"),
                    (None, Some(task)) => format!("tasks/{task}/projects"),
                    _ => unreachable!("clap enforces project list scope conflicts"),
                };
                let mut query = Vec::new();
                push_query(&mut query, "archived", archived.map(|v| v.to_string()));
                return emit_raw_collection(&path, query, &args.pagination, client, options);
            }
            let workspace = selected_workspace(config, profile, options)?;
            let collection = collect_pages(&args.pagination, options, |page| {
                client.list_projects(&workspace, archived, page)
            })?;
            emit_collection(&collection, options.json, output::print_projects)
        }
        ProjectCommand::Search(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let mut query = vec![("text".to_owned(), args.text)];
            push_query(&mut query, "teams.any", args.team);
            push_query(
                &mut query,
                "completed",
                args.completed.map(|v| v.to_string()),
            );
            query.extend(args.filters);
            emit_raw_collection(
                &format!("workspaces/{workspace}/projects/search"),
                query,
                &args.pagination,
                client,
                options,
            )
        }
        ProjectCommand::Get { gid } => emit_get(&format!("projects/{gid}"), client, options),
        ProjectCommand::Create(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let team = args.team.clone();
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "owner", args.owner);
                insert_optional_string(data, "notes", args.notes);
                insert_optional_string(data, "color", args.color);
                insert_optional_string(data, "default_view", args.default_view);
                insert_optional_bool(data, "public", args.public);
                insert_optional_bool(data, "archived", args.archived);
                insert_custom_field_values(data, args.custom_fields);
            })?;
            let path = team.map_or_else(
                || format!("workspaces/{workspace}/projects"),
                |gid| format!("teams/{gid}/projects"),
            );
            execute_write(
                Method::POST,
                &path,
                data,
                "project.create",
                false,
                client,
                options,
            )
        }
        ProjectCommand::Update(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "owner", args.owner);
                insert_optional_string(data, "notes", args.notes);
                insert_optional_string(data, "color", args.color);
                insert_optional_string(data, "default_view", args.default_view);
                insert_optional_bool(data, "public", args.public);
                if args.archive {
                    data.insert("archived".to_owned(), Value::Bool(true));
                }
                if args.unarchive {
                    data.insert("archived".to_owned(), Value::Bool(false));
                }
                insert_custom_field_values(data, args.custom_fields);
            })?;
            execute_write(
                Method::PUT,
                &format!("projects/{}", args.gid),
                data,
                "project.update",
                false,
                client,
                options,
            )
        }
        ProjectCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("projects/{gid}"),
            Value::Object(Default::default()),
            "project.delete",
            true,
            client,
            options,
        ),
        ProjectCommand::Duplicate(args) => {
            let data = mutation_data(args.data.as_deref(), |object| {
                object.insert("name".to_owned(), Value::String(args.name));
                if !args.include.is_empty() {
                    object.insert("include".to_owned(), Value::String(args.include.join(",")));
                }
            })?;
            execute_write(
                Method::POST,
                &format!("projects/{}/duplicate", args.gid),
                data,
                "project.duplicate",
                false,
                client,
                options,
            )
        }
        ProjectCommand::TaskCounts { gid } => {
            emit_get(&format!("projects/{gid}/task_counts"), client, options)
        }
        ProjectCommand::Member(args) => {
            run_project_relation(args, "Members", "members", client, options)
        }
        ProjectCommand::Follower(args) => {
            run_project_relation(args, "Followers", "followers", client, options)
        }
        ProjectCommand::CustomField(args) => {
            let (endpoint, data, destructive) = if args.remove {
                (
                    "removeCustomFieldSetting",
                    keyed_value("custom_field", Value::String(args.custom_field)),
                    true,
                )
            } else {
                (
                    "addCustomFieldSetting",
                    serde_json::json!({"custom_field": args.custom_field, "is_important": args.important}),
                    false,
                )
            };
            execute_write(
                Method::POST,
                &format!("projects/{}/{endpoint}", args.gid),
                data,
                "project.custom-field",
                destructive,
                client,
                options,
            )
        }
        ProjectCommand::CustomFields { gid, pagination } => emit_raw_collection(
            &format!("projects/{gid}/custom_field_settings"),
            Vec::new(),
            &pagination,
            client,
            options,
        ),
        ProjectCommand::Brief { command } => run_project_brief(command, client, options),
        ProjectCommand::SaveAsTemplate(args) => {
            let mut data = serde_json::Map::new();
            data.insert("name".to_owned(), Value::String(args.name));
            data.insert("public".to_owned(), Value::Bool(args.public));
            if let Some(team) = args.team {
                data.insert("team".to_owned(), Value::String(team));
            } else {
                data.insert(
                    "workspace".to_owned(),
                    Value::String(selected_workspace(config, profile, options)?),
                );
            }
            execute_write(
                Method::POST,
                &format!("projects/{}/saveAsTemplate", args.gid),
                Value::Object(data),
                "project.save-as-template",
                false,
                client,
                options,
            )
        }
    }
}

fn run_project_relation(
    args: ProjectRelationArgs,
    endpoint_suffix: &str,
    key: &str,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    let (verb, values, destructive) = if args.remove.is_empty() {
        ("add", args.add, false)
    } else {
        ("remove", args.remove, true)
    };
    execute_write(
        Method::POST,
        &format!("projects/{}/{verb}{endpoint_suffix}", args.gid),
        keyed_value(key, serde_json::json!(values)),
        &format!("project.{}.{}", key.trim_end_matches('s'), verb),
        destructive,
        client,
        options,
    )
}

fn run_project_brief(
    command: ProjectBriefCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        ProjectBriefCommand::Get { project } => emit_get(
            &format!("projects/{project}/project_briefs"),
            client,
            options,
        ),
        ProjectBriefCommand::Create(args) => {
            let data = brief_data(args.title, args.text, args.html_text, args.data.as_deref())?;
            execute_write(
                Method::POST,
                &format!("projects/{}/project_briefs", args.project),
                data,
                "project-brief.create",
                false,
                client,
                options,
            )
        }
        ProjectBriefCommand::Update(args) => {
            let data = brief_data(args.title, args.text, args.html_text, args.data.as_deref())?;
            execute_write(
                Method::PUT,
                &format!("project_briefs/{}", args.gid),
                data,
                "project-brief.update",
                false,
                client,
                options,
            )
        }
        ProjectBriefCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("project_briefs/{gid}"),
            Value::Object(Default::default()),
            "project-brief.delete",
            true,
            client,
            options,
        ),
    }
}

fn brief_data(
    title: Option<String>,
    text: Option<String>,
    html_text: Option<String>,
    raw: Option<&str>,
) -> Result<Value> {
    if text.is_some() && html_text.is_some() {
        return Err(Error::Usage("--text conflicts with --html-text".to_owned()));
    }
    mutation_data(raw, |data| {
        insert_optional_string(data, "title", title);
        insert_optional_string(data, "text", text);
        insert_optional_string(data, "html_text", html_text);
    })
}

fn run_project_template(
    command: ProjectTemplateCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        ProjectTemplateCommand::List(args) => {
            let path = args.team.map_or_else(
                || "project_templates".to_owned(),
                |gid| format!("teams/{gid}/project_templates"),
            );
            let mut query = Vec::new();
            if path == "project_templates" {
                query.push((
                    "workspace".to_owned(),
                    selected_workspace(config, profile, options)?,
                ));
            }
            emit_raw_collection(&path, query, &args.pagination, client, options)
        }
        ProjectTemplateCommand::Get { gid } => {
            emit_get(&format!("project_templates/{gid}"), client, options)
        }
        ProjectTemplateCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("project_templates/{gid}"),
            Value::Object(Default::default()),
            "project-template.delete",
            true,
            client,
            options,
        ),
        ProjectTemplateCommand::Instantiate(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                data.insert("name".to_owned(), Value::String(args.name));
                insert_optional_string(data, "team", args.team);
                insert_optional_bool(data, "public", args.public);
                if !args.requested_dates.is_empty() {
                    let dates = args
                        .requested_dates
                        .into_iter()
                        .map(|(gid, value)| serde_json::json!({"gid": gid, "value": value}))
                        .collect::<Vec<_>>();
                    data.insert("requested_dates".to_owned(), Value::Array(dates));
                }
            })?;
            execute_write(
                Method::POST,
                &format!("project_templates/{}/instantiateProject", args.gid),
                data,
                "project-template.instantiate",
                false,
                client,
                options,
            )
        }
    }
}

fn run_task_template(
    command: TaskTemplateCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        TaskTemplateCommand::List(args) => emit_raw_collection(
            "task_templates",
            vec![("project".to_owned(), args.project)],
            &args.pagination,
            client,
            options,
        ),
        TaskTemplateCommand::Get { gid } => {
            emit_get(&format!("task_templates/{gid}"), client, options)
        }
        TaskTemplateCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("task_templates/{gid}"),
            Value::Object(Default::default()),
            "task-template.delete",
            true,
            client,
            options,
        ),
        TaskTemplateCommand::Instantiate(args) => {
            let data = mutation_data_allow_empty(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name)
            })?;
            execute_write(
                Method::POST,
                &format!("task_templates/{}/instantiateTask", args.gid),
                data,
                "task-template.instantiate",
                false,
                client,
                options,
            )
        }
    }
}

fn run_user(
    command: UserCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        UserCommand::List(args) => {
            let path = if let Some(team) = args.team {
                format!("teams/{team}/users")
            } else {
                let workspace = selected_workspace(config, profile, options)?;
                format!("workspaces/{workspace}/users")
            };
            emit_raw_collection(&path, Vec::new(), &args.pagination, client, options)
        }
        UserCommand::Get(args) => {
            let path = if args.in_workspace {
                format!(
                    "workspaces/{}/users/{}",
                    selected_workspace(config, profile, options)?,
                    args.gid
                )
            } else {
                format!("users/{}", args.gid)
            };
            emit_get(&path, client, options)
        }
        UserCommand::Update(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name)
            })?;
            let path = if args.in_workspace {
                format!(
                    "workspaces/{}/users/{}",
                    selected_workspace(config, profile, options)?,
                    args.gid
                )
            } else {
                format!("users/{}", args.gid)
            };
            execute_write(
                Method::PUT,
                &path,
                data,
                "user.update",
                false,
                client,
                options,
            )
        }
        UserCommand::Favorites(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let query = vec![
                ("workspace".to_owned(), workspace),
                ("resource_type".to_owned(), args.resource_type),
            ];
            emit_raw_collection(
                &format!("users/{}/favorites", args.gid),
                query,
                &args.pagination,
                client,
                options,
            )
        }
    }
}

fn run_team(
    command: TeamCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        TeamCommand::List(args) => {
            let path = match args.user {
                Some(user) => format!("users/{user}/teams"),
                None => format!(
                    "workspaces/{}/teams",
                    selected_workspace(config, profile, options)?
                ),
            };
            emit_raw_collection(&path, Vec::new(), &args.pagination, client, options)
        }
        TeamCommand::Get { gid } => emit_get(&format!("teams/{gid}"), client, options),
        TeamCommand::Create(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let organization = args.organization;
            let mut data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "description", args.description);
                insert_optional_string(data, "html_description", args.html_description);
                insert_optional_string(data, "organization", organization);
                insert_optional_string(data, "visibility", args.visibility);
            })?;
            if data.get("organization").is_none() {
                data.as_object_mut()
                    .expect("object")
                    .insert("organization".to_owned(), Value::String(workspace));
            }
            execute_write(
                Method::POST,
                "teams",
                data,
                "team.create",
                false,
                client,
                options,
            )
        }
        TeamCommand::Update(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "description", args.description);
                insert_optional_string(data, "html_description", args.html_description);
                insert_optional_string(data, "visibility", args.visibility);
            })?;
            execute_write(
                Method::PUT,
                &format!("teams/{}", args.gid),
                data,
                "team.update",
                false,
                client,
                options,
            )
        }
        TeamCommand::UserAdd(args) => execute_write(
            Method::POST,
            &format!("teams/{}/addUser", args.gid),
            keyed_value("user", Value::String(args.user)),
            "team.user.add",
            true,
            client,
            require_admin_context(options)?,
        ),
        TeamCommand::UserRemove(args) => execute_write(
            Method::POST,
            &format!("teams/{}/removeUser", args.gid),
            keyed_value("user", Value::String(args.user)),
            "team.user.remove",
            true,
            client,
            require_admin_context(options)?,
        ),
    }
}

fn run_membership(
    command: MembershipCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        MembershipCommand::List(args) => {
            let mut query = Vec::new();
            push_query(&mut query, "parent", args.parent);
            push_query(&mut query, "member", args.member);
            push_query(&mut query, "resource_subtype", args.resource_subtype);
            return emit_raw_collection("memberships", query, &args.pagination, client, options);
        }
        MembershipCommand::Get { gid } => {
            return emit_get(&format!("memberships/{gid}"), client, options);
        }
        MembershipCommand::Create(args) => {
            let mut data = serde_json::json!({"parent": args.parent, "member": args.member});
            insert_optional_string(
                data.as_object_mut().expect("object"),
                "access_level",
                args.access_level,
            );
            return execute_write(
                Method::POST,
                "memberships",
                data,
                "membership.create",
                false,
                client,
                options,
            );
        }
        MembershipCommand::Update(args) => {
            return execute_write(
                Method::PUT,
                &format!("memberships/{}", args.gid),
                serde_json::json!({"access_level": args.access_level}),
                "membership.update",
                false,
                client,
                options,
            );
        }
        MembershipCommand::Delete { gid } => {
            return execute_write(
                Method::DELETE,
                &format!("memberships/{gid}"),
                Value::Object(Default::default()),
                "membership.delete",
                true,
                client,
                options,
            );
        }
        MembershipCommand::Project(_)
        | MembershipCommand::Portfolio(_)
        | MembershipCommand::Team(_)
        | MembershipCommand::Workspace(_) => {}
    }
    let (kind, args) = match command {
        MembershipCommand::Project(a) => ("project", a),
        MembershipCommand::Portfolio(a) => ("portfolio", a),
        MembershipCommand::Team(a) => ("team", a),
        MembershipCommand::Workspace(a) => ("workspace", a),
        _ => unreachable!("generic membership commands returned above"),
    };
    if let Some(gid) = args.gid {
        return emit_get(&format!("{kind}_memberships/{gid}"), client, options);
    }
    let path = match (args.parent, args.user) {
        (Some(parent), None) => format!("{}s/{parent}/{kind}_memberships", kind),
        (None, Some(user)) if kind == "portfolio" => {
            return emit_raw_collection(
                "portfolio_memberships",
                vec![
                    (
                        "workspace".to_owned(),
                        selected_workspace(config, profile, options)?,
                    ),
                    ("user".to_owned(), user),
                ],
                &args.pagination,
                client,
                options,
            );
        }
        (None, Some(user)) if matches!(kind, "team" | "workspace") => {
            format!("users/{user}/{kind}_memberships")
        }
        _ => {
            return Err(Error::Usage(format!(
                "membership {kind} requires --gid, --parent, or a supported --user"
            )));
        }
    };
    emit_raw_collection(&path, Vec::new(), &args.pagination, client, options)
}

fn run_access_request(
    command: AccessRequestCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        AccessRequestCommand::List(args) => emit_raw_collection(
            "access_requests",
            vec![("target".to_owned(), args.target)],
            &args.pagination,
            client,
            options,
        ),
        AccessRequestCommand::Create(args) => {
            let mut data = serde_json::json!({"target": args.target});
            insert_optional_string(
                data.as_object_mut().expect("object"),
                "message",
                args.message,
            );
            execute_write(
                Method::POST,
                "access_requests",
                data,
                "access-request.create",
                false,
                client,
                options,
            )
        }
        AccessRequestCommand::Approve { gid } => execute_action(
            &format!("access_requests/{gid}/approve"),
            "access-request.approve",
            client,
            require_admin_context(options)?,
        ),
        AccessRequestCommand::Reject { gid } => execute_action(
            &format!("access_requests/{gid}/reject"),
            "access-request.reject",
            client,
            require_admin_context(options)?,
        ),
    }
}

fn run_status_update(
    command: StatusUpdateCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        StatusUpdateCommand::List(args) => {
            let mut query = vec![("parent".to_owned(), args.parent)];
            push_query(&mut query, "created_since", args.created_since);
            emit_raw_collection("status_updates", query, &args.pagination, client, options)
        }
        StatusUpdateCommand::Get { gid } => {
            emit_get(&format!("status_updates/{gid}"), client, options)
        }
        StatusUpdateCommand::Create(args) => {
            if args.text.is_none() && args.html_text.is_none() && args.data.is_none() {
                return Err(Error::Usage(
                    "status update requires --text, --html-text, or --data".to_owned(),
                ));
            }
            let data = mutation_data(args.data.as_deref(), |data| {
                data.insert("parent".to_owned(), Value::String(args.parent));
                data.insert("status_type".to_owned(), Value::String(args.status_type));
                insert_optional_string(data, "title", args.title);
                insert_optional_string(data, "text", args.text);
                insert_optional_string(data, "html_text", args.html_text);
            })?;
            execute_write(
                Method::POST,
                "status_updates",
                data,
                "status-update.create",
                false,
                client,
                options,
            )
        }
        StatusUpdateCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("status_updates/{gid}"),
            Value::Object(Default::default()),
            "status-update.delete",
            true,
            client,
            options,
        ),
    }
}

fn run_custom_field(
    command: CustomFieldCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        CustomFieldCommand::List(pagination) => emit_raw_collection(
            &format!(
                "workspaces/{}/custom_fields",
                selected_workspace(config, profile, options)?
            ),
            Vec::new(),
            &pagination,
            client,
            options,
        ),
        CustomFieldCommand::Create(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let data = mutation_data(args.data.as_deref(), |data| {
                data.insert("workspace".to_owned(), Value::String(workspace));
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "resource_subtype", args.resource_subtype);
                insert_optional_string(data, "description", args.description);
                insert_optional_u64(data, "precision", args.precision.map(u64::from));
                insert_optional_string(data, "format", args.format);
                insert_optional_string(data, "currency_code", args.currency_code);
                insert_optional_string(data, "custom_label", args.custom_label);
                insert_optional_bool(data, "has_notifications_enabled", args.notifications);
            })?;
            execute_write(
                Method::POST,
                "custom_fields",
                data,
                "custom-field.create",
                false,
                client,
                options,
            )
        }
        CustomFieldCommand::Get { gid } => {
            emit_get(&format!("custom_fields/{gid}"), client, options)
        }
        CustomFieldCommand::Update(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "description", args.description);
                insert_optional_u64(data, "precision", args.precision.map(u64::from));
                insert_optional_string(data, "format", args.format);
                insert_optional_string(data, "currency_code", args.currency_code);
                insert_optional_string(data, "custom_label", args.custom_label);
                insert_optional_bool(data, "has_notifications_enabled", args.notifications);
            })?;
            execute_write(
                Method::PUT,
                &format!("custom_fields/{}", args.gid),
                data,
                "custom-field.update",
                false,
                client,
                options,
            )
        }
        CustomFieldCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("custom_fields/{gid}"),
            Value::Object(Default::default()),
            "custom-field.delete",
            true,
            client,
            options,
        ),
        CustomFieldCommand::EnumOption { command } => match command {
            EnumOptionCommand::Create(args) => {
                let mut data = serde_json::Map::new();
                data.insert("name".to_owned(), Value::String(args.name));
                insert_optional_string(&mut data, "color", args.color);
                insert_optional_string(&mut data, "insert_before", args.insert_before);
                insert_optional_string(&mut data, "insert_after", args.insert_after);
                execute_write(
                    Method::POST,
                    &format!("custom_fields/{}/enum_options", args.custom_field),
                    Value::Object(data),
                    "enum-option.create",
                    false,
                    client,
                    options,
                )
            }
            EnumOptionCommand::Update(args) => {
                let data = mutation_data(None, |data| {
                    insert_optional_string(data, "name", args.name);
                    insert_optional_string(data, "color", args.color);
                    insert_optional_bool(data, "enabled", args.enabled);
                })?;
                execute_write(
                    Method::PUT,
                    &format!("enum_options/{}", args.gid),
                    data,
                    "enum-option.update",
                    false,
                    client,
                    options,
                )
            }
            EnumOptionCommand::Move(args) => {
                let mut data = serde_json::json!({"enum_option": args.gid});
                let object = data.as_object_mut().expect("object");
                insert_optional_string(object, "before_enum_option", args.before_enum_option);
                insert_optional_string(object, "after_enum_option", args.after_enum_option);
                execute_write(
                    Method::POST,
                    &format!("custom_fields/{}/enum_options/insert", args.custom_field),
                    data,
                    "enum-option.move",
                    false,
                    client,
                    options,
                )
            }
        },
        CustomFieldCommand::Settings(args) => emit_raw_collection(
            &format!(
                "{}/{}/custom_field_settings",
                args.parent_type.path(),
                args.parent_gid
            ),
            Vec::new(),
            &args.pagination,
            client,
            options,
        ),
    }
}

fn run_custom_type(
    command: CustomTypeCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        CustomTypeCommand::List(args) => emit_raw_collection(
            "custom_types",
            vec![("project".to_owned(), args.project)],
            &args.pagination,
            client,
            options,
        ),
        CustomTypeCommand::Get { gid } => emit_get(&format!("custom_types/{gid}"), client, options),
    }
}

fn run_portfolio(
    command: PortfolioCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        PortfolioCommand::List(args) => emit_raw_collection(
            "portfolios",
            vec![
                (
                    "workspace".to_owned(),
                    selected_workspace(config, profile, options)?,
                ),
                ("owner".to_owned(), args.owner),
            ],
            &args.pagination,
            client,
            options,
        ),
        PortfolioCommand::Get { gid } => emit_get(&format!("portfolios/{gid}"), client, options),
        PortfolioCommand::Create(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let data = mutation_data(args.data.as_deref(), |data| {
                data.insert("workspace".to_owned(), Value::String(workspace));
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "color", args.color);
                insert_optional_string(data, "start_on", args.start_on);
                insert_optional_string(data, "due_on", args.due_on);
                insert_optional_string(data, "privacy_setting", args.privacy_setting);
            })?;
            execute_write(
                Method::POST,
                "portfolios",
                data,
                "portfolio.create",
                false,
                client,
                options,
            )
        }
        PortfolioCommand::Update(args) => {
            if (args.start_on.is_some() || args.clear_start)
                && args.due_on.is_none()
                && !args.clear_due
            {
                return Err(Error::Usage(
                    "portfolio start date changes require --due-on or --clear-due".to_owned(),
                ));
            }
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "color", args.color);
                insert_optional_string(data, "start_on", args.start_on);
                insert_optional_string(data, "due_on", args.due_on);
                if args.clear_start {
                    data.insert("start_on".to_owned(), Value::Null);
                }
                if args.clear_due {
                    data.insert("due_on".to_owned(), Value::Null);
                }
                if args.archive {
                    data.insert("archived".to_owned(), Value::Bool(true));
                }
                if args.unarchive {
                    data.insert("archived".to_owned(), Value::Bool(false));
                }
                insert_custom_field_values(data, args.custom_fields);
            })?;
            execute_write(
                Method::PUT,
                &format!("portfolios/{}", args.gid),
                data,
                "portfolio.update",
                false,
                client,
                options,
            )
        }
        PortfolioCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("portfolios/{gid}"),
            Value::Object(Default::default()),
            "portfolio.delete",
            true,
            client,
            options,
        ),
        PortfolioCommand::Duplicate(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                data.insert("name".to_owned(), Value::String(args.name));
                if !args.include.is_empty() {
                    data.insert("include".to_owned(), Value::String(args.include.join(",")));
                }
            })?;
            execute_write(
                Method::POST,
                &format!("portfolios/{}/duplicate", args.gid),
                data,
                "portfolio.duplicate",
                false,
                client,
                options,
            )
        }
        PortfolioCommand::Item(args) => {
            if let Some(item) = args.add {
                let mut data = serde_json::json!({"item": item});
                let object = data.as_object_mut().expect("object");
                insert_optional_string(object, "insert_before", args.insert_before);
                insert_optional_string(object, "insert_after", args.insert_after);
                execute_write(
                    Method::POST,
                    &format!("portfolios/{}/addItem", args.gid),
                    data,
                    "portfolio.item.add",
                    false,
                    client,
                    options,
                )
            } else if let Some(item) = args.remove {
                execute_write(
                    Method::POST,
                    &format!("portfolios/{}/removeItem", args.gid),
                    keyed_value("item", Value::String(item)),
                    "portfolio.item.remove",
                    true,
                    client,
                    options,
                )
            } else {
                emit_raw_collection(
                    &format!("portfolios/{}/items", args.gid),
                    Vec::new(),
                    &args.pagination,
                    client,
                    options,
                )
            }
        }
        PortfolioCommand::Member(args) => {
            let (verb, members, destructive) = if args.remove.is_empty() {
                ("addMembers", args.add, false)
            } else {
                ("removeMembers", args.remove, true)
            };
            execute_write(
                Method::POST,
                &format!("portfolios/{}/{verb}", args.gid),
                keyed_value("members", serde_json::json!(members)),
                "portfolio.member",
                destructive,
                client,
                options,
            )
        }
        PortfolioCommand::CustomField(args) => {
            run_strategic_custom_field("portfolios", args, client, options)
        }
        PortfolioCommand::CustomFields { gid, pagination } => emit_raw_collection(
            &format!("portfolios/{gid}/custom_field_settings"),
            Vec::new(),
            &pagination,
            client,
            options,
        ),
    }
}

fn run_goal(
    command: GoalCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        GoalCommand::List(args) => {
            let mut query = vec![(
                "workspace".to_owned(),
                selected_workspace(config, profile, options)?,
            )];
            push_query(&mut query, "portfolio", args.portfolio);
            push_query(&mut query, "project", args.project);
            push_query(&mut query, "task", args.task);
            push_query(&mut query, "team", args.team);
            push_query(
                &mut query,
                "is_workspace_level",
                args.workspace_level.map(|v| v.to_string()),
            );
            if !args.time_periods.is_empty() {
                query.push(("time_periods".to_owned(), args.time_periods.join(",")));
            }
            emit_raw_collection("goals", query, &args.pagination, client, options)
        }
        GoalCommand::Get { gid } => emit_get(&format!("goals/{gid}"), client, options),
        GoalCommand::Create(args) => {
            let workspace = selected_workspace(config, profile, options)?;
            let data = mutation_data(args.data.as_deref(), |data| {
                data.insert("workspace".to_owned(), Value::String(workspace));
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "owner", args.owner);
                insert_optional_string(data, "team", args.team);
                insert_optional_string(data, "time_period", args.time_period);
                insert_optional_string(data, "due_on", args.due_on);
                insert_optional_string(data, "start_on", args.start_on);
                insert_optional_string(data, "notes", args.notes);
                insert_optional_string(data, "html_notes", args.html_notes);
                insert_optional_bool(data, "is_workspace_level", args.workspace_level);
                if !args.followers.is_empty() {
                    data.insert("followers".to_owned(), serde_json::json!(args.followers));
                }
            })?;
            execute_write(
                Method::POST,
                "goals",
                data,
                "goal.create",
                false,
                client,
                options,
            )
        }
        GoalCommand::Update(args) => {
            let data = mutation_data(args.data.as_deref(), |data| {
                insert_optional_string(data, "name", args.name);
                insert_optional_string(data, "owner", args.owner);
                insert_optional_string(data, "team", args.team);
                insert_optional_string(data, "time_period", args.time_period);
                insert_optional_string(data, "due_on", args.due_on);
                insert_optional_string(data, "start_on", args.start_on);
                insert_optional_string(data, "notes", args.notes);
                insert_optional_string(data, "html_notes", args.html_notes);
                insert_optional_string(data, "status", args.status);
                if args.clear_owner {
                    data.insert("owner".to_owned(), Value::Null);
                }
                if args.clear_team {
                    data.insert("team".to_owned(), Value::Null);
                }
                if args.clear_time_period {
                    data.insert("time_period".to_owned(), Value::Null);
                }
                if args.clear_due {
                    data.insert("due_on".to_owned(), Value::Null);
                }
                if args.clear_start {
                    data.insert("start_on".to_owned(), Value::Null);
                }
                if args.clear_notes {
                    data.insert("notes".to_owned(), Value::String(String::new()));
                }
                insert_custom_field_values(data, args.custom_fields);
            })?;
            execute_write(
                Method::PUT,
                &format!("goals/{}", args.gid),
                data,
                "goal.update",
                false,
                client,
                options,
            )
        }
        GoalCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("goals/{gid}"),
            Value::Object(Default::default()),
            "goal.delete",
            true,
            client,
            options,
        ),
        GoalCommand::Metric { command } => match command {
            GoalMetricCommand::Set(args) => {
                let data = mutation_data(args.data.as_deref(), |object| {
                    insert_optional_string(object, "unit", args.unit);
                    insert_optional_number(object, "initial_number_value", args.initial);
                    insert_optional_number(object, "target_number_value", args.target);
                    insert_optional_number(object, "current_number_value", args.current);
                    insert_optional_u64(object, "precision", args.precision.map(u64::from));
                    insert_optional_string(object, "currency_code", args.currency_code);
                    insert_optional_string(object, "progress_source", args.progress_source);
                })?;
                execute_write(
                    Method::POST,
                    &format!("goals/{}/setMetric", args.gid),
                    data,
                    "goal.metric.set",
                    false,
                    client,
                    options,
                )
            }
            GoalMetricCommand::Update(args) => execute_write(
                Method::POST,
                &format!("goals/{}/setMetricCurrentValue", args.gid),
                serde_json::json!({"current_number_value": args.current}),
                "goal.metric.update",
                false,
                client,
                options,
            ),
        },
        GoalCommand::Follower(args) => {
            let (verb, followers, destructive) = if args.remove.is_empty() {
                ("addFollowers", args.add, false)
            } else {
                ("removeFollowers", args.remove, true)
            };
            execute_write(
                Method::POST,
                &format!("goals/{}/{verb}", args.gid),
                keyed_value("followers", serde_json::json!(followers)),
                "goal.follower",
                destructive,
                client,
                options,
            )
        }
        GoalCommand::Parents { gid, pagination } => emit_raw_collection(
            &format!("goals/{gid}/parentGoals"),
            Vec::new(),
            &pagination,
            client,
            options,
        ),
        GoalCommand::CustomField(args) => {
            run_strategic_custom_field("goals", args, client, options)
        }
        GoalCommand::CustomFields { gid, pagination } => emit_raw_collection(
            &format!("goals/{gid}/custom_field_settings"),
            Vec::new(),
            &pagination,
            client,
            options,
        ),
    }
}

fn run_strategic_custom_field(
    parent_path: &str,
    args: StrategicCustomFieldArgs,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    let (endpoint, data, destructive) = if args.remove {
        (
            "removeCustomFieldSetting",
            keyed_value("custom_field", Value::String(args.custom_field)),
            true,
        )
    } else {
        (
            "addCustomFieldSetting",
            serde_json::json!({"custom_field": args.custom_field, "is_important": args.important}),
            false,
        )
    };
    execute_write(
        Method::POST,
        &format!("{parent_path}/{}/{endpoint}", args.gid),
        data,
        "custom-field.setting",
        destructive,
        client,
        options,
    )
}

fn run_goal_relationship(
    command: GoalRelationshipCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        GoalRelationshipCommand::List(args) => {
            let mut query = vec![("supported_goal".to_owned(), args.supported_goal)];
            push_query(&mut query, "resource_subtype", args.resource_subtype);
            emit_raw_collection(
                "goal_relationships",
                query,
                &args.pagination,
                client,
                options,
            )
        }
        GoalRelationshipCommand::Get { gid } => {
            emit_get(&format!("goal_relationships/{gid}"), client, options)
        }
        GoalRelationshipCommand::Add(args) => {
            let mut data = serde_json::json!({"supporting_resource": args.supporting_resource});
            let object = data.as_object_mut().expect("object");
            insert_optional_number(object, "contribution_weight", args.contribution_weight);
            insert_optional_string(object, "insert_before", args.insert_before);
            insert_optional_string(object, "insert_after", args.insert_after);
            execute_write(
                Method::POST,
                &format!("goals/{}/addSupportingRelationship", args.goal),
                data,
                "goal-relationship.add",
                false,
                client,
                options,
            )
        }
        GoalRelationshipCommand::Update(args) => execute_write(
            Method::PUT,
            &format!("goal_relationships/{}", args.gid),
            serde_json::json!({"contribution_weight": args.contribution_weight}),
            "goal-relationship.update",
            false,
            client,
            options,
        ),
        GoalRelationshipCommand::Remove(args) => execute_write(
            Method::POST,
            &format!("goals/{}/removeSupportingRelationship", args.goal),
            serde_json::json!({"supporting_resource": args.supporting_resource}),
            "goal-relationship.remove",
            true,
            client,
            options,
        ),
    }
}

fn run_project_portfolio_setting(
    command: ProjectPortfolioSettingCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        ProjectPortfolioSettingCommand::List(args) => {
            let path = match (args.project, args.portfolio) {
                (Some(gid), None) => format!("projects/{gid}/project_portfolio_settings"),
                (None, Some(gid)) => format!("portfolios/{gid}/project_portfolio_settings"),
                _ => unreachable!("clap enforces one project portfolio setting parent"),
            };
            emit_raw_collection(&path, Vec::new(), &args.pagination, client, options)
        }
        ProjectPortfolioSettingCommand::Get { gid } => emit_get(
            &format!("project_portfolio_settings/{gid}"),
            client,
            options,
        ),
        ProjectPortfolioSettingCommand::Update(args) => execute_write(
            Method::PUT,
            &format!("project_portfolio_settings/{}", args.gid),
            serde_json::json!({"is_access_control_inherited": args.inherit_access}),
            "project-portfolio-setting.update",
            false,
            client,
            options,
        ),
    }
}

fn run_task(
    command: TaskCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        TaskCommand::List(args) => {
            let needs_workspace = (args.project.is_none()
                && args.section.is_none()
                && args.tag.is_none()
                && args.user_task_list.is_none())
                || args.assignee.is_some();
            let workspace_gid = needs_workspace
                .then(|| {
                    config::require_workspace(
                        options.workspace_override.as_deref(),
                        config,
                        profile,
                    )
                })
                .transpose()?;
            let assignee = args.assignee.as_deref().or_else(|| {
                if args.project.is_none()
                    && args.section.is_none()
                    && args.tag.is_none()
                    && args.user_task_list.is_none()
                {
                    Some("me")
                } else {
                    None
                }
            });
            let collection = collect_pages(&args.pagination, options, |page| {
                client.list_tasks(
                    &TaskListOptions {
                        workspace_gid: workspace_gid.as_deref(),
                        project_gid: args.project.as_deref(),
                        section_gid: args.section.as_deref(),
                        tag_gid: args.tag.as_deref(),
                        user_task_list_gid: args.user_task_list.as_deref(),
                        assignee,
                        include_completed: args.include_completed,
                    },
                    page,
                )
            })?;
            emit_collection(&collection, options.json, output::print_tasks)
        }
        TaskCommand::Search(args) => {
            if options.all {
                return Err(Error::Usage(
                    "task search has unstable ordering and does not support --all".to_owned(),
                ));
            }
            let workspace_gid =
                config::require_workspace(options.workspace_override.as_deref(), config, profile)?;
            let search = search_options(args)?;
            let collection =
                Collection::from_response(client.search_tasks(&workspace_gid, &search)?);
            emit_collection(&collection, options.json, output::print_tasks)
        }
        TaskCommand::Get(args) => {
            let task = client.get_task(&args.gid)?;
            let comments = args
                .comments
                .then(|| client.list_all_comments(&args.gid))
                .transpose()?;
            let attachments = args
                .attachments
                .then(|| client.list_all_attachments(&args.gid))
                .transpose()?;
            let result = TaskWithRelations {
                task,
                comments,
                attachments,
            };
            if options.json {
                output::emit_json(&result)
            } else {
                output::print_task(&result);
                Ok(())
            }
        }
        TaskCommand::GetCustom { custom_id } => {
            let workspace =
                config::require_workspace(options.workspace_override.as_deref(), config, profile)?;
            emit_raw_read(
                client.raw_json(
                    Method::GET,
                    &format!("workspaces/{workspace}/tasks/custom_id/{custom_id}"),
                    &[],
                    None,
                )?,
                options,
            )
        }
        TaskCommand::Comments(args) => {
            let collection = collect_pages(&args.pagination, options, |page| {
                client.list_stories(&args.gid, page)
            })?;
            let comments = Collection {
                data: collection
                    .data
                    .into_iter()
                    .filter(|story| story.is_comment())
                    .collect(),
                next_offset: collection.next_offset,
            };
            emit_collection(&comments, options.json, output::print_comments)
        }
        TaskCommand::Create(args) => {
            let workspace_gid = options
                .workspace_override
                .clone()
                .or_else(|| config.profile(profile).workspace_gid);
            let data = daily::task_create(&args, workspace_gid.as_deref(), false)?;
            execute_write(
                Method::POST,
                "tasks",
                data,
                "task.create",
                false,
                client,
                options,
            )
        }
        TaskCommand::Update(args) => {
            if !options.dry_run
                && let Some(expected) = &args.expect_modified_at
            {
                let modified_at = client.get_task_modified_at(&args.gid)?;
                if modified_at.as_deref() != Some(expected.as_str()) {
                    return Err(Error::Stale(format!(
                        "task {} has modified_at {:?}, expected {expected:?}",
                        args.gid, modified_at
                    )));
                }
            }
            let data = daily::task_update(&args)?;
            execute_write(
                Method::PUT,
                &format!("tasks/{}", args.gid),
                data,
                "task.update",
                false,
                client,
                options,
            )
        }
        TaskCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("tasks/{gid}"),
            serde_json::json!({}),
            "task.delete",
            true,
            client,
            options,
        ),
        TaskCommand::Duplicate(args) => {
            let mut data = serde_json::Map::new();
            insert_optional_string(&mut data, "name", args.name);
            if !args.include.is_empty() {
                data.insert("include".to_owned(), Value::String(args.include.join(",")));
            }
            execute_write(
                Method::POST,
                &format!("tasks/{}/duplicate", args.gid),
                Value::Object(data),
                "task.duplicate",
                false,
                client,
                options,
            )
        }
        TaskCommand::Comment { command } => match command {
            TaskCommentCommand::List(args) => run_task(
                TaskCommand::Comments(args),
                client,
                config,
                profile,
                options,
            ),
            TaskCommentCommand::Add(args) => {
                let data = daily::comment_create(&args)?;
                execute_write(
                    Method::POST,
                    &format!("tasks/{}/stories", args.task_gid),
                    data,
                    "task.comment.add",
                    false,
                    client,
                    options,
                )
            }
            TaskCommentCommand::Update(args) => {
                run_story(StoryCommand::Update(args), client, options)
            }
            TaskCommentCommand::Delete { gid } => {
                run_story(StoryCommand::Delete { gid }, client, options)
            }
        },
        TaskCommand::Subtask { command } => match command {
            TaskSubtaskCommand::List(args) => emit_relation_list(
                &format!("tasks/{}/subtasks", args.task_gid),
                &args.pagination,
                client,
                options,
            ),
            TaskSubtaskCommand::Create(args) => {
                if args.task.parent.is_some() {
                    return Err(Error::Usage(
                        "subtask create gets its parent from TASK_GID; do not pass --parent"
                            .to_owned(),
                    ));
                }
                let data = daily::task_create(&args.task, None, true)?;
                execute_write(
                    Method::POST,
                    &format!("tasks/{}/subtasks", args.task_gid),
                    data,
                    "task.subtask.create",
                    false,
                    client,
                    options,
                )
            }
        },
        TaskCommand::Parent { command } => match command {
            TaskParentCommand::Set(args) => {
                let mut data = serde_json::Map::new();
                data.insert("parent".to_owned(), Value::String(args.parent));
                insert_optional_string(&mut data, "insert_before", args.insert_before);
                insert_optional_string(&mut data, "insert_after", args.insert_after);
                execute_write(
                    Method::POST,
                    &format!("tasks/{}/setParent", args.task_gid),
                    Value::Object(data),
                    "task.parent.set",
                    false,
                    client,
                    options,
                )
            }
            TaskParentCommand::Clear { task_gid } => execute_write(
                Method::POST,
                &format!("tasks/{task_gid}/setParent"),
                serde_json::json!({ "parent": null }),
                "task.parent.clear",
                false,
                client,
                options,
            ),
        },
        TaskCommand::Dependency { command } => run_task_multi_relation(
            dependency_action(command),
            "dependencies",
            "dependencies",
            client,
            options,
        ),
        TaskCommand::Dependent { command } => run_task_multi_relation(
            dependent_action(command),
            "dependents",
            "dependents",
            client,
            options,
        ),
        TaskCommand::Tag { command } => match command {
            TaskTagCommand::List(args) => emit_relation_list(
                &format!("tasks/{}/tags", args.task_gid),
                &args.pagination,
                client,
                options,
            ),
            TaskTagCommand::Add(args) => execute_single_relation(
                args,
                "addTag",
                "tag",
                "task.tag.add",
                false,
                client,
                options,
            ),
            TaskTagCommand::Remove(args) => execute_single_relation(
                args,
                "removeTag",
                "tag",
                "task.tag.remove",
                true,
                client,
                options,
            ),
        },
        TaskCommand::Project { command } => match command {
            TaskProjectCommand::List(args) => emit_relation_list(
                &format!("tasks/{}/projects", args.task_gid),
                &args.pagination,
                client,
                options,
            ),
            TaskProjectCommand::Add(args) => {
                let mut data = serde_json::Map::new();
                data.insert("project".to_owned(), Value::String(args.project_gid));
                insert_optional_string(&mut data, "section", args.section);
                insert_optional_string(&mut data, "insert_before", args.insert_before);
                insert_optional_string(&mut data, "insert_after", args.insert_after);
                if args.at_start {
                    data.insert("insert_after".to_owned(), Value::Null);
                } else if args.at_end {
                    data.insert("insert_before".to_owned(), Value::Null);
                }
                execute_write(
                    Method::POST,
                    &format!("tasks/{}/addProject", args.task_gid),
                    Value::Object(data),
                    "task.project.add",
                    false,
                    client,
                    options,
                )
            }
            TaskProjectCommand::Remove(args) => execute_single_relation(
                args,
                "removeProject",
                "project",
                "task.project.remove",
                true,
                client,
                options,
            ),
        },
        TaskCommand::Follower { command } => match command {
            TaskFollowerCommand::Add(args) => execute_write(
                Method::POST,
                &format!("tasks/{}/addFollowers", args.task_gid),
                serde_json::json!({ "followers": args.user }),
                "task.follower.add",
                false,
                client,
                options,
            ),
            TaskFollowerCommand::Remove(args) => execute_write(
                Method::POST,
                &format!("tasks/{}/removeFollowers", args.task_gid),
                serde_json::json!({ "followers": args.user }),
                "task.follower.remove",
                true,
                client,
                options,
            ),
        },
    }
}

fn run_attachment(
    command: AttachmentCommand,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        AttachmentCommand::Get { gid } => emit_raw_read(
            client.raw_json(Method::GET, &format!("attachments/{gid}"), &[], None)?,
            options,
        ),
        AttachmentCommand::List(args) => {
            let collection = collect_pages(&args.pagination, options, |page| {
                client.list_attachments(&args.parent_gid, page)
            })?;
            emit_collection(&collection, options.json, output::print_attachments)
        }
        AttachmentCommand::Download(args) => {
            let downloaded = download::download_one(client, &args.attachment_gid, args.output)?;
            if options.json {
                output::emit_json(&downloaded)
            } else {
                output::print_download(&downloaded);
                Ok(())
            }
        }
        AttachmentCommand::DownloadAll(args) => {
            let summary = download::download_all(client, &args.task_gid, &args.directory)?;
            if options.json {
                output::emit_json(&summary)?;
            } else {
                output::print_download_summary(&summary);
            }
            if summary.failed.is_empty() {
                Ok(())
            } else {
                Err(Error::PartialDownload(format!(
                    "{} of {} attachment(s) could not be downloaded",
                    summary.failed.len(),
                    summary.downloaded.len() + summary.failed.len()
                )))
            }
        }
        AttachmentCommand::Upload(args) => {
            let preview = serde_json::json!({
                "parent": args.parent,
                "file": args.file,
                "resource_subtype": "asana",
            });
            if options.dry_run {
                return emit_preview(
                    &MutationPreview {
                        dry_run: true,
                        method: "POST",
                        path: "/attachments",
                        query: &[],
                        data: Some(&preview),
                        risk: "additive_write",
                        confirmation_required: false,
                    },
                    options.json,
                );
            }
            let value = client.raw_upload(
                "attachments",
                &[],
                "file",
                &args.file,
                &[("parent".to_owned(), args.parent)],
            )?;
            emit_value(&normalize_raw(value), options.json)
        }
        AttachmentCommand::AttachUrl(args) => {
            let mut fields = vec![
                ("parent".to_owned(), args.parent),
                ("resource_subtype".to_owned(), "external".to_owned()),
                ("name".to_owned(), args.name),
                ("url".to_owned(), args.url),
            ];
            if args.connect_to_app {
                fields.push(("connect_to_app".to_owned(), "true".to_owned()));
            }
            let preview = serde_json::json!({ "form_fields": fields });
            if options.dry_run {
                return emit_preview(
                    &MutationPreview {
                        dry_run: true,
                        method: "POST",
                        path: "/attachments",
                        query: &[],
                        data: Some(&preview),
                        risk: "additive_write",
                        confirmation_required: false,
                    },
                    options.json,
                );
            }
            let value = client.raw_form("attachments", &[], &fields)?;
            emit_value(&normalize_raw(value), options.json)
        }
        AttachmentCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("attachments/{gid}"),
            serde_json::json!({}),
            "attachment.delete",
            true,
            client,
            options,
        ),
    }
}

fn run_tag(
    command: TagCommand,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    match command {
        TagCommand::List(args) => {
            let workspace =
                config::require_workspace(options.workspace_override.as_deref(), config, profile)?;
            emit_raw_collection(
                "tags",
                vec![("workspace".to_owned(), workspace)],
                &args.pagination,
                client,
                options,
            )
        }
        TagCommand::Get { gid } => emit_raw_read(
            client.raw_json(Method::GET, &format!("tags/{gid}"), &[], None)?,
            options,
        ),
        TagCommand::Create(args) => {
            let workspace =
                config::require_workspace(options.workspace_override.as_deref(), config, profile)?;
            let data = daily::tag_create(&args, &workspace)?;
            execute_write(
                Method::POST,
                "tags",
                data,
                "tag.create",
                false,
                client,
                options,
            )
        }
        TagCommand::Update(args) => {
            let data = daily::tag_update(&args)?;
            execute_write(
                Method::PUT,
                &format!("tags/{}", args.gid),
                data,
                "tag.update",
                false,
                client,
                options,
            )
        }
        TagCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("tags/{gid}"),
            serde_json::json!({}),
            "tag.delete",
            true,
            client,
            options,
        ),
    }
}

fn run_story(command: StoryCommand, client: &AsanaClient, options: &GlobalOptions) -> Result<()> {
    match command {
        StoryCommand::List(args) => {
            let path = if let Some(gid) = args.task {
                format!("tasks/{gid}/stories")
            } else {
                format!(
                    "goals/{}/stories",
                    args.goal.expect("goal required by clap")
                )
            };
            emit_raw_collection(&path, Vec::new(), &args.pagination, client, options)
        }
        StoryCommand::Create(args) => {
            let path = if let Some(gid) = &args.task {
                format!("tasks/{gid}/stories")
            } else {
                format!(
                    "goals/{}/stories",
                    args.goal.as_deref().expect("goal required by clap")
                )
            };
            let data = daily::story_create(&args)?;
            execute_write(
                Method::POST,
                &path,
                data,
                "story.create",
                false,
                client,
                options,
            )
        }
        StoryCommand::Get { gid } => emit_raw_read(
            client.raw_json(Method::GET, &format!("stories/{gid}"), &[], None)?,
            options,
        ),
        StoryCommand::Update(args) => {
            let data = daily::story_update(&args)?;
            execute_write(
                Method::PUT,
                &format!("stories/{}", args.gid),
                data,
                "story.update",
                false,
                client,
                options,
            )
        }
        StoryCommand::Delete { gid } => execute_write(
            Method::DELETE,
            &format!("stories/{gid}"),
            serde_json::json!({}),
            "story.delete",
            true,
            client,
            options,
        ),
    }
}

fn run_typeahead(
    args: TypeaheadArgs,
    client: &AsanaClient,
    config: &Config,
    profile: &str,
    options: &GlobalOptions,
) -> Result<()> {
    if options.all {
        return Err(Error::Usage(
            "typeahead is intentionally non-exhaustive and does not support --all".to_owned(),
        ));
    }
    let workspace =
        config::require_workspace(options.workspace_override.as_deref(), config, profile)?;
    let value = client.raw_json(
        Method::GET,
        &format!("workspaces/{workspace}/typeahead"),
        &[
            (
                "resource_type".to_owned(),
                args.resource_type.as_api_value().to_owned(),
            ),
            ("query".to_owned(), args.query),
            ("count".to_owned(), args.count.to_string()),
        ],
        None,
    )?;
    emit_raw_read(value, options)
}

enum MultiRelationAction {
    List(RelationListArgs),
    Add(RelationChangeArgs),
    Remove(RelationChangeArgs),
}

fn dependency_action(command: TaskDependencyCommand) -> MultiRelationAction {
    match command {
        TaskDependencyCommand::List(args) => MultiRelationAction::List(args),
        TaskDependencyCommand::Add(args) => MultiRelationAction::Add(args),
        TaskDependencyCommand::Remove(args) => MultiRelationAction::Remove(args),
    }
}

fn dependent_action(command: TaskDependentCommand) -> MultiRelationAction {
    match command {
        TaskDependentCommand::List(args) => MultiRelationAction::List(args),
        TaskDependentCommand::Add(args) => MultiRelationAction::Add(args),
        TaskDependentCommand::Remove(args) => MultiRelationAction::Remove(args),
    }
}

fn run_task_multi_relation(
    action: MultiRelationAction,
    endpoint: &str,
    body_key: &str,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    match action {
        MultiRelationAction::List(args) => emit_relation_list(
            &format!("tasks/{}/{endpoint}", args.task_gid),
            &args.pagination,
            client,
            options,
        ),
        MultiRelationAction::Add(args) => execute_write(
            Method::POST,
            &format!("tasks/{}/add{}", args.task_gid, upper_first(endpoint)),
            keyed_value(body_key, serde_json::json!(args.related_gid)),
            &format!("task.{endpoint}.add"),
            false,
            client,
            options,
        ),
        MultiRelationAction::Remove(args) => execute_write(
            Method::POST,
            &format!("tasks/{}/remove{}", args.task_gid, upper_first(endpoint)),
            keyed_value(body_key, serde_json::json!(args.related_gid)),
            &format!("task.{endpoint}.remove"),
            true,
            client,
            options,
        ),
    }
}

fn execute_single_relation(
    args: TaskSingleRelationArgs,
    endpoint: &str,
    body_key: &str,
    operation: &str,
    destructive: bool,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    execute_write(
        Method::POST,
        &format!("tasks/{}/{endpoint}", args.task_gid),
        keyed_value(body_key, Value::String(args.related_gid)),
        operation,
        destructive,
        client,
        options,
    )
}

fn emit_relation_list(
    path: &str,
    pagination: &PaginationArgs,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    emit_raw_collection(path, Vec::new(), pagination, client, options)
}

fn emit_raw_collection(
    path: &str,
    mut query: Vec<(String, String)>,
    pagination: &PaginationArgs,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    query.push(("limit".to_owned(), pagination.limit.to_string()));
    if let Some(offset) = &pagination.offset {
        query.push(("offset".to_owned(), offset.clone()));
    }
    let value = if options.all {
        collect_raw_pages(client, path, query, options.max_items)?
    } else {
        normalize_raw(client.raw_json(Method::GET, path, &query, None)?)
    };
    emit_value(&value, options.json)
}

fn emit_raw_read(value: Value, options: &GlobalOptions) -> Result<()> {
    emit_value(&normalize_raw(value), options.json)
}

fn emit_get(path: &str, client: &AsanaClient, options: &GlobalOptions) -> Result<()> {
    emit_raw_read(client.raw_json(Method::GET, path, &[], None)?, options)
}

fn selected_workspace(config: &Config, profile: &str, options: &GlobalOptions) -> Result<String> {
    config::require_workspace(options.workspace_override.as_deref(), config, profile)
}

fn push_query(query: &mut Vec<(String, String)>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        query.push((key.to_owned(), value));
    }
}

fn insert_optional_bool(data: &mut serde_json::Map<String, Value>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        data.insert(key.to_owned(), Value::Bool(value));
    }
}

fn insert_optional_number(
    data: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<f64>,
) {
    if let Some(value) = value.and_then(serde_json::Number::from_f64) {
        data.insert(key.to_owned(), Value::Number(value));
    }
}

fn insert_optional_u64(data: &mut serde_json::Map<String, Value>, key: &str, value: Option<u64>) {
    if let Some(value) = value {
        data.insert(key.to_owned(), Value::Number(value.into()));
    }
}

fn insert_custom_field_values(
    data: &mut serde_json::Map<String, Value>,
    values: Vec<(String, Value)>,
) {
    if !values.is_empty() {
        data.insert(
            "custom_fields".to_owned(),
            Value::Object(values.into_iter().collect()),
        );
    }
}

fn mutation_data<F>(raw: Option<&str>, fill: F) -> Result<Value>
where
    F: FnOnce(&mut serde_json::Map<String, Value>),
{
    let mut typed = serde_json::Map::new();
    fill(&mut typed);
    let mut value = Value::Object(typed);
    if let Some(raw) = raw {
        input::merge_objects(&mut value, input::read_json(raw)?)?;
    }
    if value.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(Error::Usage(
            "at least one mutation field is required".to_owned(),
        ));
    }
    Ok(value)
}

fn mutation_data_allow_empty<F>(raw: Option<&str>, fill: F) -> Result<Value>
where
    F: FnOnce(&mut serde_json::Map<String, Value>),
{
    let mut value = Value::Object(Default::default());
    fill(value.as_object_mut().expect("object"));
    if let Some(raw) = raw {
        input::merge_objects(&mut value, input::read_json(raw)?)?;
    }
    Ok(value)
}

fn require_admin_context(options: &GlobalOptions) -> Result<&GlobalOptions> {
    if options.profile_override.is_none() || options.workspace_override.is_none() {
        return Err(Error::Usage(
            "high-impact administration requires explicit --profile and --workspace".to_owned(),
        ));
    }
    Ok(options)
}

fn execute_write(
    method: Method,
    path: &str,
    data: Value,
    operation: &str,
    destructive: bool,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    if options.dry_run {
        return emit_preview(
            &MutationPreview {
                dry_run: true,
                method: method.as_str(),
                path,
                query: &[],
                data: (method != Method::DELETE).then_some(&data),
                risk: if destructive { "destructive" } else { "write" },
                confirmation_required: destructive,
            },
            options.json,
        );
    }
    if destructive {
        safety::require_destructive_confirmation(
            options.yes,
            &format!("Execute {operation} on {path}"),
        )?;
    }
    let deleted = method == Method::DELETE;
    let value = if method == Method::DELETE {
        client.raw_json(method, path, &[], None)?
    } else {
        client.raw_json(method, path, &[], Some(&data))?
    };
    let value = normalize_raw(value);
    let value = if value == serde_json::json!({}) {
        serde_json::json!({
            "ok": true,
            "operation": operation,
            "path": path,
            "deleted": deleted,
        })
    } else {
        value
    };
    emit_value(&value, options.json)
}

fn execute_action(
    path: &str,
    operation: &str,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    if options.dry_run {
        return emit_preview(
            &MutationPreview {
                dry_run: true,
                method: "POST",
                path,
                query: &[],
                data: None,
                risk: "destructive",
                confirmation_required: true,
            },
            options.json,
        );
    }
    safety::require_destructive_confirmation(
        options.yes,
        &format!("Execute {operation} on {path}"),
    )?;
    let value = normalize_raw(client.raw_json(Method::POST, path, &[], None)?);
    let value = if value == serde_json::json!({}) {
        serde_json::json!({"ok": true, "operation": operation, "path": path})
    } else {
        value
    };
    emit_value(&value, options.json)
}

fn insert_optional_string(
    data: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        data.insert(key.to_owned(), Value::String(value));
    }
}

fn command_is_mutation(command: &Command) -> bool {
    match command {
        Command::Section { command } => !matches!(
            command,
            SectionCommand::List(_) | SectionCommand::Get { .. }
        ),
        Command::Task { command } => task_command_is_mutation(command),
        Command::Attachment { command } => matches!(
            command,
            AttachmentCommand::Upload(_)
                | AttachmentCommand::AttachUrl(_)
                | AttachmentCommand::Delete { .. }
        ),
        Command::Tag { command } => {
            !matches!(command, TagCommand::List(_) | TagCommand::Get { .. })
        }
        Command::Story { command } => {
            !matches!(command, StoryCommand::Get { .. } | StoryCommand::List(_))
        }
        Command::Api { command } => !matches!(command, ApiCommand::Get(_)),
        Command::Workspace { command } => matches!(
            command,
            WorkspaceCommand::Update(_)
                | WorkspaceCommand::UserAdd(_)
                | WorkspaceCommand::UserRemove(_)
        ),
        Command::Project { command } => !matches!(
            command,
            ProjectCommand::List(_)
                | ProjectCommand::Search(_)
                | ProjectCommand::Get { .. }
                | ProjectCommand::TaskCounts { .. }
                | ProjectCommand::CustomFields { .. }
                | ProjectCommand::Brief {
                    command: ProjectBriefCommand::Get { .. }
                }
        ),
        Command::ProjectTemplate { command } => !matches!(
            command,
            ProjectTemplateCommand::List(_) | ProjectTemplateCommand::Get { .. }
        ),
        Command::TaskTemplate { command } => !matches!(
            command,
            TaskTemplateCommand::List(_) | TaskTemplateCommand::Get { .. }
        ),
        Command::User { command } => matches!(command, UserCommand::Update(_)),
        Command::Team { command } => {
            !matches!(command, TeamCommand::List(_) | TeamCommand::Get { .. })
        }
        Command::AccessRequest { command } => !matches!(command, AccessRequestCommand::List(_)),
        Command::StatusUpdate { command } => !matches!(
            command,
            StatusUpdateCommand::List(_) | StatusUpdateCommand::Get { .. }
        ),
        Command::CustomField { command } => !matches!(
            command,
            CustomFieldCommand::List(_)
                | CustomFieldCommand::Get { .. }
                | CustomFieldCommand::Settings(_)
        ),
        Command::CustomType { .. } => false,
        Command::Portfolio { command } => match command {
            PortfolioCommand::List(_)
            | PortfolioCommand::Get { .. }
            | PortfolioCommand::CustomFields { .. } => false,
            PortfolioCommand::Item(args) => args.add.is_some() || args.remove.is_some(),
            _ => true,
        },
        Command::Goal { command } => !matches!(
            command,
            GoalCommand::List(_)
                | GoalCommand::Get { .. }
                | GoalCommand::Parents { .. }
                | GoalCommand::CustomFields { .. }
        ),
        Command::GoalRelationship { command } => !matches!(
            command,
            GoalRelationshipCommand::List(_) | GoalRelationshipCommand::Get { .. }
        ),
        Command::ProjectPortfolioSetting { command } => {
            matches!(command, ProjectPortfolioSettingCommand::Update(_))
        }
        Command::Auth { .. }
        | Command::Profile { .. }
        | Command::Job { .. }
        | Command::Setup(_)
        | Command::Typeahead(_)
        | Command::Reaction(_) => false,
        Command::Membership { command } => matches!(
            command,
            MembershipCommand::Create(_)
                | MembershipCommand::Update(_)
                | MembershipCommand::Delete { .. }
        ),
        Command::UserTaskList { .. } => false,
    }
}

fn task_command_is_mutation(command: &TaskCommand) -> bool {
    match command {
        TaskCommand::List(_)
        | TaskCommand::Search(_)
        | TaskCommand::Get(_)
        | TaskCommand::GetCustom { .. }
        | TaskCommand::Comments(_) => false,
        TaskCommand::Comment { command } => !matches!(command, TaskCommentCommand::List(_)),
        TaskCommand::Subtask { command } => !matches!(command, TaskSubtaskCommand::List(_)),
        TaskCommand::Dependency { command } => !matches!(command, TaskDependencyCommand::List(_)),
        TaskCommand::Dependent { command } => !matches!(command, TaskDependentCommand::List(_)),
        TaskCommand::Tag { command } => !matches!(command, TaskTagCommand::List(_)),
        TaskCommand::Project { command } => !matches!(command, TaskProjectCommand::List(_)),
        TaskCommand::Create(_)
        | TaskCommand::Update(_)
        | TaskCommand::Delete { .. }
        | TaskCommand::Duplicate(_)
        | TaskCommand::Parent { .. }
        | TaskCommand::Follower { .. } => true,
    }
}

fn keyed_value(key: &str, value: Value) -> Value {
    let mut data = serde_json::Map::new();
    data.insert(key.to_owned(), value);
    Value::Object(data)
}

fn upper_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn run_api(command: ApiCommand, client: &AsanaClient, options: &GlobalOptions) -> Result<()> {
    let path = match &command {
        ApiCommand::Get(args) => &args.path,
        ApiCommand::Post(args) | ApiCommand::Put(args) => &args.path,
        ApiCommand::Delete(args) => &args.path,
        ApiCommand::Upload(args) => &args.path,
    };
    parity::ensure_catalogued_path(path, options.allow_unknown_endpoint)?;
    match command {
        ApiCommand::Get(args) => {
            let value = if options.all {
                collect_raw_pages(client, &args.path, args.query, options.max_items)?
            } else {
                normalize_raw(client.raw_json(Method::GET, &args.path, &args.query, None)?)
            };
            emit_value(&value, options.json)
        }
        ApiCommand::Post(args) => run_api_write(
            Method::POST,
            &args.path,
            args.query,
            args.data,
            client,
            options,
        ),
        ApiCommand::Put(args) => run_api_write(
            Method::PUT,
            &args.path,
            args.query,
            args.data,
            client,
            options,
        ),
        ApiCommand::Delete(args) => {
            if options.dry_run {
                return emit_preview(
                    &MutationPreview {
                        dry_run: true,
                        method: "DELETE",
                        path: &args.path,
                        query: &args.query,
                        data: None,
                        risk: "destructive",
                        confirmation_required: true,
                    },
                    options.json,
                );
            }
            safety::require_destructive_confirmation(
                options.yes,
                &format!("Delete the Asana resource at {}", args.path),
            )?;
            let value = client.raw_json(Method::DELETE, &args.path, &args.query, None)?;
            let normalized = normalize_raw(value);
            let receipt = if normalized == serde_json::json!({}) {
                serde_json::json!({
                    "ok": true,
                    "operation": "api.delete",
                    "path": args.path,
                    "deleted": true,
                })
            } else {
                normalized
            };
            emit_value(&receipt, options.json)
        }
        ApiCommand::Upload(args) => {
            let preview_data = serde_json::json!({
                "file": args.file,
                "file_field": args.file_field,
                "form_fields": args.form_fields,
            });
            if options.dry_run {
                return emit_preview(
                    &MutationPreview {
                        dry_run: true,
                        method: "POST",
                        path: &args.path,
                        query: &args.query,
                        data: Some(&preview_data),
                        risk: "additive_write",
                        confirmation_required: false,
                    },
                    options.json,
                );
            }
            let value = client.raw_upload(
                &args.path,
                &args.query,
                &args.file_field,
                &args.file,
                &args.form_fields,
            )?;
            emit_value(&normalize_raw(value), options.json)
        }
    }
}

fn run_api_write(
    method: Method,
    path: &str,
    query: Vec<(String, String)>,
    source: Option<String>,
    client: &AsanaClient,
    options: &GlobalOptions,
) -> Result<()> {
    let data = source
        .as_deref()
        .map(input::read_json)
        .transpose()?
        .unwrap_or_else(|| serde_json::json!({}));
    if options.dry_run {
        return emit_preview(
            &MutationPreview {
                dry_run: true,
                method: method.as_str(),
                path,
                query: &query,
                data: Some(&data),
                risk: "write",
                confirmation_required: false,
            },
            options.json,
        );
    }
    let value = client.raw_json(method, path, &query, Some(&data))?;
    emit_value(&normalize_raw(value), options.json)
}

fn run_job(command: JobCommand, client: &AsanaClient, options: &GlobalOptions) -> Result<()> {
    let job = match command {
        JobCommand::Get { gid } => client.get_job(&gid)?,
        JobCommand::Wait {
            gid,
            timeout,
            interval,
        } => {
            if interval.is_zero() {
                return Err(Error::Usage(
                    "job polling interval must be positive".to_owned(),
                ));
            }
            let started = Instant::now();
            loop {
                let job = client.get_job(&gid)?;
                match job.get("status").and_then(Value::as_str) {
                    Some("succeeded") => break job,
                    Some("failed") => {
                        return Err(Error::Job(format!("job {gid} reported failure")));
                    }
                    _ if started.elapsed() >= timeout => {
                        return Err(Error::Job(format!(
                            "timed out waiting for job {gid} after {}",
                            humantime::format_duration(timeout)
                        )));
                    }
                    _ => thread::sleep(interval),
                }
            }
        }
    };
    emit_value(&job, options.json)
}

fn collect_pages<T, F>(
    args: &PaginationArgs,
    options: &GlobalOptions,
    mut fetch: F,
) -> Result<Collection<T>>
where
    T: DeserializeOwned,
    F: FnMut(&Page) -> Result<ApiResponse<Vec<T>>>,
{
    if !options.all {
        return Ok(Collection::from_response(fetch(&page(args))?));
    }
    if options.max_items == 0 {
        return Err(Error::Usage("--max-items must be positive".to_owned()));
    }

    let mut data = Vec::new();
    let mut offset = args.offset.clone();
    loop {
        let remaining = options.max_items.saturating_sub(data.len());
        if remaining == 0 {
            return Ok(Collection {
                data,
                next_offset: offset,
            });
        }
        let request = Page {
            limit: usize::from(args.limit).min(remaining).min(100) as u8,
            offset: offset.clone(),
        };
        let response = fetch(&request)?;
        data.extend(response.data);
        offset = response.next_page.and_then(|next| next.offset);
        if offset.is_none() {
            return Ok(Collection {
                data,
                next_offset: None,
            });
        }
    }
}

fn collect_raw_pages(
    client: &AsanaClient,
    path: &str,
    mut query: Vec<(String, String)>,
    max_items: usize,
) -> Result<Value> {
    if max_items == 0 {
        return Err(Error::Usage("--max-items must be positive".to_owned()));
    }
    if !query.iter().any(|(key, _)| key == "limit") {
        query.push(("limit".to_owned(), "100".to_owned()));
    }
    let mut data = Vec::new();
    let mut next_offset: Option<String> = None;
    loop {
        query.retain(|(key, _)| key != "offset");
        let remaining = max_items.saturating_sub(data.len());
        let page_limit = remaining.clamp(1, 100).to_string();
        if let Some((_, limit)) = query.iter_mut().find(|(key, _)| key == "limit") {
            let requested = limit.parse::<usize>().unwrap_or(100).clamp(1, 100);
            *limit = requested.min(remaining).max(1).to_string();
        } else {
            query.push(("limit".to_owned(), page_limit));
        }
        if let Some(offset) = &next_offset {
            query.push(("offset".to_owned(), offset.clone()));
        }
        let value = client.raw_json(Method::GET, path, &query, None)?;
        let page_data = value.get("data").and_then(Value::as_array).ok_or_else(|| {
            Error::Usage("--all requires an endpoint returning a data array".to_owned())
        })?;
        data.extend(page_data.iter().take(remaining).cloned());
        next_offset = value
            .get("next_page")
            .and_then(|page| page.get("offset"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        if data.len() >= max_items || next_offset.is_none() {
            return Ok(serde_json::json!({
                "data": data,
                "next_offset": next_offset,
            }));
        }
    }
}

fn normalize_raw(mut value: Value) -> Value {
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    let Some(data) = object.remove("data") else {
        return value;
    };
    let next_offset = object
        .remove("next_page")
        .and_then(|page| page.get("offset").cloned())
        .unwrap_or(Value::Null);
    if data.is_array() {
        let mut result = serde_json::Map::new();
        result.insert("data".to_owned(), data);
        result.insert("next_offset".to_owned(), next_offset);
        result.extend(std::mem::take(object));
        Value::Object(result)
    } else if object.is_empty() {
        data
    } else {
        object.insert("data".to_owned(), data);
        value
    }
}

fn emit_collection<T, F>(collection: &Collection<T>, json: bool, human: F) -> Result<()>
where
    T: serde::Serialize,
    F: FnOnce(&Collection<T>),
{
    if json {
        output::emit_json(collection)
    } else {
        human(collection);
        Ok(())
    }
}

fn emit_value(value: &Value, json: bool) -> Result<()> {
    if json {
        output::print_json_value(value)
    } else {
        output::print_generic_value(value)
    }
}

fn emit_preview(preview: &MutationPreview<'_>, _json: bool) -> Result<()> {
    output::emit_json(preview)
}

fn page(args: &PaginationArgs) -> Page {
    Page {
        limit: args.limit,
        offset: args.offset.clone(),
    }
}

fn search_options(args: TaskSearchArgs) -> Result<TaskSearchOptions> {
    let mut extra = args.filter;
    if !args.section.is_empty() {
        extra.push(("sections.any".to_owned(), args.section.join(",")));
    }
    if !args.tag.is_empty() {
        extra.push(("tags.any".to_owned(), args.tag.join(",")));
    }
    if !args.follower.is_empty() {
        extra.push(("followers.any".to_owned(), args.follower.join(",")));
    }
    for filter in args.custom_field {
        let (selector, value) = filter.split_once('=').ok_or_else(|| {
            Error::Usage("--custom-field expects FIELD_GID.OPERATOR=VALUE".to_owned())
        })?;
        let (gid, operator) = selector.split_once('.').ok_or_else(|| {
            Error::Usage("--custom-field expects FIELD_GID.OPERATOR=VALUE".to_owned())
        })?;
        const OPERATORS: &[&str] = &[
            "is_set",
            "value",
            "starts_with",
            "ends_with",
            "contains",
            "less_than",
            "greater_than",
            "before",
            "after",
        ];
        if gid.is_empty() || !OPERATORS.contains(&operator) {
            return Err(Error::Usage(format!(
                "unsupported custom-field search operator {operator:?}"
            )));
        }
        extra.push((format!("custom_fields.{gid}.{operator}"), value.to_owned()));
    }
    if let Some(filter) = args.custom_type_status {
        let (gid, status) = filter.split_once('=').ok_or_else(|| {
            Error::Usage(
                "--custom-type-status expects CUSTOM_TYPE_GID=STATUS_OPTION_GID".to_owned(),
            )
        })?;
        extra.push(("resource_subtype".to_owned(), "custom".to_owned()));
        extra.push((
            format!("custom_types.{gid}.custom_type_status_option.gid"),
            status.to_owned(),
        ));
    }
    Ok(TaskSearchOptions {
        text: args.query,
        assignee: args.assignee,
        project: args.project,
        completed: if args.completed {
            Some(true)
        } else if args.incomplete {
            Some(false)
        } else {
            None
        },
        due_before: args.due_before,
        due_after: args.due_after,
        created_before: args.created_before,
        created_after: args.created_after,
        modified_before: args.modified_before,
        modified_after: args.modified_after,
        sort_by: args.sort.map(|sort| sort.as_api_value().to_owned()),
        sort_ascending: args.ascending,
        limit: args.limit.limit,
        extra,
    })
}
