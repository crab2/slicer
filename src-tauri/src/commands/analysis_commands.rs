use crate::domain::analysis::{AnalysisBatchResultDto, AnalysisResultDto, PageWorkbenchDto};
use crate::errors::AppError;
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::services::analysis_service::AnalysisService;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub async fn analyze_page(
    workspace: State<'_, WorkspaceService>,
    page_id: String,
) -> Result<AnalysisResultDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        AnalysisService::analyze_page(&workspace, &page_id)
    })
    .await
    .map_err(|err| {
        AppError::new(
            "analysis_task_join_failed",
            "页面分析任务执行失败。",
            "analysis",
            true,
        )
        .with_details(err.to_string())
    })?
}

#[tauri::command]
pub async fn analyze_new_pages(
    workspace: State<'_, WorkspaceService>,
) -> Result<AnalysisBatchResultDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || AnalysisService::analyze_new_pages(&workspace))
        .await
        .map_err(|err| {
            AppError::new(
                "analysis_task_join_failed",
                "批量分析任务执行失败。",
                "analysis",
                true,
            )
            .with_details(err.to_string())
        })?
}

#[tauri::command]
pub async fn reanalyze_document(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<AnalysisBatchResultDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        AnalysisService::reanalyze_document(&workspace, &document_id)
    })
    .await
    .map_err(|err| {
        AppError::new(
            "analysis_task_join_failed",
            "文档重新分析任务执行失败。",
            "analysis",
            true,
        )
        .with_details(err.to_string())
    })?
}

#[tauri::command]
pub async fn reanalyze_failed_pages(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<AnalysisBatchResultDto, AppError> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        AnalysisService::reanalyze_failed_pages(&workspace, &document_id)
    })
    .await
    .map_err(|err| {
        AppError::new(
            "analysis_task_join_failed",
            "文档失败页面重新分析任务执行失败。",
            "analysis",
            true,
        )
        .with_details(err.to_string())
    })?
}

#[tauri::command]
pub fn list_workbench_pages(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<Vec<PageWorkbenchDto>, AppError> {
    let mut conn = workspace.get_db_connection()?;
    AnalysisRepository::list_workbench_pages(&mut conn, &document_id)
}

#[tauri::command]
pub fn recover_interrupted_analysis_pages(
    workspace: State<'_, WorkspaceService>,
) -> Result<u64, AppError> {
    AnalysisService::recover_interrupted_analysis_pages(&workspace)
}
