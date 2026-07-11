use std::{env, io::Write, thread, time::Duration};

use reqwest::{
    StatusCode, Url,
    blocking::{Client, Response},
    header::{ACCEPT, HeaderMap, HeaderValue, RETRY_AFTER},
};
use serde::de::DeserializeOwned;

use crate::{
    error::{Error, Result},
    models::{
        ApiErrorResponse, ApiResponse, Attachment, Project, Section, Story, Task, User, Workspace,
    },
};

const DEFAULT_BASE_URL: &str = "https://app.asana.com/api/1.0/";
const USER_AGENT: &str = concat!("asana-cli/", env!("CARGO_PKG_VERSION"));

const WORKSPACE_FIELDS: &str = "gid,name,is_organization";
const PROJECT_FIELDS: &str = "gid,name,archived,color,owner.gid,owner.name,permalink_url";
const SECTION_FIELDS: &str = "gid,name,created_at,project.gid,project.name";
const TASK_LIST_FIELDS: &str = "gid,name,completed,completed_at,assignee.gid,assignee.name,due_on,due_at,modified_at,permalink_url";
const TASK_DETAIL_FIELDS: &str = "gid,name,completed,completed_at,assignee.gid,assignee.name,due_on,due_at,start_on,notes,html_notes,created_at,modified_at,permalink_url,resource_subtype,workspace.gid,workspace.name,memberships.project.gid,memberships.project.name,memberships.section.gid,memberships.section.name";
const STORY_FIELDS: &str =
    "gid,resource_subtype,created_at,created_by.gid,created_by.name,text,html_text,is_edited";
const ATTACHMENT_FIELDS: &str =
    "gid,name,host,parent.gid,parent.name,created_at,download_url,permanent_url";

