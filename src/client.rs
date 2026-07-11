use std::{env, io::Write, path::Path, thread, time::Duration};

use reqwest::{
    Method, StatusCode, Url,
    blocking::multipart::{Form, Part},
    blocking::{Client, Response},
    header::{ACCEPT, HeaderMap, HeaderValue, RETRY_AFTER},
};
use serde::de::DeserializeOwned;
use serde_json::Value;

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
const TASK_LIST_FIELDS: &str = "gid,name,completed,completed_at,assignee.gid,assignee.name,due_on,due_at,modified_at,permalink_url,memberships.project.gid,memberships.project.name,memberships.section.gid,memberships.section.name,tags.gid,tags.name";
const TASK_DETAIL_FIELDS: &str = "gid,name,completed,completed_at,assignee.gid,assignee.name,due_on,due_at,start_on,start_at,notes,html_notes,created_at,modified_at,permalink_url,resource_subtype,workspace.gid,workspace.name,memberships.project.gid,memberships.project.name,memberships.section.gid,memberships.section.name,parent.gid,parent.name,parent.resource_subtype,followers.gid,followers.name,tags.gid,tags.name,num_subtasks,custom_fields.gid,custom_fields.name,custom_fields.resource_subtype,custom_fields.display_value,custom_fields.enum_value.gid,custom_fields.enum_value.name,custom_fields.multi_enum_values.gid,custom_fields.multi_enum_values.name,custom_fields.number_value,custom_fields.text_value,custom_fields.date_value,custom_fields.people_value.gid,custom_fields.people_value.name,custom_type.gid,custom_type.name,custom_type_status_option.gid,custom_type_status_option.name";
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
    pub extra: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskListOptions<'a> {
    pub workspace_gid: Option<&'a str>,
    pub project_gid: Option<&'a str>,
    pub section_gid: Option<&'a str>,
    pub tag_gid: Option<&'a str>,
    pub user_task_list_gid: Option<&'a str>,
    pub assignee: Option<&'a str>,
    pub include_completed: bool,
}

pub struct AsanaClient {
    http: Client,
    base_url: Url,
    token: String,
    verbose: bool,
    fields: Option<String>,
}

impl AsanaClient {
    pub fn new(token: String, verbose: bool) -> Result<Self> {
        Self::new_with_options(token, verbose, None, Duration::from_secs(60))
    }

    pub fn new_with_fields(token: String, verbose: bool, fields: Option<String>) -> Result<Self> {
        Self::new_with_options(token, verbose, fields, Duration::from_secs(60))
    }

