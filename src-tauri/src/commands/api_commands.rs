use crate::api::state::ApiAppState;
use crate::domain::settings::ApiServerStatusDto;
use crate::services::api_server_service::ApiServerService;
use tauri::State;

#[tauri::command]
pub fn get_api_server_status(api_server: State<'_, ApiServerService>) -> ApiServerStatusDto {
    api_server.get_runtime_status()
}

/// Reset the API token. Returns the new token value.
/// The token is stored in-memory and used for protecting write endpoints.
#[tauri::command]
pub fn reset_api_token(api_state: State<'_, ApiAppState>) -> String {
    api_state.reset_token()
}