#[derive(Debug, Clone)]
pub struct Page {
    pub limit: u8,
    pub offset: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskSearchOptions {
    pub text: String,
    pub assignee: Option<String>,
    pub project: Option<String>,
    pub completed: Option<bool>,
    pub due_before: Option<String>,
    pub due_after: Option<String>,
    pub created_before: Option<String>,
    pub created_after: Option<String>,
    pub modified_before: Option<String>,
    pub modified_after: Option<String>,
    pub sort_by: Option<String>,
    pub sort_ascending: bool,
    pub limit: u8,
}

pub struct AsanaClient {
    http: Client,
    base_url: Url,
    token: String,
    verbose: bool,
}

impl AsanaClient {
    pub fn new(token: String, verbose: bool) -> Result<Self> {
        let raw_base_url =
            env::var("ASANA_API_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        let base_url = normalize_base_url(&raw_base_url)?;
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .user_agent(USER_AGENT)
            .build()?;

        Ok(Self {
            http,
            base_url,
            token,
            verbose,
        })
    }

    pub fn current_user(&self) -> Result<User> {
        Ok(self
            .get_json(
                "users/me",
                &[("opt_fields".to_owned(), "gid,name,email".to_owned())],
            )?
            .data)
    }

    pub fn list_workspaces(&self, page: &Page) -> Result<ApiResponse<Vec<Workspace>>> {
        self.get_json(
            "workspaces",
            &with_page(
                vec![("opt_fields".to_owned(), WORKSPACE_FIELDS.to_owned())],
                page,
            ),
        )
    }

    pub fn get_workspace(&self, gid: &str) -> Result<Workspace> {
        Ok(self
            .get_json(
                &format!("workspaces/{gid}"),
                &[("opt_fields".to_owned(), WORKSPACE_FIELDS.to_owned())],
            )?
            .data)
    }

    pub fn list_projects(
        &self,
        workspace_gid: &str,
        archived: Option<bool>,
        page: &Page,
    ) -> Result<ApiResponse<Vec<Project>>> {
        let mut params = vec![("opt_fields".to_owned(), PROJECT_FIELDS.to_owned())];
        if let Some(archived) = archived {
            params.push(("archived".to_owned(), archived.to_string()));
        }

        self.get_json(
            &format!("workspaces/{workspace_gid}/projects"),
            &with_page(params, page),
        )
    }

    pub fn list_sections(
        &self,
        project_gid: &str,
        page: &Page,
    ) -> Result<ApiResponse<Vec<Section>>> {
        self.get_json(
            &format!("projects/{project_gid}/sections"),
            &with_page(
                vec![("opt_fields".to_owned(), SECTION_FIELDS.to_owned())],
                page,
            ),
        )
    }

    pub fn list_tasks(
        &self,
        workspace_gid: Option<&str>,
        project_gid: Option<&str>,
        section_gid: Option<&str>,
        assignee: Option<&str>,
        include_completed: bool,
        page: &Page,
    ) -> Result<ApiResponse<Vec<Task>>> {
        let mut params = vec![("opt_fields".to_owned(), TASK_LIST_FIELDS.to_owned())];

        if let Some(workspace_gid) = workspace_gid {
            params.push(("workspace".to_owned(), workspace_gid.to_owned()));
        }
        if let Some(project_gid) = project_gid {
            params.push(("project".to_owned(), project_gid.to_owned()));
        }
        if let Some(assignee) = assignee {
            params.push(("assignee".to_owned(), assignee.to_owned()));
        }
        if !include_completed {
            params.push(("completed_since".to_owned(), "now".to_owned()));
        }

        let path = section_gid
            .map(|gid| format!("sections/{gid}/tasks"))
            .unwrap_or_else(|| "tasks".to_owned());

        self.get_json(&path, &with_page(params, page))
    }

    pub fn search_tasks(
        &self,
        workspace_gid: &str,
        options: &TaskSearchOptions,
    ) -> Result<ApiResponse<Vec<Task>>> {
        let mut params = vec![
            ("opt_fields".to_owned(), TASK_LIST_FIELDS.to_owned()),
            ("text".to_owned(), options.text.clone()),
            ("limit".to_owned(), options.limit.to_string()),
        ];

        push_option(&mut params, "assignee.any", options.assignee.as_deref());
        push_option(&mut params, "projects.any", options.project.as_deref());
        if let Some(completed) = options.completed {
            params.push(("completed".to_owned(), completed.to_string()));
        }
        push_option(&mut params, "due_on.before", options.due_before.as_deref());
        push_option(&mut params, "due_on.after", options.due_after.as_deref());
        push_option(
            &mut params,
            "created_on.before",
            options.created_before.as_deref(),
        );
        push_option(
            &mut params,
            "created_on.after",
            options.created_after.as_deref(),
        );
        push_option(
            &mut params,
            "modified_on.before",
            options.modified_before.as_deref(),
        );
        push_option(
            &mut params,
            "modified_on.after",
            options.modified_after.as_deref(),
        );
        push_option(&mut params, "sort_by", options.sort_by.as_deref());
        if options.sort_by.is_some() && options.sort_ascending {
            params.push(("sort_ascending".to_owned(), "true".to_owned()));
        }

        self.get_json(&format!("workspaces/{workspace_gid}/tasks/search"), &params)
    }

    pub fn get_task(&self, gid: &str) -> Result<Task> {
        Ok(self
            .get_json(
                &format!("tasks/{gid}"),
                &[("opt_fields".to_owned(), TASK_DETAIL_FIELDS.to_owned())],
            )?
            .data)
    }

    pub fn list_stories(&self, task_gid: &str, page: &Page) -> Result<ApiResponse<Vec<Story>>> {
        self.get_json(
            &format!("tasks/{task_gid}/stories"),
            &with_page(
                vec![("opt_fields".to_owned(), STORY_FIELDS.to_owned())],
                page,
            ),
        )
    }

    pub fn list_all_comments(&self, task_gid: &str) -> Result<Vec<Story>> {
        let mut page = Page {
            limit: 100,
            offset: None,
        };
        let mut comments = Vec::new();

        loop {
            let response = self.list_stories(task_gid, &page)?;
            comments.extend(response.data.into_iter().filter(Story::is_comment));
            let next_offset = response.next_page.and_then(|next| next.offset);
            let Some(next_offset) = next_offset else {
                break;
            };
            page.offset = Some(next_offset);
        }

        Ok(comments)
    }

    pub fn list_attachments(
        &self,
        task_gid: &str,
        page: &Page,
    ) -> Result<ApiResponse<Vec<Attachment>>> {
        self.get_json(
            "attachments",
            &with_page(
                vec![
                    ("parent".to_owned(), task_gid.to_owned()),
                    ("opt_fields".to_owned(), ATTACHMENT_FIELDS.to_owned()),
                ],
                page,
            ),
        )
    }

    pub fn list_all_attachments(&self, task_gid: &str) -> Result<Vec<Attachment>> {
        let mut page = Page {
            limit: 100,
            offset: None,
        };
        let mut attachments = Vec::new();

        loop {
            let response = self.list_attachments(task_gid, &page)?;
            attachments.extend(response.data);
            let next_offset = response.next_page.and_then(|next| next.offset);
            let Some(next_offset) = next_offset else {
                break;
            };
            page.offset = Some(next_offset);
        }

        Ok(attachments)
    }

    pub fn get_attachment(&self, gid: &str) -> Result<Attachment> {
        Ok(self
            .get_json(
                &format!("attachments/{gid}"),
                &[("opt_fields".to_owned(), ATTACHMENT_FIELDS.to_owned())],
            )?
            .data)
    }

    pub fn download_url_to_writer<W: Write>(&self, url: &str, writer: &mut W) -> Result<u64> {
        let url = Url::parse(url).map_err(|error| Error::Api {
            status: 0,
            message: format!("Asana returned an invalid attachment download URL: {error}"),
            request_id: None,
        })?;
        let mut response = self.request_with_retry(url, &[], false)?;
        std::io::copy(&mut response, writer).map_err(Error::Io)
    }

    fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
    ) -> Result<ApiResponse<T>> {
        let url = self.api_url(path)?;
        let response = self.request_with_retry(url, params, true)?;
        let body = response.text()?;
        serde_json::from_str(&body).map_err(Error::Json)
    }

