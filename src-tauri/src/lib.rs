pub mod api;
mod artifacts;
mod commands;
mod diagnostics;
mod domain;
mod errors;
mod jobs;
mod providers;
mod repositories;
mod security;
mod services;

use commands::analysis_commands::{
    analyze_new_pages, analyze_page, list_workbench_pages, reanalyze_document,
    reanalyze_failed_pages, recover_interrupted_analysis_pages,
};
use commands::api_commands::{get_api_server_status, reset_api_token};
use commands::diagnostics_commands::record_diagnostic_error;
use commands::export_commands::export_media;
use commands::import_commands::{
    delete_document, import_image, import_pdf, list_documents, list_pages, retry_import,
};
use commands::job_commands::{
    create_job, create_placeholder_job, fail_job, get_core_status_catalog, list_jobs,
    recover_interrupted_jobs, update_job_progress,
};
use commands::search_commands::{
    get_index_status, get_page_image_preview, search_pages, start_index_rebuild,
};
use commands::settings_commands::{
    accept_privacy_notice, activate_api_key, activate_model_profile, add_api_key, delete_api_key,
    delete_api_key_record, delete_model_profile, delete_provider_api_key, find_libreoffice_path,
    get_app_settings, get_model_configuration_status, get_privacy_notice_status, list_api_keys,
    list_model_profiles, list_openai_models, save_api_key, save_app_settings,
    save_provider_api_key, upsert_model_profile,
};
use commands::workspace_commands::{get_workspace_status, select_workspace};
use services::api_server_service::ApiServerService;
use services::settings_service::SettingsService;
use services::workspace_service::WorkspaceService;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_dir = resolve_config_dir();
    let log_dir = config_dir.join("logs");
    let _guard = diagnostics::init_tracing(&log_dir);

    let workspace = WorkspaceService::new(config_dir);
    let api_state = crate::api::state::ApiAppState::new(Arc::new(workspace.clone()));
    api_state.reset_token();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(workspace)
        .manage(api_state.clone())
        .manage(ApiServerService::new(api_state))
        .setup(|app| {
            let workspace = app.state::<WorkspaceService>();
            let api_server = app.state::<ApiServerService>();
            match SettingsService::get_settings(&workspace) {
                Ok(settings) => {
                    if settings.api_enabled {
                        if let Err(err) = api_server.start(&settings) {
                            tracing::warn!(
                                target: "api",
                                code = %err.code,
                                "应用启动时 localhost API 启动失败"
                            );
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        target: "api",
                        code = %err.code,
                        "应用启动时无法加载设置以决定 localhost API 状态"
                    );
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_workspace_status,
            select_workspace,
            get_app_settings,
            save_app_settings,
            find_libreoffice_path,
            save_api_key,
            save_provider_api_key,
            list_api_keys,
            add_api_key,
            activate_api_key,
            delete_api_key_record,
            delete_api_key,
            delete_provider_api_key,
            list_model_profiles,
            upsert_model_profile,
            activate_model_profile,
            delete_model_profile,
            get_model_configuration_status,
            list_openai_models,
            get_privacy_notice_status,
            accept_privacy_notice,
            get_core_status_catalog,
            list_jobs,
            create_placeholder_job,
            create_job,
            update_job_progress,
            fail_job,
            recover_interrupted_jobs,
            recover_interrupted_analysis_pages,
            record_diagnostic_error,
            analyze_page,
            analyze_new_pages,
            reanalyze_document,
            reanalyze_failed_pages,
            list_workbench_pages,
            import_image,
            import_pdf,
            retry_import,
            delete_document,
            list_documents,
            list_pages,
            get_index_status,
            search_pages,
            get_page_image_preview,
            start_index_rebuild,
            get_api_server_status,
            reset_api_token,
            export_media
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
