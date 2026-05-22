use crate::domain::settings::{
    AppSettingsDto, ModelConfigurationStatusDto, PrivacyNoticeStatusDto,
};
use crate::errors::AppError;
use crate::services::api_server_service::ApiServerService;
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
pub fn save_app_settings(
    settings: AppSettingsDto,
    workspace: State<'_, WorkspaceService>,
    api_server: State<'_, ApiServerService>,
) -> Result<(), AppError> {
    SettingsService::save_settings(&workspace, &api_server, &settings)
}

#[tauri::command]
pub fn save_api_key(key: String) -> Result<(), AppError> {
    SettingsService::save_api_key(&key)
}

#[tauri::command]
pub fn delete_api_key() -> Result<(), AppError> {
    SettingsService::delete_api_key()
}

#[tauri::command]
pub fn get_model_configuration_status(
    workspace: State<'_, WorkspaceService>,
) -> Result<ModelConfigurationStatusDto, AppError> {
    SettingsService::get_model_configuration_status(&workspace)
}

#[tauri::command]
pub fn get_privacy_notice_status(
    workspace: State<'_, WorkspaceService>,
) -> Result<PrivacyNoticeStatusDto, AppError> {
    SettingsService::get_privacy_notice_status(&workspace)
}

#[tauri::command]
pub fn accept_privacy_notice(workspace: State<'_, WorkspaceService>) -> Result<(), AppError> {
    SettingsService::accept_privacy_notice(&workspace)
}
