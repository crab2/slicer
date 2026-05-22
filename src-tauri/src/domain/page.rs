use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum PageLifecycleStatus {
    Pending,
    Rendered,
    AnalysisPending,
    Analyzed,
    Failed,
}

impl PageLifecycleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Rendered => "rendered",
            Self::AnalysisPending => "analysis_pending",
            Self::Analyzed => "analyzed",
            Self::Failed => "failed",
        }
    }
}

pub fn page_lifecycle_statuses() -> Vec<String> {
    [
        PageLifecycleStatus::Pending,
        PageLifecycleStatus::Rendered,
        PageLifecycleStatus::AnalysisPending,
        PageLifecycleStatus::Analyzed,
        PageLifecycleStatus::Failed,
    ]
    .into_iter()
    .map(|status| status.as_str().to_string())
    .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct PageRecordDto {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
    pub image_hash: String,
    pub status: String,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageAssetDto {
    pub image_hash: String,
    pub file_path: String,
    pub file_size: i64,
    pub created_at: String,
}
