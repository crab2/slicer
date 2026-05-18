use crate::domain::document::document_lifecycle_statuses;
use crate::domain::page::page_lifecycle_statuses;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct CoreStatusCatalogDto {
    pub document_statuses: Vec<String>,
    pub page_statuses: Vec<String>,
    pub job_statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

pub fn core_status_catalog() -> CoreStatusCatalogDto {
    CoreStatusCatalogDto {
        document_statuses: document_lifecycle_statuses(),
        page_statuses: page_lifecycle_statuses(),
        job_statuses: [
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Succeeded,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ]
        .into_iter()
        .map(|status| status.as_str().to_string())
        .collect(),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobDto {
    pub job_id: String,
    pub job_type: String,
    pub status: String,
    pub progress: u8,
    pub created_at: String,
    pub updated_at: String,
    pub error_id: Option<String>,
    pub error_summary: Option<String>,
    pub last_event_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateJobRequestDto {
    pub job_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateJobProgressRequestDto {
    pub job_id: String,
    pub progress: u8,
    pub message: Option<String>,
}
