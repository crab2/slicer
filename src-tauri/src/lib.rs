mod artifacts;
mod commands;
mod domain;
mod errors;
mod jobs;
mod repositories;
mod services;

use commands::diagnostics_commands::record_diagnostic_error;
use commands::job_commands::{
    create_job, create_placeholder_job, fail_job, get_core_status_catalog, list_jobs,
    recover_interrupted_jobs, update_job_progress,
};
use commands::settings_commands::get_app_settings;
use commands::workspace_commands::{get_workspace_status, select_workspace};
use services::workspace_service::WorkspaceService;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(WorkspaceService::new(resolve_config_dir()))
        .invoke_handler(tauri::generate_handler![
            get_workspace_status,
            select_workspace,
            get_app_settings,
            get_core_status_catalog,
            list_jobs,
            create_placeholder_job,
            create_job,
            update_job_progress,
            fail_job,
            recover_interrupted_jobs,
            record_diagnostic_error
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn resolve_config_dir() -> std::path::PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return std::path::PathBuf::from(app_data).join("slicer");
    }
    std::env::temp_dir().join("slicer")
}
