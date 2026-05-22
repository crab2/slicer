use crate::domain::index::{IndexRebuildStartDto, IndexStatusDto, SearchResponseDto};
use crate::errors::AppError;
use crate::services::search_service::SearchService;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub async fn get_index_status(
    workspace: State<'_, WorkspaceService>,
) -> Result<IndexStatusDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || SearchService::get_index_status(&workspace))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn search_pages(
    workspace: State<'_, WorkspaceService>,
    query: String,
    limit: Option<usize>,
) -> Result<SearchResponseDto, AppError> {
    let workspace = workspace.inner().clone();
    let limit = limit.unwrap_or(20);
    tauri::async_runtime::spawn_blocking(move || SearchService::search(&workspace, &query, limit))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn get_page_image_preview(
    workspace: State<'_, WorkspaceService>,
    page_id: String,
) -> Result<Option<String>, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        SearchService::get_page_image_preview(&workspace, &page_id)
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn start_index_rebuild(
    workspace: State<'_, WorkspaceService>,
) -> Result<IndexRebuildStartDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || SearchService::start_index_rebuild(&workspace))
        .await
        .map_err(join_error)?
}

fn join_error(err: tauri::Error) -> AppError {
    AppError::new(
        "search_task_join_failed",
        "搜索任务执行失败。",
        "search",
        true,
    )
    .with_details(err.to_string())
}
