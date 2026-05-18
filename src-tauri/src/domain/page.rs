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
