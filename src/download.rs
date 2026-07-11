use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    client::AsanaClient,
    error::{Error, Result},
    models::Attachment,
};

#[derive(Debug, Clone, Serialize)]
pub struct DownloadedAttachment {
    pub gid: String,
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadFailure {
    pub gid: String,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadSummary {
    pub task_gid: String,
    pub downloaded: Vec<DownloadedAttachment>,
    pub failed: Vec<DownloadFailure>,
}

pub fn download_one(
    client: &AsanaClient,
    attachment_gid: &str,
    output: Option<PathBuf>,
) -> Result<DownloadedAttachment> {
    let attachment = client.get_attachment(attachment_gid)?;
    let destination = output.unwrap_or_else(|| PathBuf::from(safe_attachment_name(&attachment)));
    download_attachment_to_path(client, &attachment, &destination)
}

pub fn download_all(
    client: &AsanaClient,
    task_gid: &str,
    directory: &Path,
) -> Result<DownloadSummary> {
    fs::create_dir_all(directory)?;
    let attachments = client.list_all_attachments(task_gid)?;
    let mut downloaded = Vec::new();
    let mut failed = Vec::new();

    for attachment in attachments {
        let name = attachment_name(&attachment);
        let destination = directory.join(safe_attachment_name(&attachment));
        match download_attachment_to_path(client, &attachment, &destination) {
            Ok(result) => downloaded.push(result),
            Err(error) => failed.push(DownloadFailure {
                gid: attachment.gid,
                name,
                error: error.to_string(),
            }),
        }
    }

    Ok(DownloadSummary {
        task_gid: task_gid.to_owned(),
        downloaded,
        failed,
    })
}

fn download_attachment_to_path(
    client: &AsanaClient,
    attachment: &Attachment,
    destination: &Path,
) -> Result<DownloadedAttachment> {
    if destination.exists() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("refusing to overwrite {}", destination.display()),
        )));
    }

    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("output directory does not exist: {}", parent.display()),
        )));
    }

    // Refresh right before download: Asana's download_url may expire quickly.
    let attachment = client.get_attachment(&attachment.gid)?;
    let download_url = attachment
        .download_url
        .as_deref()
        .ok_or_else(|| Error::Api {
            status: 0,
            message: format!(
                "attachment {} has no downloadable URL (host: {})",
                attachment.gid,
                attachment.host.as_deref().unwrap_or("unknown")
            ),
            request_id: None,
        })?;

    let temporary = temporary_path(destination)?;
    let result = (|| -> Result<u64> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let bytes = client.download_url_to_writer(download_url, &mut file)?;
        file.flush()?;
        drop(file);
        // Linking in the same directory publishes the completed file without the
        // replacement semantics that `rename` has on Unix.
        fs::hard_link(&temporary, destination)?;
        fs::remove_file(&temporary)?;
        Ok(bytes)
    })();

    let bytes = match result {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };

    let name = attachment_name(&attachment);
    Ok(DownloadedAttachment {
        gid: attachment.gid,
        name,
        path: destination.display().to_string(),
        bytes,
    })
}

fn temporary_path(destination: &Path) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let filename = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            Error::Usage(format!(
                "output path must name a file: {}",
                destination.display()
            ))
        })?;

    for suffix in 0..100_u16 {
        let temporary = parent.join(format!(
            ".{filename}.asana-cli-{}-{suffix}.part",
            std::process::id()
        ));
        if !temporary.exists() {
            return Ok(temporary);
        }
    }

    Err(Error::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "could not allocate a temporary file next to {}",
            destination.display()
        ),
    )))
}

fn attachment_name(attachment: &Attachment) -> String {
    attachment
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("attachment-{}", attachment.gid))
}

fn safe_attachment_name(attachment: &Attachment) -> String {
    let candidate = attachment_name(attachment);
    let base = Path::new(&candidate)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment");

    let mut safe = base
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    while safe.starts_with('.') {
        safe.remove(0);
    }
    if safe.is_empty() || safe == "." || safe == ".." {
        safe = format!("attachment-{}", attachment.gid);
    }

    safe
}
