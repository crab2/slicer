use crate::domain::settings::AppSettingsDto;
use crate::errors::AppError;
use crate::services::settings_service::SettingsService;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub fn get_app_settings(
    workspace: State<'_, WorkspaceService>,
) -> Result<AppSettingsDto, AppError> {
    SettingsService::get_settings(&workspace)
}

#[tauri::command]
pub fn save_api_key(key: String) -> Result<(), AppError> {
    SettingsService::save_api_key(&key)
}

#[tauri::command]
pub fn delete_api_key() -> Result<(), AppError> {
    SettingsService::delete_api_key()
}
