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
        AuthCommand::Login => {
            let user = auth::login(profile, options.verbose, options.request_timeout)?;
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
        },
        Command::Project { command } => match command {
            ProjectCommand::List(args) => {
                let workspace_gid = config::require_workspace(
                    options.workspace_override.as_deref(),
                    config,
                    profile,
                )?;
                let archived = if args.all_projects {
                    None
                } else if args.archived {
                    Some(true)
                } else {
                    Some(false)
                };
                let collection = collect_pages(&args.pagination, options, |page| {
                    client.list_projects(&workspace_gid, archived, page)
                })?;
                emit_collection(&collection, options.json, output::print_projects)
            }
        },
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
        Command::Auth { .. } | Command::Profile { .. } => {
            unreachable!("commands handled before authentication")
        }
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
                data.insert("include".to_owned(), serde_json::json!(args.include));
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
        Command::Auth { .. }
        | Command::Workspace { .. }
        | Command::Project { .. }
        | Command::Profile { .. }
        | Command::Job { .. }
        | Command::Typeahead(_)
        | Command::Reaction(_) => false,
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
