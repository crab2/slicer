use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum DocumentLifecycleStatus {
    Pending,
    Importing,
    Ready,
    Failed,
}

impl DocumentLifecycleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Importing => "importing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

pub fn document_lifecycle_statuses() -> Vec<String> {
    [
        DocumentLifecycleStatus::Pending,
        DocumentLifecycleStatus::Importing,
        DocumentLifecycleStatus::Ready,
        DocumentLifecycleStatus::Failed,
    ]
    .into_iter()
    .map(|status| status.as_str().to_string())
    .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentDto {
    pub document_id: String,
    pub original_filename: String,
    pub file_type: String,
    pub file_hash: String,
    pub original_path: String,
    pub page_count: Option<i64>,
    pub status: String,
    pub error_summary: Option<String>,
    pub job_id: Option<String>,
    pub analysis_succeeded_pages: i64,
    pub analysis_failed_pages: i64,
    pub last_analyzed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
