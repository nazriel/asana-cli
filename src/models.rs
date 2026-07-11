use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    #[serde(default)]
    pub next_page: Option<NextPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextPage {
    pub offset: Option<String>,
    pub path: Option<String>,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    #[serde(default)]
    pub errors: Vec<ApiErrorItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorItem {
    pub message: Option<String>,
    pub phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub gid: String,
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub gid: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub gid: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub is_organization: Option<bool>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub gid: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub archived: Option<bool>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub owner: Option<Resource>,
    #[serde(default)]
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub gid: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub assignee: Option<Resource>,
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub start_on: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub html_notes: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub resource_subtype: Option<String>,
    #[serde(default)]
    pub workspace: Option<Resource>,
    #[serde(default)]
    pub memberships: Option<Vec<TaskMembership>>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMembership {
    #[serde(default)]
    pub project: Option<Resource>,
    #[serde(default)]
    pub section: Option<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub gid: String,
    #[serde(default)]
    pub resource_subtype: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Resource>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub html_text: Option<String>,
    #[serde(default)]
    pub is_edited: Option<bool>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

impl Story {
    pub fn is_comment(&self) -> bool {
        self.resource_subtype.as_deref() == Some("comment_added")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub gid: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub parent: Option<Resource>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub permanent_url: Option<String>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Collection<T> {
    pub data: Vec<T>,
    pub next_offset: Option<String>,
}

impl<T> Collection<T> {
    pub fn from_response(response: ApiResponse<Vec<T>>) -> Self {
        Self {
            data: response.data,
            next_offset: response.next_page.and_then(|page| page.offset),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskWithRelations {
    #[serde(flatten)]
    pub task: Task,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<Story>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthStatus {
    pub user: User,
    pub token_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_gid: Option<String>,
}
