use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Usage(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error("Asana API error ({status}): {message}")]
    Api {
        status: u16,
        message: String,
        request_id: Option<String>,
    },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not parse Asana response: {0}")]
    Json(#[from] serde_json::Error),

    #[error("partial download failure: {0}")]
    PartialDownload(String),

    #[error("destructive operation was not confirmed: {0}")]
    Confirmation(String),

    #[error("write outcome is ambiguous: {0}")]
    AmbiguousWrite(String),

    #[error("stale resource precondition: {0}")]
    Stale(String),

    #[error("asynchronous job failed or timed out: {0}")]
    Job(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Config(_) => 2,
            Self::Auth(_) => 3,
            Self::Api { status: 401, .. } => 3,
            Self::Api { .. } => 4,
            Self::PartialDownload(_) => 5,
            Self::Confirmation(_) => 6,
            Self::AmbiguousWrite(_) | Self::Stale(_) => 7,
            Self::Job(_) => 9,
            Self::Network(_) | Self::Io(_) | Self::Json(_) => 1,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            Self::Config(_) => "config",
            Self::Auth(_) => "auth",
            Self::Api { status: 401, .. } => "auth",
            Self::Api { status: 402, .. } => "plan_required",
            Self::Api { status: 403, .. } => "permission",
            Self::Api { .. } => "api",
            Self::Network(_) => "network",
            Self::Io(_) => "io",
            Self::Json(_) => "invalid_response",
            Self::PartialDownload(_) => "partial_download",
            Self::Confirmation(_) => "confirmation_required",
            Self::AmbiguousWrite(_) => "ambiguous_write",
            Self::Stale(_) => "stale_resource",
            Self::Job(_) => "job_failed",
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
