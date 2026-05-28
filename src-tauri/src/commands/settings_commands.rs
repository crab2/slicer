use crate::domain::settings::{
    ApiKeyListDto, AppSettingsDto, ModelConfigurationStatusDto, PrivacyNoticeStatusDto,
};
use crate::errors::AppError;
use crate::providers::libreoffice_converter;
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
pub fn save_provider_api_key(provider: String, key: String) -> Result<(), AppError> {
    SettingsService::save_api_key_for_provider(&provider, &key)
}

#[tauri::command]
pub fn list_api_keys(workspace: State<'_, WorkspaceService>) -> Result<ApiKeyListDto, AppError> {
    SettingsService::list_api_keys(&workspace)
}

#[tauri::command]
pub fn add_api_key(
    provider: String,
    label: String,
    key: String,
    activate: bool,
    workspace: State<'_, WorkspaceService>,
) -> Result<ApiKeyListDto, AppError> {
    SettingsService::add_api_key(&workspace, &provider, &label, &key, activate)
}

#[tauri::command]
pub fn activate_api_key(
    provider: String,
    key_id: String,
    workspace: State<'_, WorkspaceService>,
) -> Result<ApiKeyListDto, AppError> {
    SettingsService::activate_api_key(&workspace, &provider, &key_id)
}

#[tauri::command]
pub fn delete_api_key_record(
    provider: String,
    key_id: String,
    workspace: State<'_, WorkspaceService>,
) -> Result<ApiKeyListDto, AppError> {
    SettingsService::delete_api_key_record(&workspace, &provider, &key_id)
}

#[tauri::command]
pub fn delete_api_key() -> Result<(), AppError> {
    SettingsService::delete_api_key()
}

#[tauri::command]
pub fn delete_provider_api_key(provider: String) -> Result<(), AppError> {
    SettingsService::delete_api_key_for_provider(&provider)
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

#[tauri::command]
pub fn find_libreoffice_path() -> Option<String> {
    libreoffice_converter::find_libreoffice_installation()
        .map(|path| path.to_string_lossy().to_string())
}
