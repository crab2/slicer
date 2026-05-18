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
