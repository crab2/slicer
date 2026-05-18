use crate::domain::settings::AppSettingsDto;
use crate::errors::{AppError, AppResult};
use crate::security;
use crate::services::workspace_service::WorkspaceService;

pub struct SettingsService;

impl SettingsService {
    pub fn get_settings(workspace: &WorkspaceService) -> AppResult<AppSettingsDto> {
        let status = workspace.get_workspace_status();
        if status.status == "error" {
            return Err(status.error.unwrap_or_else(|| {
                AppError::new(
                    "settings_workspace_error",
                    "工作区状态不可用。",
                    "settings",
                    true,
                )
            }));
        }

        let mut settings = AppSettingsDto::default();
        settings.workspace_path = status.workspace_path;
        settings.api_key_configured = security::has_api_key();
        Ok(settings)
    }

    pub fn save_api_key(key: &str) -> AppResult<()> {
        security::store_api_key(key)
    }

    pub fn delete_api_key() -> AppResult<()> {
        security::delete_api_key()
    }

    pub fn is_api_key_configured() -> bool {
        security::has_api_key()
    }
}