    fn api_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path)
            .map_err(|error| Error::Config(format!("could not build Asana API URL: {error}")))
    }

    fn request_with_retry(
        &self,
        url: Url,
        params: &[(String, String)],
        authorized: bool,
    ) -> Result<Response> {
        const ATTEMPTS: u8 = 3;

        for attempt in 0..ATTEMPTS {
            if self.verbose {
                eprintln!(
                    "GET {}{}",
                    redacted_url(&url),
                    if attempt == 0 { "" } else { " (retry)" }
                );
            }

            let mut request = self
                .http
                .get(url.clone())
                .query(params)
                .header(ACCEPT, "application/json");
            if authorized {
                request = request.bearer_auth(&self.token);
            }

            match request.send() {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => {
                    let status = response.status();
                    if is_retryable_status(status) && attempt + 1 < ATTEMPTS {
                        let wait = retry_delay(response.headers(), attempt);
                        if self.verbose {
                            eprintln!("Asana returned {status}; retrying in {}s", wait.as_secs());
                        }
                        thread::sleep(wait);
                        continue;
                    }
                    return Err(api_error(response));
                }
                Err(error)
                    if (error.is_connect() || error.is_timeout()) && attempt + 1 < ATTEMPTS =>
                {
                    let wait = Duration::from_secs(u64::from(attempt + 1));
                    if self.verbose {
                        eprintln!("request failed ({error}); retrying in {}s", wait.as_secs());
                    }
                    thread::sleep(wait);
                }
                Err(error) => return Err(Error::Network(error)),
            }
        }

        unreachable!("each retry-loop branch returns on its final attempt")
    }
}

fn normalize_base_url(raw: &str) -> Result<Url> {
    let raw = if raw.ends_with('/') {
        raw.to_owned()
    } else {
        format!("{raw}/")
    };
    Url::parse(&raw)
        .map_err(|error| Error::Config(format!("ASANA_API_BASE_URL is not a valid URL: {error}")))
}

fn with_page(mut params: Vec<(String, String)>, page: &Page) -> Vec<(String, String)> {
    params.push(("limit".to_owned(), page.limit.to_string()));
    if let Some(offset) = &page.offset {
        params.push(("offset".to_owned(), offset.clone()));
    }
    params
}

fn push_option(params: &mut Vec<(String, String)>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        params.push((key.to_owned(), value.to_owned()));
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::BAD_GATEWAY
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::GATEWAY_TIMEOUT
}

fn retry_delay(headers: &HeaderMap, attempt: u8) -> Duration {
    let from_header = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    Duration::from_secs(from_header.unwrap_or(u64::from(attempt + 1)))
}

fn redacted_url(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    safe.to_string()
}

fn api_error(response: Response) -> Error {
    let status = response.status();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value: &HeaderValue| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response.text().unwrap_or_default();

    let message = serde_json::from_str::<ApiErrorResponse>(&body)
        .ok()
        .and_then(|response| {
            let messages = response
                .errors
                .into_iter()
                .filter_map(|error| error.message.or(error.phrase))
                .collect::<Vec<_>>();
            (!messages.is_empty()).then(|| messages.join("; "))
        })
        .unwrap_or_else(|| {
            let fallback = body.trim();
            if fallback.is_empty() {
                status
                    .canonical_reason()
                    .unwrap_or("request failed")
                    .to_owned()
            } else {
                fallback.to_owned()
            }
        });

    Error::Api {
        status: status.as_u16(),
        message,
        request_id,
    }
}
