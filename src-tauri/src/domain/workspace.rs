use crate::errors::AppError;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum WorkspaceStatus {
    NotSelected,
    Ready,
    Missing,
    Invalid,
    Error,
}

impl WorkspaceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotSelected => "not_selected",
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceStatusDto {
    pub status: String,
    pub workspace_path: Option<String>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone)]
pub struct CurrentWorkspace {
    pub root: PathBuf,
}
