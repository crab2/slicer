use crate::services::workspace_service::WorkspaceService;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct ApiAppState {
    pub workspace: Arc<WorkspaceService>,
    pub api_token: Arc<RwLock<Option<String>>>,
}

impl ApiAppState {
    pub fn new(workspace: Arc<WorkspaceService>) -> Self {
        Self {
            workspace,
            api_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Generate a new API token and store it. Returns the new token.
    pub fn reset_token(&self) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        if let Ok(mut guard) = self.api_token.write() {
            *guard = Some(token.clone());
        }
        token
    }

    /// Create a test instance with a temporary workspace and a pre-set token.
    pub fn for_test(config_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&config_dir);
        let state = Self::new(Arc::new(WorkspaceService::new(config_dir)));
        state.reset_token();
        state
    }
}
