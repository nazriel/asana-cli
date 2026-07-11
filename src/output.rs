use serde::Serialize;

use crate::{
    download::{DownloadSummary, DownloadedAttachment},
    error::{Error, Result},
    models::{
        Attachment, AuthStatus, Collection, Project, Story, Task, TaskWithRelations, User,
        Workspace,
    },
};

pub fn emit_json<T: Serialize>(value: &T) -> Result<()> {
    let output = serde_json::to_string_pretty(value)?;
    println!("{output}");
    Ok(())
}

pub fn print_error(error: &Error, json: bool) {
    if json {
        let body = serde_json::json!({
            "error": {
                "code": error.code(),
                "message": error.to_string(),
                "status": error.status(),
                "request_id": error.request_id(),
            }
        });
        match serde_json::to_string(&body) {
            Ok(body) => eprintln!("{body}"),
            Err(_) => eprintln!("{}", error),
        }
        return;
    }

    eprintln!("error: {error}");
    if let Some(request_id) = error.request_id() {
        eprintln!("Asana request ID: {request_id}");
    }
}

pub fn print_login(user: &User) {
    println!(
        "Authenticated as {} ({}) and stored the token in the operating-system keyring.",
        user.name.as_deref().unwrap_or("unknown user"),
        user.email.as_deref().unwrap_or(&user.gid)
    );
}

pub fn print_auth_status(status: &AuthStatus) {
    println!(
        "Authenticated as: {}",
        status.user.name.as_deref().unwrap_or("unknown user")
    );
    println!("User GID:        {}", status.user.gid);
    if let Some(email) = &status.user.email {
        println!("Email:           {email}");
    }
    println!("Token source:    {}", status.token_source);
    println!(
        "Workspace:       {}",
        status.workspace_gid.as_deref().unwrap_or("not selected")
    );
}

pub fn print_workspaces(collection: &Collection<Workspace>) {
    println!("GID\tTYPE\tNAME");
    for workspace in &collection.data {
        let kind = match workspace.is_organization {
            Some(true) => "organization",
            Some(false) => "workspace",
            None => "workspace",
        };
        println!(
            "{}\t{}\t{}",
            workspace.gid,
            kind,
            compact(workspace.name.as_deref().unwrap_or(""))
        );
    }
    print_next_offset(collection.next_offset.as_deref());
}

pub fn print_projects(collection: &Collection<Project>) {
    println!("GID\tARCHIVED\tOWNER\tNAME");
    for project in &collection.data {
        println!(
            "{}\t{}\t{}\t{}",
            project.gid,
            project.archived.unwrap_or(false),
            project
                .owner
                .as_ref()
                .and_then(|owner| owner.name.as_deref())
                .unwrap_or(""),
            compact(project.name.as_deref().unwrap_or(""))
        );
    }
    print_next_offset(collection.next_offset.as_deref());
}

pub fn print_tasks(collection: &Collection<Task>) {
    println!("GID\tSTATUS\tDUE\tASSIGNEE\tNAME");
    for task in &collection.data {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            task.gid,
            if task.completed.unwrap_or(false) {
                "done"
            } else {
                "open"
            },
            task.due_on
                .as_deref()
                .or(task.due_at.as_deref())
                .unwrap_or(""),
            task.assignee
                .as_ref()
                .and_then(|assignee| assignee.name.as_deref())
                .unwrap_or(""),
            compact(task.name.as_deref().unwrap_or(""))
        );
    }
    print_next_offset(collection.next_offset.as_deref());
}

pub fn print_task(task: &TaskWithRelations) {
    let value = &task.task;
    println!("{}", value.name.as_deref().unwrap_or("Untitled task"));
    println!("GID:        {}", value.gid);
    println!(
        "Status:     {}",
        if value.completed.unwrap_or(false) {
            "completed"
        } else {
            "open"
        }
    );
    if let Some(assignee) = &value.assignee {
        println!(
            "Assignee:   {}",
            assignee.name.as_deref().unwrap_or(&assignee.gid)
        );
    }
    if let Some(due) = value.due_on.as_deref().or(value.due_at.as_deref()) {
        println!("Due:        {due}");
    }
    if let Some(url) = &value.permalink_url {
        println!("URL:        {url}");
    }
    if let Some(notes) = value
        .notes
        .as_deref()
        .filter(|notes| !notes.trim().is_empty())
    {
        println!("\nDescription:\n{notes}");
    }
    if let Some(comments) = &task.comments {
        println!("\nComments ({})", comments.len());
        print_stories_inner(comments);
    }
    if let Some(attachments) = &task.attachments {
        println!("\nAttachments ({})", attachments.len());
        print_attachments_inner(attachments);
    }
}

pub fn print_comments(collection: &Collection<Story>) {
    print_stories_inner(&collection.data);
    print_next_offset(collection.next_offset.as_deref());
}

pub fn print_attachments(collection: &Collection<Attachment>) {
    print_attachments_inner(&collection.data);
    print_next_offset(collection.next_offset.as_deref());
}

pub fn print_download(result: &DownloadedAttachment) {
    println!(
        "Downloaded {} to {} ({} bytes)",
        result.name, result.path, result.bytes
    );
}

pub fn print_download_summary(summary: &DownloadSummary) {
    for download in &summary.downloaded {
        println!("downloaded\t{}\t{}", download.name, download.path);
    }
    for failure in &summary.failed {
        println!("failed\t{}\t{}", failure.name, failure.error);
    }
    println!(
        "Downloaded {} attachment(s); {} failed.",
        summary.downloaded.len(),
        summary.failed.len()
    );
}

fn print_stories_inner(stories: &[Story]) {
    println!("CREATED\tAUTHOR\tCOMMENT");
    for story in stories {
        println!(
            "{}\t{}\t{}",
            story.created_at.as_deref().unwrap_or(""),
            story
                .created_by
                .as_ref()
                .and_then(|author| author.name.as_deref())
                .unwrap_or(""),
            compact(story.text.as_deref().unwrap_or(""))
        );
    }
}

fn print_attachments_inner(attachments: &[Attachment]) {
    println!("GID\tHOST\tNAME");
    for attachment in attachments {
        println!(
            "{}\t{}\t{}",
            attachment.gid,
            attachment.host.as_deref().unwrap_or(""),
            compact(attachment.name.as_deref().unwrap_or(""))
        );
    }
}

fn print_next_offset(offset: Option<&str>) {
    if let Some(offset) = offset {
        println!("\nNext page: --offset {offset}");
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