    pub fn new_with_options(
        token: String,
        verbose: bool,
        fields: Option<String>,
        request_timeout: Duration,
    ) -> Result<Self> {
        if request_timeout.is_zero() {
            return Err(Error::Usage("request timeout must be positive".to_owned()));
        }
        let raw_base_url =
            env::var("ASANA_API_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        let base_url = normalize_base_url(&raw_base_url)?;
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(request_timeout)
            .user_agent(USER_AGENT)
            .build()?;

        Ok(Self {
            http,
            base_url,
            token,
            verbose,
            fields,
        })
    }

    pub fn current_user(&self) -> Result<User> {
        Ok(self
            .get_json(
                "users/me",
                &[("opt_fields".to_owned(), self.fields("gid,name,email"))],
            )?
            .data)
    }

    pub fn list_workspaces(&self, page: &Page) -> Result<ApiResponse<Vec<Workspace>>> {
        self.get_json(
            "workspaces",
            &with_page(
                vec![("opt_fields".to_owned(), self.fields(WORKSPACE_FIELDS))],
                page,
            ),
        )
    }

    pub fn get_workspace(&self, gid: &str) -> Result<Workspace> {
        Ok(self
            .get_json(
                &format!("workspaces/{gid}"),
                &[("opt_fields".to_owned(), self.fields(WORKSPACE_FIELDS))],
            )?
            .data)
    }

    pub fn list_projects(
        &self,
        workspace_gid: &str,
        archived: Option<bool>,
        page: &Page,
    ) -> Result<ApiResponse<Vec<Project>>> {
        let mut params = vec![("opt_fields".to_owned(), self.fields(PROJECT_FIELDS))];
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
                vec![("opt_fields".to_owned(), self.fields(SECTION_FIELDS))],
                page,
            ),
        )
    }

    pub fn list_tasks(
        &self,
        options: &TaskListOptions<'_>,
        page: &Page,
    ) -> Result<ApiResponse<Vec<Task>>> {
        let mut params = vec![("opt_fields".to_owned(), self.fields(TASK_LIST_FIELDS))];

        if let Some(workspace_gid) = options.workspace_gid {
            params.push(("workspace".to_owned(), workspace_gid.to_owned()));
        }
        if let Some(project_gid) = options.project_gid {
            params.push(("project".to_owned(), project_gid.to_owned()));
        }
        if let Some(assignee) = options.assignee {
            params.push(("assignee".to_owned(), assignee.to_owned()));
        }
        if let Some(tag_gid) = options.tag_gid {
            params.push(("tag".to_owned(), tag_gid.to_owned()));
        }
        if !options.include_completed {
            params.push(("completed_since".to_owned(), "now".to_owned()));
        }

        let path = if let Some(gid) = options.section_gid {
            format!("sections/{gid}/tasks")
        } else if let Some(gid) = options.user_task_list_gid {
            format!("user_task_lists/{gid}/tasks")
        } else {
            "tasks".to_owned()
        };

        self.get_json(&path, &with_page(params, page))
    }

    pub fn search_tasks(
        &self,
        workspace_gid: &str,
        options: &TaskSearchOptions,
    ) -> Result<ApiResponse<Vec<Task>>> {
        let mut params = vec![
            ("opt_fields".to_owned(), self.fields(TASK_LIST_FIELDS)),
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
        params.extend(options.extra.clone());

        self.get_json(&format!("workspaces/{workspace_gid}/tasks/search"), &params)
    }

    pub fn get_task(&self, gid: &str) -> Result<Task> {
        Ok(self
            .get_json(
                &format!("tasks/{gid}"),
                &[("opt_fields".to_owned(), self.fields(TASK_DETAIL_FIELDS))],
            )?
            .data)
    }

    pub fn get_task_modified_at(&self, gid: &str) -> Result<Option<String>> {
        let task: Task = self
            .get_json(
                &format!("tasks/{gid}"),
                &[("opt_fields".to_owned(), "gid,modified_at".to_owned())],
            )?
            .data;
        Ok(task.modified_at)
    }

    pub fn list_stories(&self, task_gid: &str, page: &Page) -> Result<ApiResponse<Vec<Story>>> {
        self.get_json(
            &format!("tasks/{task_gid}/stories"),
            &with_page(
                vec![("opt_fields".to_owned(), self.fields(STORY_FIELDS))],
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
                    ("opt_fields".to_owned(), self.fields(ATTACHMENT_FIELDS)),
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
                &[("opt_fields".to_owned(), self.fields(ATTACHMENT_FIELDS))],
            )?
            .data)
    }

    pub fn download_url_to_writer<W: Write>(&self, url: &str, writer: &mut W) -> Result<u64> {
        let url = Url::parse(url).map_err(|error| Error::Api {
            status: 0,
            message: format!("Asana returned an invalid attachment download URL: {error}"),
            request_id: None,
        })?;
        let mut response = self.send_external_get(url)?;
        std::io::copy(&mut response, writer).map_err(Error::Io)
    }

    pub fn raw_json(
        &self,
        method: Method,
        path: &str,
        params: &[(String, String)],
        data: Option<&Value>,
    ) -> Result<Value> {
        let mut params = params.to_vec();
        if method == Method::GET
            && let Some(fields) = &self.fields
            && !params.iter().any(|(key, _)| key == "opt_fields")
        {
            params.push(("opt_fields".to_owned(), fields.clone()));
        }
        let mut body = data.map(|data| serde_json::json!({ "data": data }));
        if method != Method::GET
            && let Some(fields) = &self.fields
        {
            let envelope = body.get_or_insert_with(|| serde_json::json!({ "data": {} }));
            envelope["options"] = serde_json::json!({ "fields": split_fields(fields) });
        }

        let response = self.send_api(method, path, &params, body.as_ref())?;
        let body = response.text()?;
        if body.trim().is_empty() {
            return Ok(serde_json::json!({ "data": {} }));
        }
        serde_json::from_str(&body).map_err(Error::Json)
    }

    pub fn raw_upload(
        &self,
        path: &str,
        params: &[(String, String)],
        file_field: &str,
        file: &Path,
        fields: &[(String, String)],
    ) -> Result<Value> {
        validate_api_path(path)?;
        let mut params = params.to_vec();
        if let Some(fields) = &self.fields
            && !params.iter().any(|(key, _)| key == "opt_fields")
        {
            params.push(("opt_fields".to_owned(), fields.clone()));
        }
        let url = self.api_url(path.trim_start_matches('/'))?;
        let filename = file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::Usage(format!(
                    "file path has no valid filename: {}",
                    file.display()
                ))
            })?;
        let part = Part::file(file)?.file_name(encoded_filename(filename));
        let mut form = Form::new().part(file_field.to_owned(), part);
        for (key, value) in fields {
            form = form.text(key.clone(), value.clone());
        }

        if self.verbose {
            eprintln!("POST {} (multipart)", redacted_url(&url));
        }
        let response = self
            .http
            .post(url)
            .query(&params)
            .bearer_auth(&self.token)
            .header(ACCEPT, "application/json")
            .multipart(form)
            .send()
            .map_err(|error| Error::AmbiguousWrite(format!(
                "the upload request may or may not have reached Asana ({error}); inspect the target before retrying"
            )))?;
        let response = ensure_success(response)?;
        let body = response.text()?;
        serde_json::from_str(&body).map_err(Error::Json)
    }

    pub fn raw_form(
        &self,
        path: &str,
        params: &[(String, String)],
        fields: &[(String, String)],
    ) -> Result<Value> {
        validate_api_path(path)?;
        let mut params = params.to_vec();
        if let Some(fields) = &self.fields
            && !params.iter().any(|(key, _)| key == "opt_fields")
        {
            params.push(("opt_fields".to_owned(), fields.clone()));
        }
        let url = self.api_url(path.trim_start_matches('/'))?;
        let mut form = Form::new();
        for (key, value) in fields {
            form = form.text(key.clone(), value.clone());
        }
        if self.verbose {
            eprintln!("POST {} (multipart)", redacted_url(&url));
        }
        let response = self
            .http
            .post(url)
            .query(&params)
            .bearer_auth(&self.token)
            .header(ACCEPT, "application/json")
            .multipart(form)
            .send()
            .map_err(|error| Error::AmbiguousWrite(format!(
                "the multipart request may or may not have reached Asana ({error}); inspect the target before retrying"
            )))?;
        let response = ensure_success(response)?;
        let body = response.text()?;
        serde_json::from_str(&body).map_err(Error::Json)
    }

    pub fn get_job(&self, gid: &str) -> Result<Value> {
        let fields = self
            .fields
            .as_deref()
            .map(|fields| format!("{fields},status"))
            .unwrap_or_else(|| {
                "gid,status,new_task,new_project,new_portfolio,new_project_template".to_owned()
            });
        Ok(self
            .get_json(&format!("jobs/{gid}"), &[("opt_fields".to_owned(), fields)])?
            .data)
    }

    fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
    ) -> Result<ApiResponse<T>> {
        let response = self.send_api(Method::GET, path, params, None)?;
        let body = response.text()?;
        serde_json::from_str(&body).map_err(Error::Json)
    }

    fn api_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path)
            .map_err(|error| Error::Config(format!("could not build Asana API URL: {error}")))
    }

    fn send_api(
        &self,
        method: Method,
        path: &str,
        params: &[(String, String)],
        body: Option<&Value>,
    ) -> Result<Response> {
        validate_api_path(path)?;
        let url = self.api_url(path.trim_start_matches('/'))?;
        let retryable_method = method == Method::GET;
        const ATTEMPTS: u8 = 3;

        for attempt in 0..ATTEMPTS {
            if self.verbose {
                eprintln!(
                    "{} {}{}",
                    method,
                    redacted_url(&url),
                    if attempt == 0 { "" } else { " (retry)" }
                );
            }

            let mut request = self
                .http
                .request(method.clone(), url.clone())
                .query(params)
                .header(ACCEPT, "application/json")
                .bearer_auth(&self.token);
            if let Some(body) = body {
                request = request.json(body);
            }

            match request.send() {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => {
                    let status = response.status();
                    if retryable_method && is_retryable_status(status) && attempt + 1 < ATTEMPTS {
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
                    if retryable_method
                        && (error.is_connect() || error.is_timeout())
                        && attempt + 1 < ATTEMPTS =>
                {
                    let wait = Duration::from_secs(u64::from(attempt + 1));
                    if self.verbose {
                        eprintln!("request failed ({error}); retrying in {}s", wait.as_secs());
                    }
                    thread::sleep(wait);
                }
                Err(error) if retryable_method => return Err(Error::Network(error)),
                Err(error) => {
                    return Err(Error::AmbiguousWrite(format!(
                        "the {} request may or may not have reached Asana ({error}); inspect the target before retrying",
                        method
                    )));
                }
            }
        }

        unreachable!("each retry-loop branch returns on its final attempt")
    }

    fn send_external_get(&self, url: Url) -> Result<Response> {
        const ATTEMPTS: u8 = 3;
        for attempt in 0..ATTEMPTS {
            if self.verbose {
                eprintln!("GET {}", redacted_url(&url));
            }
            match self.http.get(url.clone()).send() {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response)
                    if is_retryable_status(response.status()) && attempt + 1 < ATTEMPTS =>
                {
                    thread::sleep(retry_delay(response.headers(), attempt));
                }
                Ok(response) => return Err(api_error(response)),
                Err(error)
                    if (error.is_connect() || error.is_timeout()) && attempt + 1 < ATTEMPTS =>
                {
                    thread::sleep(Duration::from_secs(u64::from(attempt + 1)));
                }
                Err(error) => return Err(Error::Network(error)),
            }
        }
        unreachable!("each retry-loop branch returns on its final attempt")
    }

    fn fields(&self, defaults: &str) -> String {
        self.fields.clone().unwrap_or_else(|| defaults.to_owned())
    }
}

fn split_fields(fields: &str) -> Vec<&str> {
    fields
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect()
}

fn encoded_filename(filename: &str) -> String {
    let mut encoded = String::new();
    for byte in filename.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn validate_api_path(path: &str) -> Result<()> {
    let path = path.trim();
    if path.is_empty()
        || path.contains("://")
        || path.contains('?')
        || path.contains('#')
        || path.split('/').any(|segment| segment == "..")
    {
        return Err(Error::Usage(
            "API path must be a relative path without a host, query, fragment, or '..'".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_success(response: Response) -> Result<Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(api_error(response))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_hosts_queries_fragments_and_parent_segments() {
        for path in [
            "https://example.com/tasks/1",
            "/tasks?project=1",
            "/tasks#fragment",
            "/tasks/../users",
        ] {
            assert!(validate_api_path(path).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn accepts_relative_api_paths() {
        validate_api_path("/tasks/123/stories").expect("valid API path");
    }

    #[test]
    fn percent_encodes_non_ascii_attachment_filename() {
        assert_eq!(encoded_filename("résumé 1.pdf"), "r%C3%A9sum%C3%A9%201.pdf");
    }
}
