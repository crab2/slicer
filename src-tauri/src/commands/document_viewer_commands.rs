use crate::domain::document_viewer::{DocumentViewerContentDto, DocumentViewerManifestDto};
use crate::errors::AppError;
use crate::services::document_viewer_service::DocumentViewerService;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub async fn get_document_viewer_manifest(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<DocumentViewerManifestDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        DocumentViewerService::get_manifest(&workspace, &document_id)
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn get_document_viewer_content(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
    format: String,
) -> Result<DocumentViewerContentDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        DocumentViewerService::get_content(&workspace, &document_id, &format)
    })
    .await
    .map_err(join_error)?
}

fn join_error(err: tauri::Error) -> AppError {
    AppError::new(
        "document_viewer_task_join_failed",
        "文档查看任务执行失败。",
        "document_viewer",
        true,
    )
    .with_details(err.to_string())
}
