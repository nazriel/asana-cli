use crate::{
    auth,
    cli::{
        AttachmentCommand, AuthCommand, Cli, Command, PaginationArgs, ProjectCommand,
        SectionCommand, TaskCommand, TaskSearchArgs, WorkspaceCommand,
    },
    client::{AsanaClient, Page, TaskSearchOptions},
    config::{self, Config},
    download,
    error::{Error, Result},
    models::{AuthStatus, Collection, TaskWithRelations},
    output,
};

pub fn run(cli: Cli) -> Result<()> {
    let Cli {
        json,
        workspace,
        verbose,
        command,
    } = cli;

    match command {
        Command::Auth { command } => run_auth(command, json, verbose),
        command => run_authenticated(command, json, workspace, verbose),
    }
}

fn run_auth(command: AuthCommand, json: bool, verbose: bool) -> Result<()> {
    match command {
        AuthCommand::Login => {
            let user = auth::login(verbose)?;
            if json {
                output::emit_json(&user)
            } else {
                output::print_login(&user);
                Ok(())
            }
        }
        AuthCommand::Status => {
            let config = Config::load()?;
            let (token, token_source) = auth::resolve_token()?;
            let client = AsanaClient::new(token, verbose)?;
            let status = AuthStatus {
                user: client.current_user()?,
                token_source: token_source.as_str().to_owned(),
                workspace_gid: config.workspace_gid,
            };
            if json {
                output::emit_json(&status)
            } else {
                output::print_auth_status(&status);
                Ok(())
            }
        }
        AuthCommand::Logout => {
            auth::logout()?;
            if json {
                output::emit_json(&serde_json::json!({ "logged_out": true }))
            } else {
                println!("Removed the locally stored Asana token from the keyring.");
                println!("ASANA_ACCESS_TOKEN, if set, remains active for its process.");
                Ok(())
            }
        }
    }
}

fn run_authenticated(
    command: Command,
    json: bool,
    workspace_override: Option<String>,
    verbose: bool,
) -> Result<()> {
    let mut config = Config::load()?;
    let (token, _) = auth::resolve_token()?;
    let client = AsanaClient::new(token, verbose)?;

    match command {
        Command::Workspace { command } => match command {
            WorkspaceCommand::List(args) => {
                let collection = Collection::from_response(client.list_workspaces(&page(&args))?);
                if json {
                    output::emit_json(&collection)
                } else {
                    output::print_workspaces(&collection);
                    Ok(())
                }
            }
            WorkspaceCommand::Use { gid } => {
                let workspace = client.get_workspace(&gid)?;
                config.workspace_gid = Some(workspace.gid.clone());
                config.save()?;
                if json {
                    output::emit_json(&workspace)
                } else {
                    println!(
                        "Selected workspace: {} ({})",
                        workspace.name.as_deref().unwrap_or("unnamed workspace"),
                        workspace.gid
                    );
                    Ok(())
                }
            }
        },
        Command::Project { command } => match command {
            ProjectCommand::List(args) => {
                let workspace_gid =
                    config::require_workspace(workspace_override.as_deref(), &config)?;
                let archived = if args.all_projects {
                    None
                } else if args.archived {
                    Some(true)
                } else {
                    Some(false)
                };
                let collection = Collection::from_response(client.list_projects(
                    &workspace_gid,
                    archived,
                    &page(&args.pagination),
                )?);
                if json {
                    output::emit_json(&collection)
                } else {
                    output::print_projects(&collection);
                    Ok(())
                }
            }
        },
        Command::Section { command } => match command {
            SectionCommand::List(args) => {
                let collection = Collection::from_response(
                    client.list_sections(&args.project, &page(&args.pagination))?,
                );
                if json {
                    output::emit_json(&collection)
                } else {
                    output::print_sections(&collection);
                    Ok(())
                }
            }
        },
        Command::Task { command } => run_task(
            command,
            &client,
            &config,
            workspace_override.as_deref(),
            json,
        ),
        Command::Attachment { command } => run_attachment(command, &client, json),
        Command::Auth { .. } => unreachable!("auth commands are handled before authentication"),
    }
}

fn run_task(
    command: TaskCommand,
    client: &AsanaClient,
    config: &Config,
    workspace_override: Option<&str>,
    json: bool,
) -> Result<()> {
    match command {
        TaskCommand::List(args) => {
            let needs_workspace =
                (args.project.is_none() && args.section.is_none()) || args.assignee.is_some();
            let workspace_gid = needs_workspace
                .then(|| config::require_workspace(workspace_override, config))
                .transpose()?;
            let assignee = args.assignee.as_deref().or_else(|| {
                if args.project.is_none() && args.section.is_none() {
                    Some("me")
                } else {
                    None
                }
            });
            let collection = Collection::from_response(client.list_tasks(
                workspace_gid.as_deref(),
                args.project.as_deref(),
                args.section.as_deref(),
                assignee,
                args.include_completed,
                &page(&args.pagination),
            )?);
            if json {
                output::emit_json(&collection)
            } else {
                output::print_tasks(&collection);
                Ok(())
            }
        }
        TaskCommand::Search(args) => {
            let workspace_gid = config::require_workspace(workspace_override, config)?;
            let options = search_options(args);
            let collection =
                Collection::from_response(client.search_tasks(&workspace_gid, &options)?);
            if json {
                output::emit_json(&collection)
            } else {
                output::print_tasks(&collection);
                Ok(())
            }
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
            if json {
                output::emit_json(&result)
            } else {
                output::print_task(&result);
                Ok(())
            }
        }
        TaskCommand::Comments(args) => {
            let response = client.list_stories(&args.gid, &page(&args.pagination))?;
            let next_offset = response.next_page.and_then(|page| page.offset);
            let collection = Collection {
                data: response
                    .data
                    .into_iter()
                    .filter(|story| story.is_comment())
                    .collect(),
                next_offset,
            };
            if json {
                output::emit_json(&collection)
            } else {
                output::print_comments(&collection);
                Ok(())
            }
        }
    }
}

fn run_attachment(command: AttachmentCommand, client: &AsanaClient, json: bool) -> Result<()> {
    match command {
        AttachmentCommand::List(args) => {
            let collection = Collection::from_response(
                client.list_attachments(&args.task_gid, &page(&args.pagination))?,
            );
            if json {
                output::emit_json(&collection)
            } else {
                output::print_attachments(&collection);
                Ok(())
            }
        }
        AttachmentCommand::Download(args) => {
            let downloaded = download::download_one(client, &args.attachment_gid, args.output)?;
            if json {
                output::emit_json(&downloaded)
            } else {
                output::print_download(&downloaded);
                Ok(())
            }
        }
        AttachmentCommand::DownloadAll(args) => {
            let summary = download::download_all(client, &args.task_gid, &args.directory)?;
            if json {
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
    }
}

fn page(args: &PaginationArgs) -> Page {
    Page {
        limit: args.limit,
        offset: args.offset.clone(),
    }
}

fn search_options(args: TaskSearchArgs) -> TaskSearchOptions {
    TaskSearchOptions {
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
    }
}
