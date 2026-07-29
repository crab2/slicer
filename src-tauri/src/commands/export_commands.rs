use crate::artifacts::media_exporter::{MediaExporter, MediaExportResult};
use crate::errors::AppError;
use crate::services::workspace_service::WorkspaceService;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn export_media(
    workspace: State<'_, WorkspaceService>,
    destination: String,
) -> Result<MediaExportResult, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let dest = PathBuf::from(&destination);
        MediaExporter::export(&workspace, &dest)
    })
    .await
    .map_err(|err| {
        AppError::new(
            "export_task_join_failed",
            "导出任务执行失败。",
            "export",
            true,
        )
        .with_details(err.to_string())
    })?
}
